# Public API

## Canonical Static Hardware Contract

`burncloud-hardware-probe` does not define or re-export a second hardware
contract. `RealHardwareProbe` directly implements these types from the pinned
`burncloud-node-runtime` dependency:

```rust
use burncloud_node_runtime::{
    AcceleratorKind, AcceleratorProfile, HardwareProbe, HardwareProbeError,
    HardwareProfile,
};
```

```rust
#[async_trait]
pub trait HardwareProbe: Send + Sync {
    async fn inspect(&self) -> Result<HardwareProfile, HardwareProbeError>;
}

pub struct HardwareProfile {
    pub cpu_threads: usize,
    pub memory_bytes: u64,
    pub disk_available_bytes: u64,
    pub accelerators: Vec<AcceleratorProfile>,
}

pub struct AcceleratorProfile {
    pub kind: AcceleratorKind,
    pub name: String,
    pub memory_bytes: Option<u64>,
}
```

`HardwareProfile` has no CPU brand or CPU model fields. Those values may be
collected privately by a platform detector, but need a separate BurnCloud
contract-change review before they can become public output.

### `RealHardwareProbe`

```rust
pub struct RealHardwareProbe;

impl RealHardwareProbe {
    pub fn new() -> Self;
}

impl Default for RealHardwareProbe;
impl burncloud_node_runtime::HardwareProbe for RealHardwareProbe;
```

The probe reports static machine facts only. NVIDIA devices are inspected with
`nvidia-smi`; when platform discovery confirms NVIDIA hardware but the command
cannot inspect it, `inspect()` returns the canonical
`HardwareProbeError::DetectionFailed` instead of fabricating an empty list.

## Experimental Metrics API

`burncloud-metrics-probe` exposes a local research API that is not part of the
accepted BurnCloud contract:

```rust
pub trait RuntimeMetricsProbe: Send + Sync {
    async fn sample(&self) -> Result<RuntimeMetrics, MetricsProbeError>;
}

pub struct RuntimeMetrics {
    pub cpu_usage_percent: Option<f32>,
    pub memory_available_bytes: Option<u64>,
    pub accelerators: Vec<AcceleratorMetrics>,
}

pub struct AcceleratorMetrics {
    pub kind: burncloud_node_runtime::AcceleratorKind,
    pub index: Option<u32>,
    pub name: String,
    pub memory_total_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_free_bytes: Option<u64>,
    pub utilization_percent: Option<f32>,
}
```

`RealMetricsProbe` takes a fresh sample on every call. Unavailable values are
represented by `None`; it never fabricates zero values. This API requires a
separate BurnCloud contract review before any integration.

## Private Details

Platform adapters, raw structs, command execution, parser functions, internal
errors, CPU identity cache, and vendor-specific tooling are intentionally not
public API.
