# BurnCloud Hardware Probe Workspace

This is an independent development workspace for the BurnCloud Node hardware
probe. It is intentionally not embedded in the BurnCloud repository yet.

- `burncloud-node-contracts` contains the single stable hardware contract.
- `burncloud-hardware-probe` implements `HardwareProbe` using platform-local
  command line detection.
- Static profiles include optional CPU brand/model fields and best-effort AMD
  and Apple GPU identity detection on Windows, Linux, and macOS.
- `burncloud-metrics-probe` refreshes optional CPU, memory, NVIDIA, AMD, and
  Apple runtime metrics without caching samples.

The contract crate must stay synchronized with
`crates/platform/node/src/contracts.rs` in the BurnCloud repository. Dynamic
RAM, disk, and GPU facts are refreshed on every `inspect()` call. Resource
requirements and runtime compatibility decisions do not belong to this probe.

## Development

```text
cargo test --workspace
cargo build -p burncloud-hardware-probe
```

Embedding this workspace into BurnCloud is a later step and is deliberately
outside the scope of this package.
