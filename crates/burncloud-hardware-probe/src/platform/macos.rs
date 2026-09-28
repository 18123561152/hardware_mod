use super::{
    parse_u64_value, parse_usize, run_command, DetectError, RawAccelerator, RawCpuIdentity,
};

pub(crate) async fn collect_cpu() -> Result<RawCpuIdentity, DetectError> {
    let threads = run_command("sysctl", &["-n", "hw.ncpu"]).await?;
    let brand = run_command("sysctl", &["-n", "machdep.cpu.brand_string"]).await?;
    let brand_text = String::from_utf8_lossy(&brand.stdout);
    let (cpu_brand, cpu_model) = parse_cpu_brand_string(&brand_text);
    Ok(RawCpuIdentity {
        cpu_threads: parse_usize(
            "CPU thread count",
            &String::from_utf8_lossy(&threads.stdout),
        )?,
        cpu_brand,
        cpu_model,
    })
}

pub(crate) async fn collect_memory() -> Result<u64, DetectError> {
    let memory = run_command("sysctl", &["-n", "hw.memsize"]).await?;
    parse_u64_value("memory bytes", &String::from_utf8_lossy(&memory.stdout))
}

pub(crate) async fn collect_disk() -> Result<u64, DetectError> {
    let disk = run_command("df", &["-k", "/"]).await?;
    let disk_available_kib = String::from_utf8_lossy(&disk.stdout)
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().nth(3))
        .ok_or_else(|| DetectError::Parse("df output is missing available kibibytes".into()))
        .and_then(|value| parse_u64_value("disk available kibibytes", value))?;
    disk_available_kib
        .checked_mul(1024)
        .ok_or_else(|| DetectError::Parse("disk available bytes overflow".into()))
}

pub(crate) async fn collect_vendor_accelerators() -> Result<Vec<RawAccelerator>, DetectError> {
    parse_spdisplays(&system_profiler_displays().await?)
}

pub(crate) async fn has_nvidia_hardware() -> Result<bool, DetectError> {
    Ok(system_profiler_displays()
        .await?
        .to_ascii_lowercase()
        .contains("nvidia"))
}

async fn system_profiler_displays() -> Result<String, DetectError> {
    let output = tokio::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .await
        .map_err(|error| DetectError::Command {
            program: "system_profiler".into(),
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(DetectError::Command {
            program: "system_profiler".into(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    String::from_utf8(output.stdout)
        .map_err(|error| DetectError::Parse(format!("invalid system_profiler UTF-8: {error}")))
}

pub(crate) fn parse_cpu_brand_string(raw: &str) -> (Option<String>, Option<String>) {
    let value = raw.trim();
    if value.is_empty() {
        return (None, None);
    }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("apple ") {
        return (Some("Apple".into()), Some(value[6..].trim().to_string()));
    }
    if lower.contains("intel") {
        let model = value
            .split_whitespace()
            .find(|part| {
                part.starts_with('i')
                    && part.chars().skip(1).all(|c| c.is_ascii_digit() || c == '-')
            })
            .map(str::to_string);
        return (Some("Intel".into()), model);
    }
    if lower.contains("amd") {
        return (Some("AMD".into()), Some(value.to_string()));
    }
    (Some("Unknown".into()), Some(value.to_string()))
}

fn parse_memory_bytes(value: &str) -> Result<Option<u64>, DetectError> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") {
        return Ok(None);
    }
    let mut parts = value.split_whitespace();
    let amount = parts
        .next()
        .ok_or_else(|| DetectError::Parse("GPU memory value is missing".into()))?
        .parse::<u64>()
        .map_err(|error| DetectError::Parse(format!("invalid GPU memory: {error}")))?;
    let unit = parts.next().unwrap_or("B").to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "b" => 1,
        "kb" => 1024,
        "mb" => 1024 * 1024,
        "gb" => 1024 * 1024 * 1024,
        _ => {
            return Err(DetectError::Parse(format!(
                "unsupported GPU memory unit: {unit}"
            )))
        }
    };
    amount
        .checked_mul(multiplier)
        .map(Some)
        .ok_or_else(|| DetectError::Parse("GPU memory overflow".into()))
}

fn parse_spdisplays(input: &str) -> Result<Vec<RawAccelerator>, DetectError> {
    let mut result = Vec::new();
    let mut chipset = None;
    let mut vendor = None;
    let mut memory = None;
    let mut flush = |chipset: &mut Option<String>,
                     vendor: &mut Option<String>,
                     memory: &mut Option<Option<u64>>,
                     result: &mut Vec<RawAccelerator>|
     -> Result<(), DetectError> {
        let name = chipset.take();
        let vendor_value = vendor.take().unwrap_or_default();
        let memory_value = memory.take().unwrap_or(None);
        let Some(name) = name else { return Ok(()) };
        let combined = format!("{name} {vendor_value}").to_ascii_lowercase();
        let kind = if combined.contains("amd") || combined.contains("radeon") {
            Some(burncloud_node_runtime::AcceleratorKind::Amd)
        } else if combined.contains("apple") {
            Some(burncloud_node_runtime::AcceleratorKind::Apple)
        } else {
            None
        };
        if let Some(kind) = kind {
            result.push(RawAccelerator {
                kind,
                name,
                memory_bytes: memory_value,
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
            memory = Some(parse_memory_bytes(value)?);
        }
    }
    flush(&mut chipset, &mut vendor, &mut memory, &mut result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{parse_cpu_brand_string, parse_spdisplays};
    use burncloud_node_runtime::AcceleratorKind;

    #[test]
    fn parses_apple_silicon_and_intel_cpu_names() {
        assert_eq!(
            parse_cpu_brand_string("Apple M1 Pro"),
            (Some("Apple".into()), Some("M1 Pro".into()))
        );
        assert_eq!(
            parse_cpu_brand_string("Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz"),
            (Some("Intel".into()), Some("i7-9750H".into()))
        );
    }

    #[test]
    fn parses_vendor_display_blocks() {
        let output = "  Apple M2:\n    Chipset Model: Apple M2\n    Vendor: Apple (0x106b)\n  AMD Radeon Pro:\n    Chipset Model: AMD Radeon Pro\n    Vendor: AMD (0x1002)\n    VRAM (Total): 8 GB\n";
        let values = parse_spdisplays(output).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].kind, AcceleratorKind::Apple);
        assert_eq!(values[1].kind, AcceleratorKind::Amd);
        assert_eq!(values[1].memory_bytes, Some(8 * 1024 * 1024 * 1024));
    }
}
