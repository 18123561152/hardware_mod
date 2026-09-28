use std::sync::OnceLock;

use async_trait::async_trait;
use burncloud_node_runtime::{
    AcceleratorProfile, HardwareProbe, HardwareProbeError, HardwareProfile,
};

use crate::platform::{self, DetectError};

pub struct RealHardwareProbe {
    cached_cpu: OnceLock<CachedCpuIdentity>,
}

#[derive(Debug, Clone)]
struct CachedCpuIdentity {
    threads: usize,
    _brand: Option<String>,
    _model: Option<String>,
}

impl RealHardwareProbe {
    pub fn new() -> Self {
        Self {
            cached_cpu: OnceLock::new(),
        }
    }
}

impl Default for RealHardwareProbe {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HardwareProbe for RealHardwareProbe {
    async fn inspect(&self) -> Result<HardwareProfile, HardwareProbeError> {
        // 这里直接使用 BurnCloud 权威运行时合约。
        // 平台采集结果只映射为稳定的机器级事实。
        // CPU 品牌和型号保留在内部，不扩展公共 HardwareProfile。
        if self.cached_cpu.get().is_none() {
            let raw_cpu = platform::collect_cpu().await.map_err(map_detect_error)?;
            let identity = CachedCpuIdentity {
                threads: raw_cpu.cpu_threads,
                _brand: normalize_cpu_brand(raw_cpu.cpu_brand.as_deref().unwrap_or("")),
                _model: raw_cpu.cpu_model,
            };
            let _ = self.cached_cpu.set(identity);
        }
        let cpu = self.cached_cpu.get().expect("CPU identity is initialized");
        let memory_bytes = platform::collect_memory().await.map_err(map_detect_error)?;
        let disk_available_bytes = platform::collect_disk().await.map_err(map_detect_error)?;
        let raw_accelerators = platform::collect_accelerators()
            .await
            .map_err(map_detect_error)?;
        // 平台原始检测结果在这里统一映射为稳定的 Node HardwareProfile 合约。

        Ok(HardwareProfile {
            cpu_threads: cpu.threads,
            memory_bytes,
            disk_available_bytes,
            accelerators: raw_accelerators
                .into_iter()
                .map(|accelerator| AcceleratorProfile {
                    kind: accelerator.kind,
                    name: accelerator.name,
                    memory_bytes: accelerator.memory_bytes,
                })
                .collect(),
        })
    }
}

fn normalize_cpu_brand(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    let normalized = value.to_ascii_lowercase();
    if normalized.contains("genuineintel") || normalized == "intel" {
        Some("Intel".into())
    } else if normalized.contains("authenticamd") || normalized == "amd" {
        Some("AMD".into())
    } else if normalized.contains("apple") {
        Some("Apple".into())
    } else {
        Some("Unknown".into())
    }
}

fn map_detect_error(error: DetectError) -> HardwareProbeError {
    HardwareProbeError::DetectionFailed(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{map_detect_error, normalize_cpu_brand, RealHardwareProbe};
    use burncloud_node_runtime::{HardwareProbe, HardwareProbeError};

    fn assert_hardware_probe<T: HardwareProbe>() {}

    #[test]
    fn real_probe_implements_the_existing_contract() {
        assert_hardware_probe::<RealHardwareProbe>();
    }

    #[test]
    fn detection_errors_map_to_the_public_contract() {
        let error = map_detect_error(super::DetectError::Command {
            program: "nproc".into(),
            detail: "failed".into(),
        });
        assert!(matches!(error, HardwareProbeError::DetectionFailed(_)));
    }

    #[test]
    fn normalizes_cpu_brands_without_inventing_empty_values() {
        assert_eq!(normalize_cpu_brand("GenuineIntel"), Some("Intel".into()));
        assert_eq!(normalize_cpu_brand("AuthenticAMD"), Some("AMD".into()));
        assert_eq!(normalize_cpu_brand("Apple M1"), Some("Apple".into()));
        assert_eq!(normalize_cpu_brand("Vendor X"), Some("Unknown".into()));
        assert_eq!(normalize_cpu_brand("  "), None);
    }
}
