use burncloud_metrics_probe::{RealMetricsProbe, RuntimeMetricsProbe};

fn assert_probe<T: RuntimeMetricsProbe>() {}

#[test]
fn real_metrics_probe_satisfies_the_contract() {
    assert_probe::<RealMetricsProbe>();
}
