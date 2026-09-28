use burncloud_node_runtime::AcceleratorKind;

use super::{parse_f64, parse_u64, CpuSnapshot, MetricsError, RawAcceleratorMetrics};

const GPU_QUERY: &str = "Get-CimInstance Win32_VideoController | ForEach-Object { \"$($_.Name)|$($_.AdapterCompatibility)|$($_.AdapterRAM)\" }";
const GPU_COUNTER_QUERY: &str = "Get-Counter '\\GPU Engine(*)\\Utilization Percentage' -ErrorAction SilentlyContinue | ForEach-Object { \"$($_.InstanceName)|$($_.CookedValue)\" }";

async fn powershell(expression: &str) -> Result<Option<String>, MetricsError> {
    let output = match tokio::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", expression])
        .output()
        .await
    {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return Ok(None),
    };
    String::from_utf8(output.stdout)
        .map(Some)
        .map_err(|error| MetricsError::Parse(format!("invalid PowerShell UTF-8: {error}")))
}

pub(crate) async fn sample_cpu() -> Result<Option<CpuSnapshot>, MetricsError> {
    let output = powershell(
        "$p=Get-CimInstance Win32_PerfRawData_PerfOS_Processor -Filter \"Name='_Total'\"; \"$($p.PercentProcessorTime),$($p.PercentIdleTime)\"",
    )
    .await?;
    match output {
        Some(value) => parse_cpu_counters(&value),
        None => Ok(None),
    }
}

pub(crate) async fn sample_memory_available() -> Result<Option<u64>, MetricsError> {
    let output = powershell("(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory").await?;
    match output {
        Some(value) => Ok(Some(
            parse_u64("FreePhysicalMemory", &value)?
                .checked_mul(1024)
                .ok_or_else(|| MetricsError::Parse("FreePhysicalMemory overflow".into()))?,
        )),
        None => Ok(None),
    }
}

pub(crate) async fn sample_vendor_accelerators() -> Result<Vec<RawAcceleratorMetrics>, MetricsError>
{
    let adapters = powershell(GPU_QUERY).await?.unwrap_or_default();
    let counters = powershell(GPU_COUNTER_QUERY).await?.unwrap_or_default();
    parse_gpu_adapters(&adapters, &counters)
}

fn parse_cpu_counters(input: &str) -> Result<Option<CpuSnapshot>, MetricsError> {
    let values: Vec<_> = input.trim().split(',').collect();
    if values.len() != 2 {
        return Err(MetricsError::Parse(
            "Windows CPU counter output is malformed".into(),
        ));
    }
    let processor = parse_f64("PercentProcessorTime", values[0])?;
    let idle = parse_f64("PercentIdleTime", values[1])?;
    if processor < 0.0 || idle < 0.0 {
        return Ok(None);
    }
    Ok(Some(CpuSnapshot {
        total: processor as u64 + idle as u64,
        idle: idle as u64,
    }))
}

fn parse_gpu_adapters(
    input: &str,
    counters: &str,
) -> Result<Vec<RawAcceleratorMetrics>, MetricsError> {
    let mut result = Vec::new();
    for (row, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<_> = line.splitn(3, '|').map(str::trim).collect();
        if fields.len() != 3 {
            return Err(MetricsError::Parse(format!(
                "Windows GPU row {} is malformed",
                row + 1
            )));
        }
        let combined = format!("{} {}", fields[0], fields[1]).to_ascii_lowercase();
        let kind = if combined.contains("nvidia") {
            continue;
        } else if combined.contains("amd")
            || combined.contains("radeon")
            || combined.contains("advanced micro devices")
        {
            AcceleratorKind::Amd
        } else if combined.contains("apple") {
            AcceleratorKind::Apple
        } else {
            continue;
        };
        let memory_total_bytes = if fields[2].is_empty() || fields[2] == "0" {
            None
        } else {
            Some(parse_u64("GPU memory bytes", fields[2])?)
        };
        let utilization_percent = find_matching_counter(fields[0], counters)?;
        result.push(RawAcceleratorMetrics {
            kind,
            index: Some(result.len() as u32),
            name: if fields[0].is_empty() {
                format!("{kind:?} GPU")
            } else {
                fields[0].into()
            },
            memory_total_bytes,
            memory_used_bytes: None,
            memory_free_bytes: None,
            utilization_percent,
        });
    }
    Ok(result)
}

fn find_matching_counter(name: &str, counters: &str) -> Result<Option<f32>, MetricsError> {
    let name = name.to_ascii_lowercase();
    for line in counters.lines() {
        let Some((instance, value)) = line.split_once('|') else {
            continue;
        };
        if !instance.to_ascii_lowercase().contains(&name) {
            continue;
        }
        let value = parse_f64("GPU utilization", value)?;
        if !(0.0..=100.0).contains(&value) {
            return Err(MetricsError::Parse(format!(
                "GPU utilization out of range: {value}"
            )));
        }
        return Ok(Some(value as f32));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{parse_cpu_counters, parse_gpu_adapters, sample_memory_available};
    use burncloud_node_runtime::AcceleratorKind;

    #[test]
    fn parses_windows_cpu_counter_output() {
        let snapshot = parse_cpu_counters("1000,250").unwrap().unwrap();
        assert_eq!(snapshot.total, 1250);
        assert_eq!(snapshot.idle, 250);
    }

    #[test]
    fn malformed_windows_cpu_output_is_rejected() {
        assert!(parse_cpu_counters("1000").is_err());
    }

    #[test]
    fn parses_optional_amd_gpu_metrics() {
        let values = parse_gpu_adapters(
            "Radeon RX 7900|AMD|17179869184\n",
            "Radeon RX 7900 3D|37.5\n",
        )
        .unwrap();
        assert_eq!(values[0].kind, AcceleratorKind::Amd);
        assert_eq!(values[0].memory_total_bytes, Some(17179869184));
        assert_eq!(values[0].utilization_percent, Some(37.5));
    }

    #[allow(dead_code)]
    async fn keeps_memory_collection_platform_local() {
        let _ = sample_memory_available().await;
    }
}
