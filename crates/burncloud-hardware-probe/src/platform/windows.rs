use super::{
    parse_u64_value, parse_usize, run_command, DetectError, RawAccelerator, RawCpuIdentity,
};

const CPU_QUERY: &str = "Get-CimInstance Win32_Processor | Select-Object -First 1 Manufacturer,Name,NumberOfLogicalProcessors | ForEach-Object { \"$($_.Manufacturer)|$($_.Name)|$($_.NumberOfLogicalProcessors)\" }";
const MEMORY_QUERY: &str = "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory";
const DISK_QUERY: &str = "(Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='C:'\").FreeSpace";
const GPU_QUERY: &str = "Get-CimInstance Win32_VideoController | ForEach-Object { \"$($_.Name)|$($_.AdapterCompatibility)|$($_.AdapterRAM)\" }";

async fn powershell(expression: &str) -> Result<String, DetectError> {
    let output = run_command("powershell", &["-NoProfile", "-Command", expression]).await?;
    String::from_utf8(output.stdout)
        .map_err(|error| DetectError::Parse(format!("invalid PowerShell UTF-8: {error}")))
}

pub(crate) async fn collect_cpu() -> Result<RawCpuIdentity, DetectError> {
    parse_cpu_output(&powershell(CPU_QUERY).await?)
}

pub(crate) async fn collect_memory() -> Result<u64, DetectError> {
    parse_u64_value("memory bytes", &powershell(MEMORY_QUERY).await?)
}

pub(crate) async fn collect_disk() -> Result<u64, DetectError> {
    parse_u64_value("disk available bytes", &powershell(DISK_QUERY).await?)
}

pub(crate) async fn collect_vendor_accelerators() -> Result<Vec<RawAccelerator>, DetectError> {
    parse_video_controllers(&powershell(GPU_QUERY).await?)
}

pub(crate) async fn has_nvidia_hardware() -> Result<bool, DetectError> {
    parse_has_nvidia(&powershell(GPU_QUERY).await?)
}

fn parse_has_nvidia(input: &str) -> Result<bool, DetectError> {
    for (row, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<_> = line.splitn(3, '|').map(str::trim).collect();
        if fields.len() != 3 {
            return Err(DetectError::Parse(format!(
                "Windows GPU row {} is malformed",
                row + 1
            )));
        }
        if format!("{} {}", fields[0], fields[1])
            .to_ascii_lowercase()
            .contains("nvidia")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_cpu_output(input: &str) -> Result<RawCpuIdentity, DetectError> {
    let fields: Vec<_> = input.trim().splitn(3, '|').map(str::trim).collect();
    if fields.len() != 3 {
        return Err(DetectError::Parse("Windows CPU output is malformed".into()));
    }
    Ok(RawCpuIdentity {
        cpu_threads: parse_usize("CPU thread count", fields[2])?,
        cpu_brand: (!fields[0].is_empty()).then(|| fields[0].to_string()),
        cpu_model: (!fields[1].is_empty()).then(|| fields[1].to_string()),
    })
}

fn classify_gpu(
    name: &str,
    compatibility: &str,
) -> Option<burncloud_node_runtime::AcceleratorKind> {
    let value = format!("{name} {compatibility}").to_ascii_lowercase();
    if value.contains("nvidia") {
        None
    } else if value.contains("amd")
        || value.contains("advanced micro devices")
        || value.contains("radeon")
    {
        Some(burncloud_node_runtime::AcceleratorKind::Amd)
    } else if value.contains("apple") {
        Some(burncloud_node_runtime::AcceleratorKind::Apple)
    } else {
        None
    }
}

fn parse_video_controllers(input: &str) -> Result<Vec<RawAccelerator>, DetectError> {
    let mut result = Vec::new();
    for (row, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<_> = line.splitn(3, '|').map(str::trim).collect();
        if fields.len() != 3 {
            return Err(DetectError::Parse(format!(
                "Windows GPU row {} is malformed",
                row + 1
            )));
        }
        let Some(kind) = classify_gpu(fields[0], fields[1]) else {
            continue;
        };
        let memory_bytes = if fields[2].is_empty() || fields[2] == "0" {
            None
        } else {
            Some(parse_u64_value("GPU memory bytes", fields[2])?)
        };
        result.push(RawAccelerator {
            kind,
            name: if fields[0].is_empty() {
                format!("{kind:?} GPU")
            } else {
                fields[0].to_string()
            },
            memory_bytes,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_cpu_output, parse_has_nvidia, parse_video_controllers, CPU_QUERY, DISK_QUERY,
        MEMORY_QUERY,
    };
    use burncloud_node_runtime::AcceleratorKind;

    #[test]
    fn uses_cim_and_never_wmic_for_windows_detection() {
        assert!(CPU_QUERY.contains("Get-CimInstance"));
        assert!(MEMORY_QUERY.contains("Get-CimInstance"));
        assert!(DISK_QUERY.contains("Get-CimInstance"));
        assert!(!CPU_QUERY.to_ascii_lowercase().contains("wmic"));
    }

    #[test]
    fn parses_cpu_and_vendor_gpu_rows() {
        let cpu = parse_cpu_output("AuthenticAMD|AMD Ryzen 9 7950X|32").unwrap();
        assert_eq!(cpu.cpu_brand.as_deref(), Some("AuthenticAMD"));
        assert_eq!(cpu.cpu_threads, 32);
        let gpus = parse_video_controllers("Radeon RX 7900|AMD|17179869184\nNVIDIA RTX|NVIDIA|0\n")
            .unwrap();
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].kind, AcceleratorKind::Amd);
        assert_eq!(gpus[0].memory_bytes, Some(17179869184));
    }

    #[test]
    fn preserves_unmeasurable_gpu_memory() {
        let gpus = parse_video_controllers("Apple M2|Apple|0\n").unwrap();
        assert_eq!(gpus[0].kind, AcceleratorKind::Apple);
        assert_eq!(gpus[0].memory_bytes, None);
    }

    #[test]
    fn identifies_nvidia_hardware_separately_from_vendor_collection() {
        assert!(parse_has_nvidia("NVIDIA RTX|NVIDIA|0\n").unwrap());
        assert!(!parse_has_nvidia("Radeon|AMD|0\n").unwrap());
    }
}
