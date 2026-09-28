use burncloud_node_runtime::AcceleratorKind;

use crate::platform::{MetricsError, RawAcceleratorMetrics};
use tokio::process::Command;

pub(crate) async fn sample_nvidia() -> Result<Vec<RawAcceleratorMetrics>, MetricsError> {
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,name,memory.total,memory.used,memory.free,utilization.gpu",
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
        .map_err(|error| MetricsError::Parse(format!("invalid nvidia-smi UTF-8: {error}")))?;
    parse_nvidia_smi_csv(&stdout)
}

pub(crate) fn parse_nvidia_smi_csv(
    input: &str,
) -> Result<Vec<RawAcceleratorMetrics>, MetricsError> {
    let mut metrics = Vec::new();
    for (line_index, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if fields.len() != 6 {
            return Err(MetricsError::Parse(format!(
                "nvidia-smi row {} has {} fields; expected 6",
                line_index + 1,
                fields.len()
            )));
        }

        metrics.push(RawAcceleratorMetrics {
            kind: AcceleratorKind::Nvidia,
            index: parse_optional_u32(fields[0], "GPU index", line_index + 1)?,
            name: fields[1].to_string(),
            memory_total_bytes: parse_optional_megabytes(
                fields[2],
                "total memory",
                line_index + 1,
            )?,
            memory_used_bytes: parse_optional_megabytes(fields[3], "used memory", line_index + 1)?,
            memory_free_bytes: parse_optional_megabytes(fields[4], "free memory", line_index + 1)?,
            utilization_percent: parse_optional_percent(
                fields[5],
                "GPU utilization",
                line_index + 1,
            )?,
        });
    }
    Ok(metrics)
}

fn is_unavailable(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("n/a") || value.eq_ignore_ascii_case("na")
}

fn parse_optional_u32(value: &str, label: &str, row: usize) -> Result<Option<u32>, MetricsError> {
    if is_unavailable(value) {
        return Ok(None);
    }
    value
        .parse::<u32>()
        .map(Some)
        .map_err(|error| MetricsError::Parse(format!("invalid {label} in row {row}: {error}")))
}

fn parse_optional_megabytes(
    value: &str,
    label: &str,
    row: usize,
) -> Result<Option<u64>, MetricsError> {
    if is_unavailable(value) {
        return Ok(None);
    }
    let megabytes = value
        .parse::<u64>()
        .map_err(|error| MetricsError::Parse(format!("invalid {label} in row {row}: {error}")))?;
    megabytes
        .checked_mul(1024 * 1024)
        .map(Some)
        .ok_or_else(|| MetricsError::Parse(format!("{label} overflow in row {row}")))
}

fn parse_optional_percent(
    value: &str,
    label: &str,
    row: usize,
) -> Result<Option<f32>, MetricsError> {
    if is_unavailable(value) {
        return Ok(None);
    }
    let percent = value
        .parse::<f32>()
        .map_err(|error| MetricsError::Parse(format!("invalid {label} in row {row}: {error}")))?;
    if !(0.0..=100.0).contains(&percent) {
        return Err(MetricsError::Parse(format!(
            "{label} out of range in row {row}: {percent}"
        )));
    }
    Ok(Some(percent))
}

#[cfg(test)]
mod tests {
    use super::parse_nvidia_smi_csv;

    #[test]
    fn parses_all_dynamic_gpu_fields() {
        let parsed = parse_nvidia_smi_csv("0, RTX 5090, 24576, 1024, 23552, 7").unwrap();
        assert_eq!(parsed[0].index, Some(0));
        assert_eq!(parsed[0].memory_total_bytes, Some(24576 * 1024 * 1024));
        assert_eq!(parsed[0].memory_used_bytes, Some(1024 * 1024 * 1024));
        assert_eq!(parsed[0].utilization_percent, Some(7.0));
    }

    #[test]
    fn preserves_none_for_unavailable_metrics() {
        let parsed = parse_nvidia_smi_csv("N/A, GPU, N/A, N/A, N/A, N/A").unwrap();
        assert_eq!(parsed[0].index, None);
        assert_eq!(parsed[0].memory_total_bytes, None);
        assert_eq!(parsed[0].utilization_percent, None);
    }

    #[test]
    fn rejects_malformed_rows_and_invalid_values() {
        assert!(parse_nvidia_smi_csv("0,GPU,1,2,3").is_err());
        assert!(parse_nvidia_smi_csv("0,GPU,nope,2,3,4").is_err());
        assert!(parse_nvidia_smi_csv("0,GPU,1,2,3,101").is_err());
    }
}
