use crate::platform::{DetectError, RawAccelerator};

pub(crate) fn parse_nvidia_smi_csv(input: &str) -> Result<Vec<RawAccelerator>, DetectError> {
    let mut accelerators = Vec::new();
    for (index, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if fields.len() != 2 {
            return Err(DetectError::Parse(format!(
                "nvidia-smi row {} has {} fields; expected 2",
                index + 1,
                fields.len()
            )));
        }

        let memory_bytes = if fields[1].is_empty() || fields[1].eq_ignore_ascii_case("N/A") {
            None
        } else {
            Some(
                fields[1]
                    .parse::<u64>()
                    .map_err(|error| {
                        DetectError::Parse(format!(
                            "invalid NVIDIA memory in row {}: {error}",
                            index + 1
                        ))
                    })?
                    .checked_mul(1024 * 1024)
                    .ok_or_else(|| {
                        DetectError::Parse(format!("NVIDIA memory overflow in row {}", index + 1))
                    })?,
            )
        };

        accelerators.push(RawAccelerator {
            kind: burncloud_node_contracts::AcceleratorKind::Nvidia,
            name: fields[0].to_string(),
            memory_bytes,
        });
    }
    Ok(accelerators)
}

#[cfg(test)]
mod tests {
    use super::parse_nvidia_smi_csv;

    #[test]
    fn parses_multiple_devices() {
        let devices = parse_nvidia_smi_csv("RTX 5090, 32607\nTesla, 81559\n").unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "RTX 5090");
        assert_eq!(devices[0].memory_bytes, Some(32607 * 1024 * 1024));
    }

    #[test]
    fn accepts_empty_input_and_unmeasurable_memory() {
        assert!(parse_nvidia_smi_csv("").unwrap().is_empty());
        assert_eq!(
            parse_nvidia_smi_csv("GPU, N/A").unwrap()[0].memory_bytes,
            None
        );
    }

    #[test]
    fn rejects_missing_fields_and_invalid_memory() {
        assert!(parse_nvidia_smi_csv("GPU").is_err());
        assert!(parse_nvidia_smi_csv("GPU, nope").is_err());
    }
}
