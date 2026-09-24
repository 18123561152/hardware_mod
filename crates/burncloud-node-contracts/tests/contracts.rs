use async_trait::async_trait;
use burncloud_node_contracts::{HardwareProbe, HardwareProbeError, HardwareProfile};

struct FakeProbe;

#[async_trait]
impl HardwareProbe for FakeProbe {
    async fn inspect(&self) -> Result<HardwareProfile, HardwareProbeError> {
        Ok(HardwareProfile {
            cpu_threads: 1,
            cpu_brand: Some("Intel".into()),
            cpu_model: Some("test CPU".into()),
            memory_bytes: 2,
            disk_available_bytes: 3,
            accelerators: Vec::new(),
        })
    }
}

#[tokio::test]
async fn external_crate_can_implement_the_contract() {
    let profile = FakeProbe.inspect().await.expect("fake probe succeeds");
    assert_eq!(profile.cpu_threads, 1);
    assert_eq!(profile.cpu_brand.as_deref(), Some("Intel"));
    assert_eq!(profile.cpu_model.as_deref(), Some("test CPU"));
}

#[test]
fn hardware_profile_default_has_optional_identity_fields() {
    let profile = HardwareProfile::default();
    assert_eq!(profile.cpu_brand, None);
    assert_eq!(profile.cpu_model, None);
}

#[test]
fn detection_error_is_constructible_and_displayable() {
    let error = HardwareProbeError::DetectionFailed("permission denied".into());
    assert_eq!(
        error.to_string(),
        "hardware detection failed: permission denied"
    );
}
