use std::fmt;

use burncloud_node_runtime::AcceleratorKind;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CpuSnapshot {
    pub(crate) total: u64,
    pub(crate) idle: u64,
}

#[derive(Debug)]
pub(crate) struct RawAcceleratorMetrics {
    pub(crate) kind: AcceleratorKind,
    pub(crate) index: Option<u32>,
    pub(crate) name: String,
    pub(crate) memory_total_bytes: Option<u64>,
    pub(crate) memory_used_bytes: Option<u64>,
    pub(crate) memory_free_bytes: Option<u64>,
    pub(crate) utilization_percent: Option<f32>,
}

#[derive(Debug)]
pub(crate) enum MetricsError {
    Parse(String),
    UnsupportedPlatform,
}

impl fmt::Display for MetricsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(detail) => formatter.write_str(detail),
            Self::UnsupportedPlatform => formatter.write_str("unsupported platform"),
        }
    }
}

pub(crate) async fn sample_cpu() -> Result<Option<CpuSnapshot>, MetricsError> {
    // 通过编译期 target_os 选择平台实现，避免运行期混用不同系统命令。
    #[cfg(target_os = "windows")]
    return windows::sample_cpu().await;
    #[cfg(target_os = "linux")]
    return linux::sample_cpu().await;
    #[cfg(target_os = "macos")]
    return macos::sample_cpu().await;
    #[allow(unreachable_code)]
    Err(MetricsError::UnsupportedPlatform)
}

pub(crate) async fn sample_memory_available() -> Result<Option<u64>, MetricsError> {
    #[cfg(target_os = "windows")]
    return windows::sample_memory_available().await;
    #[cfg(target_os = "linux")]
    return linux::sample_memory_available().await;
    #[cfg(target_os = "macos")]
    return macos::sample_memory_available().await;
    #[allow(unreachable_code)]
    Err(MetricsError::UnsupportedPlatform)
}

pub(crate) async fn sample_accelerators() -> Result<Vec<RawAcceleratorMetrics>, MetricsError> {
    let mut accelerators = crate::nvidia::sample_nvidia().await?;
    #[cfg(target_os = "windows")]
    accelerators.extend(windows::sample_vendor_accelerators().await?);
    #[cfg(target_os = "linux")]
    accelerators.extend(linux::sample_vendor_accelerators().await?);
    #[cfg(target_os = "macos")]
    accelerators.extend(macos::sample_vendor_accelerators().await?);
    Ok(accelerators)
}

pub(crate) fn compute_usage(first: CpuSnapshot, second: CpuSnapshot) -> Option<f32> {
    let total_delta = second.total.checked_sub(first.total)?;
    let idle_delta = second.idle.checked_sub(first.idle)?;
    if total_delta == 0 || idle_delta > total_delta {
        return None;
    }
    Some(((total_delta - idle_delta) as f64 / total_delta as f64 * 100.0) as f32)
}

pub(crate) fn parse_u64(label: &str, value: &str) -> Result<u64, MetricsError> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|error| MetricsError::Parse(format!("invalid {label}: {error}")))
}

pub(crate) fn parse_f64(label: &str, value: &str) -> Result<f64, MetricsError> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|error| MetricsError::Parse(format!("invalid {label}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::{compute_usage, CpuSnapshot};

    #[test]
    fn compute_usage_handles_normal_delta() {
        assert_eq!(
            compute_usage(
                CpuSnapshot {
                    total: 100,
                    idle: 40
                },
                CpuSnapshot {
                    total: 200,
                    idle: 90
                }
            ),
            Some(50.0)
        );
    }

    #[test]
    fn compute_usage_handles_zero_and_invalid_deltas() {
        assert_eq!(
            compute_usage(
                CpuSnapshot { total: 10, idle: 5 },
                CpuSnapshot { total: 10, idle: 5 }
            ),
            None
        );
        assert_eq!(
            compute_usage(
                CpuSnapshot {
                    total: 20,
                    idle: 10
                },
                CpuSnapshot { total: 10, idle: 5 }
            ),
            None
        );
        assert_eq!(
            compute_usage(
                CpuSnapshot { total: 10, idle: 2 },
                CpuSnapshot {
                    total: 20,
                    idle: 21
                }
            ),
            None
        );
    }
}
