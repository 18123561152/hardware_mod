use burncloud_node_contracts::AcceleratorKind;

use super::{parse_u64, CpuSnapshot, MetricsError, RawAcceleratorMetrics};

pub(crate) async fn sample_cpu() -> Result<Option<CpuSnapshot>, MetricsError> {
    let output = match tokio::process::Command::new("sysctl")
        .args(["-n", "kern.cp_time"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return Ok(None),
    };
    let text = String::from_utf8(output.stdout)
        .map_err(|error| MetricsError::Parse(format!("invalid sysctl UTF-8: {error}")))?;
    parse_cp_time(&text).map(Some)
}

pub(crate) async fn sample_memory_available() -> Result<Option<u64>, MetricsError> {
    let output = match tokio::process::Command::new("vm_stat").output().await {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return Ok(None),
    };
    let text = String::from_utf8(output.stdout)
        .map_err(|error| MetricsError::Parse(format!("invalid vm_stat UTF-8: {error}")))?;
    parse_vm_stat_available(&text)
}

pub(crate) async fn sample_vendor_accelerators() -> Result<Vec<RawAcceleratorMetrics>, MetricsError>
{
    let output = match tokio::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return Ok(Vec::new()),
    };
    let text = String::from_utf8(output.stdout)
        .map_err(|error| MetricsError::Parse(format!("invalid system_profiler UTF-8: {error}")))?;
    let utilization = sample_powermetrics_utilization().await?;
    parse_spdisplays(&text, utilization)
}

async fn sample_powermetrics_utilization() -> Result<Option<f32>, MetricsError> {
    let output = match tokio::process::Command::new("powermetrics")
        .args(["-n", "1", "-i", "100", "--samplers", "gpu_power"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return Ok(None),
    };
    let text = String::from_utf8(output.stdout)
        .map_err(|error| MetricsError::Parse(format!("invalid powermetrics UTF-8: {error}")))?;
    parse_gpu_utilization(&text)
}

fn parse_gpu_utilization(input: &str) -> Result<Option<f32>, MetricsError> {
    for line in input.lines() {
        let lower = line.to_ascii_lowercase();
        if !lower.contains("gpu") || !line.contains('%') {
            continue;
        }
        let Some(value) = line
            .split('%')
            .next_back()
            .and_then(|part| part.split_whitespace().last())
        else {
            continue;
        };
        let value =
            value.trim_matches(|character: char| !character.is_ascii_digit() && character != '.');
        if value.is_empty() {
            continue;
        }
        let parsed = value
            .parse::<f32>()
            .map_err(|error| MetricsError::Parse(format!("invalid GPU utilization: {error}")))?;
        if !(0.0..=100.0).contains(&parsed) {
            return Err(MetricsError::Parse(format!(
                "GPU utilization out of range: {parsed}"
            )));
        }
        return Ok(Some(parsed));
    }
    Ok(None)
}

fn parse_memory(value: &str) -> Result<Option<u64>, MetricsError> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") {
        return Ok(None);
    }
    let mut parts = value.split_whitespace();
    let amount = parts
        .next()
        .ok_or_else(|| MetricsError::Parse("GPU memory value is missing".into()))?
        .parse::<u64>()
        .map_err(|error| MetricsError::Parse(format!("invalid GPU memory: {error}")))?;
    let multiplier = match parts.next().unwrap_or("B").to_ascii_lowercase().as_str() {
        "b" => 1,
        "kb" => 1024,
        "mb" => 1024 * 1024,
        "gb" => 1024 * 1024 * 1024,
        unit => {
            return Err(MetricsError::Parse(format!(
                "unsupported GPU memory unit: {unit}"
            )))
        }
    };
    amount
        .checked_mul(multiplier)
        .map(Some)
        .ok_or_else(|| MetricsError::Parse("GPU memory overflow".into()))
}

fn parse_spdisplays(
    input: &str,
    utilization: Option<f32>,
) -> Result<Vec<RawAcceleratorMetrics>, MetricsError> {
    let mut result = Vec::new();
    let mut chipset = None;
    let mut vendor = None;
    let mut memory = None;
    let mut flush = |chipset: &mut Option<String>,
                     vendor: &mut Option<String>,
                     memory: &mut Option<Option<u64>>,
                     result: &mut Vec<RawAcceleratorMetrics>|
     -> Result<(), MetricsError> {
        let name = chipset.take();
        let vendor_value = vendor.take().unwrap_or_default();
        let memory_value = memory.take().unwrap_or(None);
        let Some(name) = name else { return Ok(()) };
        let combined = format!("{name} {vendor_value}").to_ascii_lowercase();
        let kind = if combined.contains("amd") || combined.contains("radeon") {
            Some(AcceleratorKind::Amd)
        } else if combined.contains("apple") {
            Some(AcceleratorKind::Apple)
        } else {
            None
        };
        if let Some(kind) = kind {
            result.push(RawAcceleratorMetrics {
                kind,
                index: Some(result.len() as u32),
                name,
                memory_total_bytes: memory_value,
                memory_used_bytes: None,
                memory_free_bytes: None,
                utilization_percent: utilization,
            });
        }
        Ok(())
    };
    for line in input.lines() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        if indent == 2 && trimmed.ends_with(':') {
            flush(&mut chipset, &mut vendor, &mut memory, &mut result)?;
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("Chipset Model:") {
            chipset = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("Vendor:") {
            vendor = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("VRAM (Total):") {
            memory = Some(parse_memory(value)?);
        }
    }
    flush(&mut chipset, &mut vendor, &mut memory, &mut result)?;
    Ok(result)
}

fn parse_cp_time(input: &str) -> Result<CpuSnapshot, MetricsError> {
    let values: Vec<_> = input.split_whitespace().collect();
    if values.len() < 4 {
        return Err(MetricsError::Parse("kern.cp_time is incomplete".into()));
    }
    let parsed: Result<Vec<u64>, _> = values
        .iter()
        .map(|value| parse_u64("CPU tick", value))
        .collect();
    let parsed = parsed?;
    Ok(CpuSnapshot {
        total: parsed.iter().sum(),
        idle: parsed[3],
    })
}

fn parse_vm_stat_available(input: &str) -> Result<Option<u64>, MetricsError> {
    let page_size = input
        .lines()
        .next()
        .and_then(|line| line.split("page size of ").nth(1))
        .and_then(|value| value.split_whitespace().next())
        .ok_or_else(|| MetricsError::Parse("vm_stat page size is missing".into()))
        .and_then(|value| parse_u64("vm_stat page size", value))?;

    let mut pages = 0_u64;
    let mut found = false;
    for line in input.lines() {
        let is_available = [
            "Pages free:",
            "Pages inactive:",
            "Pages speculative:",
            "Pages purgeable:",
            "File-backed pages:",
        ]
        .iter()
        .any(|prefix| line.trim_start().starts_with(prefix));
        if is_available {
            found = true;
            let value = line
                .split(':')
                .nth(1)
                .ok_or_else(|| MetricsError::Parse("vm_stat row is malformed".into()))?
                .trim()
                .trim_end_matches('.');
            pages = pages
                .checked_add(parse_u64("vm_stat page count", value)?)
                .ok_or_else(|| MetricsError::Parse("vm_stat page count overflow".into()))?;
        }
    }
    if !found {
        return Ok(None);
    }
    pages
        .checked_mul(page_size)
        .map(Some)
        .ok_or_else(|| MetricsError::Parse("vm_stat available bytes overflow".into()))
}

#[cfg(test)]
mod tests {
    use super::{parse_cp_time, parse_gpu_utilization, parse_spdisplays, parse_vm_stat_available};
    use burncloud_node_contracts::AcceleratorKind;

    #[test]
    fn parses_macos_cpu_and_vm_stat() {
        let cpu = parse_cp_time("10 2 3 80 5").unwrap();
        assert_eq!(cpu.total, 100);
        assert_eq!(cpu.idle, 80);
        let memory = parse_vm_stat_available(
            "Mach Virtual Memory Statistics: (page size of 4096 bytes)\nPages free: 2.\nPages inactive: 3.\n",
        )
        .unwrap();
        assert_eq!(memory, Some(5 * 4096));
    }

    #[test]
    fn rejects_malformed_macos_output() {
        assert!(parse_cp_time("10 2").is_err());
        assert_eq!(parse_vm_stat_available("Pages free: 2.\n").unwrap(), None);
    }

    #[test]
    fn parses_optional_apple_gpu_metrics() {
        let output = "  Apple M2:\n    Chipset Model: Apple M2\n    Vendor: Apple (0x106b)\n    VRAM (Total): 8 GB\n";
        let values = parse_spdisplays(output, Some(22.0)).unwrap();
        assert_eq!(values[0].kind, AcceleratorKind::Apple);
        assert_eq!(values[0].memory_total_bytes, Some(8 * 1024 * 1024 * 1024));
        assert_eq!(values[0].utilization_percent, Some(22.0));
        assert_eq!(
            parse_gpu_utilization("GPU Busy: 15.5%\n").unwrap(),
            Some(15.5)
        );
    }
}
