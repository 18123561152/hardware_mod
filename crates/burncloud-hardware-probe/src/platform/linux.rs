use std::fs;

use super::{
    parse_u64_value, parse_usize, run_command, DetectError, RawAccelerator, RawCpuIdentity,
};

pub(crate) async fn collect_cpu() -> Result<RawCpuIdentity, DetectError> {
    let cpu = run_command("nproc", &[]).await?;
    let cpu_threads = parse_usize("CPU thread count", &String::from_utf8_lossy(&cpu.stdout))?;
    let info = run_command("cat", &["/proc/cpuinfo"]).await?;
    let (cpu_brand, cpu_model) = parse_cpuinfo(&String::from_utf8_lossy(&info.stdout));
    Ok(RawCpuIdentity {
        cpu_threads,
        cpu_brand,
        cpu_model,
    })
}

pub(crate) async fn collect_memory() -> Result<u64, DetectError> {
    let memory = run_command("cat", &["/proc/meminfo"]).await?;
    let memory_text = String::from_utf8_lossy(&memory.stdout);
    memory_text
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))
        .ok_or_else(|| DetectError::Parse("MemTotal is missing".into()))
        .and_then(|value| parse_u64_value("MemTotal", value.trim().trim_end_matches(" kB")))?
        .checked_mul(1024)
        .ok_or_else(|| DetectError::Parse("MemTotal overflow".into()))
}

pub(crate) async fn collect_disk() -> Result<u64, DetectError> {
    let disk = run_command("df", &["-B1", "/"]).await?;
    parse_df_available(&String::from_utf8_lossy(&disk.stdout))
}

pub(crate) async fn collect_vendor_accelerators() -> Result<Vec<RawAccelerator>, DetectError> {
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
            "0x1002" => burncloud_node_contracts::AcceleratorKind::Amd,
            "0x106b" => burncloud_node_contracts::AcceleratorKind::Apple,
            _ => continue,
        };
        let product = fs::read_to_string(device.join("product_name"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let memory_bytes = match fs::read_to_string(device.join("mem_info_vram_total")) {
            Ok(value) if !value.trim().is_empty() && value.trim() != "0" => {
                Some(parse_u64_value("VRAM total", value.trim())?)
            }
            _ => None,
        };
        result.push(RawAccelerator {
            kind,
            name: product.unwrap_or_else(|| format!("{kind:?} GPU")),
            memory_bytes,
        });
    }
    Ok(result)
}

fn parse_cpuinfo(input: &str) -> (Option<String>, Option<String>) {
    let mut vendor = None;
    let mut model = None;
    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "vendor_id" | "CPU implementer" if vendor.is_none() => {
                vendor = Some(value.trim().to_string());
            }
            "model name" | "Model" | "Hardware" if model.is_none() => {
                model = Some(value.trim().to_string());
            }
            _ => {}
        }
    }
    (
        vendor.filter(|value| !value.is_empty()),
        model.filter(|value| !value.is_empty()),
    )
}

fn parse_df_available(output: &str) -> Result<u64, DetectError> {
    output
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().nth(3))
        .ok_or_else(|| DetectError::Parse("df output is missing available bytes".into()))
        .and_then(|value| parse_u64_value("disk available bytes", value))
}

#[cfg(test)]
mod tests {
    use super::{parse_cpuinfo, parse_df_available};

    #[test]
    fn parses_linux_cpu_and_disk() {
        let (vendor, model) = parse_cpuinfo("vendor_id : AuthenticAMD\nmodel name : AMD Ryzen 9\n");
        assert_eq!(vendor.as_deref(), Some("AuthenticAMD"));
        assert_eq!(model.as_deref(), Some("AMD Ryzen 9"));
        assert_eq!(
            parse_df_available(
                "Filesystem 1B-blocks Used Available Use% Mounted\n/dev 10 2 8 20% /\n"
            )
            .unwrap(),
            8
        );
    }

    #[test]
    fn returns_none_for_missing_cpu_fields() {
        assert_eq!(parse_cpuinfo("processor : 0\n"), (None, None));
    }
}
