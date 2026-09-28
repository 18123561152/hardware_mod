use burncloud_hardware_probe::RealHardwareProbe;
use burncloud_node_runtime::{HardwareProbe, HardwareProfile};

fn assert_probe<T: HardwareProbe>() {}

#[test]
fn real_detector_satisfies_the_contract() {
    assert_probe::<RealHardwareProbe>();
}

#[tokio::test]
async fn inspect_is_called_through_the_contract() {
    let probe: &dyn HardwareProbe = &RealHardwareProbe::default();
    let result: Result<HardwareProfile, _> = probe.inspect().await;
    if let Ok(profile) = result {
        assert!(profile.cpu_threads > 0);
    }
}
