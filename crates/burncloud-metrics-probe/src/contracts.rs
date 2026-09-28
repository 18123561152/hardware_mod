//! Experimental runtime-metrics API.
//!
//! These types are local research interfaces. They are not part of the
//! accepted BurnCloud Node hardware contract.

use async_trait::async_trait;
pub use burncloud_node_runtime::AcceleratorKind;

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeMetrics {
    pub cpu_usage_percent: Option<f32>,
    pub memory_available_bytes: Option<u64>,
    pub accelerators: Vec<AcceleratorMetrics>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcceleratorMetrics {
    pub kind: AcceleratorKind,
    pub index: Option<u32>,
    pub name: String,
    pub memory_total_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_free_bytes: Option<u64>,
    pub utilization_percent: Option<f32>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MetricsProbeError {
    #[error("runtime metrics sampling failed: {0}")]
    SamplingFailed(String),
}

#[async_trait]
pub trait RuntimeMetricsProbe: Send + Sync {
    async fn sample(&self) -> Result<RuntimeMetrics, MetricsProbeError>;
}
