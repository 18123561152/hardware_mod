# Public API

This document describes the public Rust API exposed by the independent
hardware probe workspace.

## `burncloud-node-contracts`

### `HardwareProbe`

The single public contract for inspecting local machine facts.

```rust
#[async_trait]
pub trait HardwareProbe: Send + Sync {
    async fn inspect(&self) -> Result<HardwareProfile, HardwareProbeError>;
}
```

### `HardwareProfile`

```rust
pub struct HardwareProfile {
    pub cpu_threads: usize,
    pub cpu_brand: Option<String>,
    pub cpu_model: Option<String>,
    pub memory_bytes: u64,
    pub disk_available_bytes: u64,
    pub accelerators: Vec<AcceleratorProfile>,
}
```

`cpu_brand` is normalized to `Intel`, `AMD`, `Apple`, or `Unknown` when
available. Both CPU identity fields are optional and remain `None` when the
platform cannot measure them. `HardwareProfile` also implements `Default`.

All sizes are measured in bytes. These are machine facts only; they do not
express resource requirements or runtime compatibility decisions.

### `AcceleratorProfile`

```rust
pub struct AcceleratorProfile {
    pub kind: AcceleratorKind,
    pub name: String,
    pub memory_bytes: Option<u64>,
}
```

`memory_bytes` is `None` when accelerator memory cannot be measured. AMD and
Apple devices are collected on a best-effort basis on Windows, Linux, and
macOS; NVIDIA devices continue to use `nvidia-smi`.

### `AcceleratorKind`

```rust
pub enum AcceleratorKind {
    Nvidia,
    Amd,
    Apple,
    Other,
}
```

### `HardwareProbeError`

```rust
pub enum HardwareProbeError {
    DetectionFailed(String),
}
```

All public inspection failures are reported through this error type.

## `burncloud-hardware-probe`

### `RealHardwareProbe`

The production implementation of
`burncloud_node_contracts::HardwareProbe`.

```rust
pub struct RealHardwareProbe;

impl RealHardwareProbe {
    pub fn new() -> Self;
}

impl Default for RealHardwareProbe;
impl HardwareProbe for RealHardwareProbe;
```

Example:

```rust
use burncloud_hardware_probe::RealHardwareProbe;
use burncloud_node_contracts::HardwareProbe;

let probe = RealHardwareProbe::new();
let profile = probe.inspect().await?;
```

## Non-public implementation details

The following are intentionally not public API: platform adapters, command
execution, NVIDIA CSV parsing, raw probe types, internal detection errors, and
static-fact caching. Consumers must depend only on `HardwareProbe` and its
contract types.

## `burncloud-node-contracts::metrics`

Dynamic runtime metrics are deliberately separate from `HardwareProbe` and
`HardwareProfile`.

### `RuntimeMetricsProbe`

```rust
#[async_trait]
pub trait RuntimeMetricsProbe: Send + Sync {
    async fn sample(&self) -> Result<RuntimeMetrics, MetricsProbeError>;
}
```

### `RuntimeMetrics`

```rust
pub struct RuntimeMetrics {
    pub cpu_usage_percent: Option<f32>,
    pub memory_available_bytes: Option<u64>,
    pub accelerators: Vec<AcceleratorMetrics>,
}
```

### `AcceleratorMetrics`

```rust
pub struct AcceleratorMetrics {
    pub kind: AcceleratorKind,
    pub index: Option<u32>,
    pub name: String,
    pub memory_total_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_free_bytes: Option<u64>,
    pub utilization_percent: Option<f32>,
}
```

Unavailable dynamic values are represented by `None`; the implementation does
not replace unavailable measurements with fabricated zero values.

### `MetricsProbeError`

```rust
pub enum MetricsProbeError {
    SamplingFailed(String),
}
```

## `burncloud-metrics-probe`

### `RealMetricsProbe`

The production implementation of
`burncloud_node_contracts::RuntimeMetricsProbe`. It performs a fresh sample on
every call and does not depend on `burncloud-hardware-probe`.

```rust
pub struct RealMetricsProbe;

impl RealMetricsProbe {
    pub fn new() -> Self;
    pub fn with_sample_interval(self, interval: std::time::Duration) -> Self;
}

impl Default for RealMetricsProbe;
impl RuntimeMetricsProbe for RealMetricsProbe;
```

The default CPU sampling interval is 200 milliseconds. CPU usage uses two
platform samples; RAM availability and NVIDIA, AMD, and Apple GPU metrics are
sampled fresh where the operating system exposes them. Unsupported or
unmeasurable dynamic fields are represented by `None`.
