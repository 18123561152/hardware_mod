use std::time::Duration;

use crate::{
    platform::{self, compute_usage, MetricsError},
    AcceleratorMetrics, MetricsProbeError, RuntimeMetrics, RuntimeMetricsProbe,
};
use async_trait::async_trait;

pub struct RealMetricsProbe {
    sample_interval: Duration,
}

impl RealMetricsProbe {
    pub fn new() -> Self {
        Self {
            sample_interval: Duration::from_millis(200),
        }
    }

    pub fn with_sample_interval(mut self, interval: Duration) -> Self {
        self.sample_interval = interval;
        self
    }
}

impl Default for RealMetricsProbe {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeMetricsProbe for RealMetricsProbe {
    async fn sample(&self) -> Result<RuntimeMetrics, MetricsProbeError> {
        // 每次 sample 都重新读取动态资源，不缓存上一次采样结果。
        let first_cpu = platform::sample_cpu().await.map_err(map_error)?;
        tokio::time::sleep(self.sample_interval).await;
        let second_cpu = platform::sample_cpu().await.map_err(map_error)?;

        let cpu_usage_percent = match (first_cpu, second_cpu) {
            (Some(first), Some(second)) => compute_usage(first, second),
            _ => None,
        };
        let memory_available_bytes = platform::sample_memory_available()
            .await
            .map_err(map_error)?;
        let accelerators = platform::sample_accelerators()
            .await
            .map_err(map_error)?
            .into_iter()
            .map(|metric| AcceleratorMetrics {
                kind: metric.kind,
                index: metric.index,
                name: metric.name,
                memory_total_bytes: metric.memory_total_bytes,
                memory_used_bytes: metric.memory_used_bytes,
                memory_free_bytes: metric.memory_free_bytes,
                utilization_percent: metric.utilization_percent,
            })
            .collect();

        Ok(RuntimeMetrics {
            cpu_usage_percent,
            memory_available_bytes,
            accelerators,
        })
    }
}

fn map_error(error: MetricsError) -> MetricsProbeError {
    MetricsProbeError::SamplingFailed(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RealMetricsProbe;
    use crate::RuntimeMetricsProbe;

    fn assert_probe<T: RuntimeMetricsProbe>() {}

    #[test]
    fn real_probe_implements_runtime_metrics_contract() {
        assert_probe::<RealMetricsProbe>();
        assert_eq!(
            RealMetricsProbe::new().sample_interval,
            Duration::from_millis(200)
        );
    }

    #[tokio::test]
    async fn sample_returns_option_fields_without_inventing_values() {
        let probe = RealMetricsProbe::new().with_sample_interval(Duration::from_millis(1));
        let metrics = probe
            .sample()
            .await
            .expect("metrics sample should complete");
        assert!(metrics.cpu_usage_percent.is_none() || metrics.cpu_usage_percent.unwrap() <= 100.0);
        assert!(metrics
            .accelerators
            .iter()
            .all(|accelerator| accelerator.memory_total_bytes.is_some()
                || accelerator.memory_used_bytes.is_none()));
    }
}
