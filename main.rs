use std::time::Duration;

use burncloud_hardware_probe::RealHardwareProbe;
use burncloud_metrics_probe::{
    AcceleratorMetrics, MetricsProbeError, RealMetricsProbe, RuntimeMetrics, RuntimeMetricsProbe,
};
use burncloud_node_runtime::{
    AcceleratorKind, AcceleratorProfile, HardwareProbe, HardwareProbeError, HardwareProfile,
};

fn assert_hardware_probe<T: HardwareProbe>() {}

fn assert_metrics_probe<T: RuntimeMetricsProbe>() {}

fn verify_contract_types() {
    let hardware_profile = HardwareProfile {
        cpu_threads: 8,
        memory_bytes: 16 * 1024 * 1024 * 1024,
        disk_available_bytes: 256 * 1024 * 1024 * 1024,
        accelerators: vec![
            AcceleratorProfile {
                kind: AcceleratorKind::Nvidia,
                name: "NVIDIA test device".into(),
                memory_bytes: Some(24 * 1024 * 1024 * 1024),
            },
            AcceleratorProfile {
                kind: AcceleratorKind::Amd,
                name: "AMD test device".into(),
                memory_bytes: None,
            },
            AcceleratorProfile {
                kind: AcceleratorKind::Apple,
                name: "Apple test device".into(),
                memory_bytes: None,
            },
            AcceleratorProfile {
                kind: AcceleratorKind::Other,
                name: "Other test device".into(),
                memory_bytes: None,
            },
        ],
    };
    assert_eq!(hardware_profile.accelerators.len(), 4);

    let runtime_metrics = RuntimeMetrics {
        cpu_usage_percent: Some(42.0),
        memory_available_bytes: None,
        accelerators: vec![AcceleratorMetrics {
            kind: AcceleratorKind::Nvidia,
            index: Some(0),
            name: "NVIDIA metrics device".into(),
            memory_total_bytes: Some(24 * 1024 * 1024 * 1024),
            memory_used_bytes: None,
            memory_free_bytes: None,
            utilization_percent: Some(42.0),
        }],
    };
    assert_eq!(runtime_metrics.accelerators.len(), 1);
    assert_eq!(runtime_metrics.memory_available_bytes, None);

    let hardware_error = HardwareProbeError::DetectionFailed("test failure".into());
    assert_eq!(
        hardware_error.to_string(),
        "hardware detection failed: test failure"
    );

    let metrics_error = MetricsProbeError::SamplingFailed("test failure".into());
    assert_eq!(
        metrics_error.to_string(),
        "runtime metrics sampling failed: test failure"
    );
}

fn verify_hardware_profile(profile: &HardwareProfile) {
    assert!(profile.cpu_threads > 0, "CPU thread count must be positive");
    assert!(profile.memory_bytes > 0, "total memory must be positive");

    for accelerator in &profile.accelerators {
        assert!(
            !accelerator.name.trim().is_empty(),
            "accelerator names must not be empty"
        );
        // `None` is valid when accelerator memory cannot be measured.
        let _ = accelerator.memory_bytes;
    }
}

async fn verify_hardware_probe() {
    assert_hardware_probe::<RealHardwareProbe>();

    let newly_constructed = RealHardwareProbe::new();
    let default_constructed = RealHardwareProbe::default();

    for probe in [
        &newly_constructed as &dyn HardwareProbe,
        &default_constructed,
    ] {
        let first = probe.inspect().await;
        let second = probe.inspect().await;

        match (first, second) {
            (Ok(first_profile), Ok(second_profile)) => {
                verify_hardware_profile(&first_profile);
                verify_hardware_profile(&second_profile);
                assert_eq!(
                    first_profile.cpu_threads, second_profile.cpu_threads,
                    "static CPU thread count must remain stable"
                );
                println!(
                    "HardwareProbe::inspect succeeded: {} CPU threads, {} bytes RAM, {} bytes disk, {} accelerators",
                    second_profile.cpu_threads,
                    second_profile.memory_bytes,
                    second_profile.disk_available_bytes,
                    second_profile.accelerators.len()
                );
            }
            (
                Err(HardwareProbeError::DetectionFailed(first_detail)),
                Err(HardwareProbeError::DetectionFailed(second_detail)),
            ) => {
                assert!(!first_detail.trim().is_empty());
                assert!(!second_detail.trim().is_empty());
                println!(
                    "HardwareProbe::inspect returned the public DetectionFailed error: {second_detail}"
                );
            }
            (first_result, second_result) => {
                panic!(
                    "HardwareProbe::inspect returned inconsistent outcomes: {first_result:?} then {second_result:?}"
                );
            }
        }
    }
}

fn verify_runtime_metrics(metrics: &RuntimeMetrics) {
    if let Some(cpu_usage) = metrics.cpu_usage_percent {
        assert!(
            (0.0..=100.0).contains(&cpu_usage),
            "CPU usage must be within 0..=100"
        );
    }

    for accelerator in &metrics.accelerators {
        assert!(
            !accelerator.name.trim().is_empty(),
            "accelerator metric names must not be empty"
        );
        if let Some(utilization) = accelerator.utilization_percent {
            assert!(
                (0.0..=100.0).contains(&utilization),
                "GPU utilization must be within 0..=100"
            );
        }
    }
}

async fn verify_metrics_probe() {
    assert_metrics_probe::<RealMetricsProbe>();

    let newly_constructed = RealMetricsProbe::new().with_sample_interval(Duration::from_millis(1));
    let default_constructed =
        RealMetricsProbe::default().with_sample_interval(Duration::from_millis(1));

    for probe in [
        &newly_constructed as &dyn RuntimeMetricsProbe,
        &default_constructed,
    ] {
        for sample_index in 1..=2 {
            match probe.sample().await {
                Ok(metrics) => {
                    verify_runtime_metrics(&metrics);
                    println!(
                        "RuntimeMetricsProbe::sample #{sample_index} succeeded: cpu={:?}, memory_available={:?}, accelerators={}",
                        metrics.cpu_usage_percent,
                        metrics.memory_available_bytes,
                        metrics.accelerators.len()
                    );
                }
                Err(MetricsProbeError::SamplingFailed(detail)) => {
                    assert!(!detail.trim().is_empty());
                    println!(
                        "RuntimeMetricsProbe::sample returned the public SamplingFailed error: {detail}"
                    );
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    verify_contract_types();
    verify_hardware_probe().await;
    verify_metrics_probe().await;
    println!("All documented public hardware and metrics interfaces were exercised.");
}
