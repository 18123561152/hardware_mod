# BurnCloud Hardware Probe Workspace

This workspace provides an external implementation of the BurnCloud Node
hardware probe. Its static hardware integration depends directly on the
canonical `burncloud-node-runtime` crate pinned in `Cargo.lock`.

- `burncloud-hardware-probe` implements `HardwareProbe` using platform-local
  command line detection and returns the canonical `HardwareProfile`.
- CPU brand and model are detected only as private implementation data; the
  canonical profile does not expose those fields.
- Static AMD and Apple GPU identity detection is best-effort on Windows,
  Linux, and macOS.
- `burncloud-metrics-probe` refreshes optional CPU, memory, NVIDIA, AMD, and
  Apple runtime metrics without caching samples. It is an experimental local
  API and is not an accepted BurnCloud contract.

The static probe reports machine facts only. Resource requirements, runtime
compatibility decisions, model selection, GPU scheduling, and process startup
do not belong to this workspace.

## Development

```text
cargo test --workspace
cargo build -p burncloud-hardware-probe
```

Before integration, run the canonical compatibility test in this workspace and
then build the resulting integration in the BurnCloud workspace.
