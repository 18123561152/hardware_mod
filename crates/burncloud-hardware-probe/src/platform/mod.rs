use std::fmt;
use std::process::Output;

use burncloud_node_contracts::AcceleratorKind;
use tokio::process::Command;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug)]
pub(crate) struct RawCpuIdentity {
    pub(crate) cpu_threads: usize,
    pub(crate) cpu_brand: Option<String>,
    pub(crate) cpu_model: Option<String>,
}

#[derive(Debug)]
pub(crate) struct RawAccelerator {
    pub(crate) kind: AcceleratorKind,
    pub(crate) name: String,
    pub(crate) memory_bytes: Option<u64>,
}

#[derive(Debug)]
pub(crate) enum DetectError {
    Command { program: String, detail: String },
    Parse(String),
    UnsupportedPlatform,
}

impl fmt::Display for DetectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { program, detail } => write!(formatter, "{program}: {detail}"),
            Self::Parse(detail) => formatter.write_str(detail),
            Self::UnsupportedPlatform => formatter.write_str("unsupported platform"),
        }
    }
}

pub(crate) async fn collect_cpu() -> Result<RawCpuIdentity, DetectError> {
    // 使用编译期平台选择，避免运行期分发和跨平台命令代码混杂。
    #[cfg(target_os = "windows")]
    return windows::collect_cpu().await;
    #[cfg(target_os = "linux")]
    return linux::collect_cpu().await;
    #[cfg(target_os = "macos")]
    return macos::collect_cpu().await;
    #[allow(unreachable_code)]
    Err(DetectError::UnsupportedPlatform)
}

pub(crate) async fn collect_memory() -> Result<u64, DetectError> {
    #[cfg(target_os = "windows")]
    return windows::collect_memory().await;
    #[cfg(target_os = "linux")]
    return linux::collect_memory().await;
    #[cfg(target_os = "macos")]
    return macos::collect_memory().await;
    #[allow(unreachable_code)]
    Err(DetectError::UnsupportedPlatform)
}

pub(crate) async fn collect_disk() -> Result<u64, DetectError> {
    #[cfg(target_os = "windows")]
    return windows::collect_disk().await;
    #[cfg(target_os = "linux")]
    return linux::collect_disk().await;
    #[cfg(target_os = "macos")]
    return macos::collect_disk().await;
    #[allow(unreachable_code)]
    Err(DetectError::UnsupportedPlatform)
}

pub(crate) async fn collect_accelerators() -> Result<Vec<RawAccelerator>, DetectError> {
    let mut accelerators = collect_nvidia().await?;
    #[cfg(target_os = "windows")]
    accelerators.extend(windows::collect_vendor_accelerators().await?);
    #[cfg(target_os = "linux")]
    accelerators.extend(linux::collect_vendor_accelerators().await?);
    #[cfg(target_os = "macos")]
    accelerators.extend(macos::collect_vendor_accelerators().await?);
    Ok(accelerators)
}

pub(crate) async fn run_command(program: &str, args: &[&str]) -> Result<Output, DetectError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|error| DetectError::Command {
            program: program.to_string(),
            detail: error.to_string(),
        })?;

    if !output.status.success() {
        return Err(DetectError::Command {
            program: program.to_string(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(output)
}

pub(crate) async fn collect_nvidia() -> Result<Vec<RawAccelerator>, DetectError> {
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .await
    {
        Ok(output) => output,
        Err(_) => return Ok(Vec::new()),
    };

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| DetectError::Parse(format!("invalid nvidia-smi UTF-8: {error}")))?;
    crate::nvidia::parse_nvidia_smi_csv(&stdout)
}

fn parse_u64(label: &str, value: &str) -> Result<u64, DetectError> {
    value
        .trim()
        .parse()
        .map_err(|error| DetectError::Parse(format!("invalid {label}: {error}")))
}

pub(crate) fn parse_usize(label: &str, value: &str) -> Result<usize, DetectError> {
    value
        .trim()
        .parse()
        .map_err(|error| DetectError::Parse(format!("invalid {label}: {error}")))
}

pub(crate) fn parse_u64_value(label: &str, value: &str) -> Result<u64, DetectError> {
    parse_u64(label, value)
}
