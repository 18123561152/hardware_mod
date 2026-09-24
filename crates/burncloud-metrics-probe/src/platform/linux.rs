use std::fs;

use burncloud_node_contracts::AcceleratorKind;

use super::{parse_u64, CpuSnapshot, MetricsError, RawAcceleratorMetrics};

pub(crate) async fn sample_cpu() -> Result<Option<CpuSnapshot>, MetricsError> {
    let content = match tokio::fs::read_to_string("/proc/stat").await {
        Ok(content) => content,
        Err(_) => return Ok(None),
    };
    parse_proc_stat(&content).map(Some)
}

pub(crate) async fn sample_memory_available() -> Result<Option<u64>, MetricsError> {
    let content = match tokio::fs::read_to_string("/proc/meminfo").await {
        Ok(content) => content,
        Err(_) => return Ok(None),
    };
    parse_mem_available(&content)
}

pub(crate) async fn sample_vendor_accelerators() -> Result<Vec<RawAcceleratorMetrics>, MetricsError>
{
    let mut result = Vec::new();
    let entries = match fs::read_dir("/sys/class/drm") {
        Ok(entries) => entries,
        Err(_) => return Ok(result),
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.starts_with("card") || file_name.contains('-') {
            continue;
        }
        let device = entry.path().join("device");
        let vendor = match fs::read_to_string(device.join("vendor")) {
            Ok(value) => value.trim().to_ascii_lowercase(),
            Err(_) => continue,
        };
        let kind = match vendor.as_str() {
            "0x1002" => AcceleratorKind::Amd,
            "0x106b" => AcceleratorKind::Apple,
            _ => continue,
        };
        let name = fs::read_to_string(device.join("product_name"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{kind:?} GPU"));
        let memory_total_bytes = read_optional_u64(&device.join("mem_info_vram_total"))?;
        let memory_used_bytes = read_optional_u64(&device.join("mem_info_vram_used"))?;
        let memory_free_bytes = match (memory_total_bytes, memory_used_bytes) {
            (Some(total), Some(used)) => total.checked_sub(used),
            _ => None,
        };
        let utilization_percent = read_optional_f32(&device.join("gpu_busy_percent"))?;
        result.push(RawAcceleratorMetrics {
            kind,
            index: Some(result.len() as u32),
            name,
            memory_total_bytes,
            memory_used_bytes,
            memory_free_bytes,
            utilization_percent,
        });
    }
    Ok(result)
}

fn read_optional_u64(path: &std::path::Path) -> Result<Option<u64>, MetricsError> {
    let Ok(value) = fs::read_to_string(path) else {
        return Ok(None);
    };
    if value.trim().is_empty() || value.trim() == "0" {
        return Ok(None);
    }
    parse_u64(path.to_string_lossy().as_ref(), value.trim()).map(Some)
}

fn read_optional_f32(path: &std::path::Path) -> Result<Option<f32>, MetricsError> {
    let Ok(value) = fs::read_to_string(path) else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    let parsed = value
        .trim()
        .parse::<f32>()
        .map_err(|error| MetricsError::Parse(format!("invalid GPU utilization: {error}")))?;
    if !(0.0..=100.0).contains(&parsed) {
        return Err(MetricsError::Parse(format!(
            "GPU utilization out of range: {parsed}"
        )));
    }
    Ok(Some(parsed))
}

fn parse_proc_stat(input: &str) -> Result<CpuSnapshot, MetricsError> {
    let line = input
        .lines()
        .find(|line| line.starts_with("cpu "))
        .ok_or_else(|| MetricsError::Parse("/proc/stat has no aggregate CPU row".into()))?;
    let values: Vec<_> = line.split_whitespace().skip(1).collect();
    if values.len() < 4 {
        return Err(MetricsError::Parse(
            "/proc/stat CPU row is incomplete".into(),
        ));
    }
    let parsed: Result<Vec<u64>, _> = values
        .iter()
        .map(|value| parse_u64("CPU jiffy", value))
        .collect();
    let parsed = parsed?;
    let total = parsed.iter().sum();
    let idle = parsed[3].saturating_add(parsed.get(4).copied().unwrap_or(0));
    Ok(CpuSnapshot { total, idle })
}

fn parse_mem_available(input: &str) -> Result<Option<u64>, MetricsError> {
    let Some(value) = input
        .lines()
        .find_map(|line| line.strip_prefix("MemAvailable:"))
    else {
        return Ok(None);
    };
    parse_u64("MemAvailable", value.trim().trim_end_matches(" kB"))?
        .checked_mul(1024)
        .map(Some)
        .ok_or_else(|| MetricsError::Parse("MemAvailable overflow".into()))
}

#[cfg(test)]
mod tests {
    use super::{parse_mem_available, parse_proc_stat};

    #[test]
    fn parses_linux_cpu_and_memory() {
        let cpu = parse_proc_stat("cpu  10 2 3 80 5 0 0 0 0 0\n").unwrap();
        assert_eq!(cpu.total, 100);
        assert_eq!(cpu.idle, 85);
        assert_eq!(
            parse_mem_available("MemTotal: 100 kB\nMemAvailable: 64 kB\n").unwrap(),
            Some(64 * 1024)
        );
    }

    #[test]
    fn rejects_incomplete_linux_values() {
        assert!(parse_proc_stat("cpu 1 2\n").is_err());
        assert_eq!(parse_mem_available("MemTotal: 1 kB\n").unwrap(), None);
    }
}
