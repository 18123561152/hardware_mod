//! Stable machine-level hardware facts for BurnCloud Node.

use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HardwareProfile {
    pub cpu_threads: usize,
    pub cpu_brand: Option<String>,
    pub cpu_model: Option<String>,
    pub memory_bytes: u64,
    pub disk_available_bytes: u64,
    pub accelerators: Vec<AcceleratorProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorProfile {
    pub kind: AcceleratorKind,
    pub name: String,
    pub memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceleratorKind {
    Nvidia,
    Amd,
    Apple,
    Other,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HardwareProbeError {
    #[error("hardware detection failed: {0}")]
    DetectionFailed(String),
}

#[async_trait]
pub trait HardwareProbe: Send + Sync {
    async fn inspect(&self) -> Result<HardwareProfile, HardwareProbeError>;
}
