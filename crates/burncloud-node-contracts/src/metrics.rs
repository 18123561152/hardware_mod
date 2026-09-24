//! Independent runtime metrics contracts.
//!
//! Runtime metrics are separate from the stable `HardwareProbe` contract and
//! are intentionally not part of `HardwareProfile`.

use async_trait::async_trait;

use crate::AcceleratorKind;

// 动态运行指标独立于 HardwareProbe 和 HardwareProfile，不参与稳定硬件事实合约。
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
