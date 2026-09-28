mod contracts;
mod detector;
mod nvidia;
mod platform;

pub use contracts::{
    AcceleratorKind, AcceleratorMetrics, MetricsProbeError, RuntimeMetrics, RuntimeMetricsProbe,
};
pub use detector::RealMetricsProbe;
