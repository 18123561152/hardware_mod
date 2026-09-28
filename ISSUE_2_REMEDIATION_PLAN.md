# Issue #2 整改方案：对齐 BurnCloud 权威 HardwareProbe 合约

> 来源：GitHub Issue #2 `fix(contract): implement BurnCloud's canonical HardwareProbe contract`。
> 本文创建于 2026-09-28，用于将 `hardware_mod` 从独立合约原型收敛为可进入 BurnCloud Integration Queue 的伙伴实现。

## 目标

`RealHardwareProbe` 必须直接实现 BurnCloud 的权威合约：

```text
crates/platform/node/src/contracts.rs
        ↓
burncloud_node_runtime::HardwareProbe
        ↓
RealHardwareProbe
        ↓
Windows / Linux / macOS 平台检测
```

最终不能保留第二套可参与集成的 `HardwareProbe`、`HardwareProfile`、
`AcceleratorProfile`、`AcceleratorKind` 或 `HardwareProbeError`。

## 现状与差异

当前 `hardware_mod` 的 `burncloud-node-contracts` 是 BurnCloud 合约的复制品，
但 BurnCloud 集成时实际消费的是 `burncloud_node_runtime` 中的类型。即使 trait
名称和签名相同，两个 crate 导出的 trait 仍是不同 Rust 类型，因而现有本地
`contract_compatibility` 测试不足以证明 BurnCloud 兼容性。

此外，本地 `HardwareProfile` 目前增加了以下非权威字段：

```rust
pub cpu_brand: Option<String>,
pub cpu_model: Option<String>,
```

并派生了 `Default`。这三项均不属于 Issue #2 指定的当前 BurnCloud 权威合约，
不得在本次集成中要求 BurnCloud 接受。

## 整改步骤

### 1. 收敛到权威依赖

1. 在 `burncloud-hardware-probe/Cargo.toml` 中移除对本地
   `burncloud-node-contracts` 的运行时依赖。
2. 添加对权威 `burncloud-node-runtime` 的依赖；集成验证阶段使用 BurnCloud
   workspace 中与目标提交一致的来源，而不是本地复制 crate。
3. 修改 `detector.rs` 的 import，使 `RealHardwareProbe` 直接实现：

   ```rust
   impl burncloud_node_runtime::HardwareProbe for RealHardwareProbe
   ```

4. `inspect()` 的返回值必须是：

   ```rust
   Result<burncloud_node_runtime::HardwareProfile,
          burncloud_node_runtime::HardwareProbeError>
   ```

5. 保持平台专用 `Raw*` 结构、解析器和内部 `DetectError` 为私有实现细节；
   仅在 `HardwareProbe` 边界映射为权威 `HardwareProbeError`。

### 2. 收紧 HardwareProfile 映射

`RealHardwareProbe` 向权威 profile 只映射：

```rust
HardwareProfile {
    cpu_threads,
    memory_bytes,
    disk_available_bytes,
    accelerators,
}
```

- 移除对 `HardwareProfile::default()` 的所有依赖。
- 不向 BurnCloud 的 `HardwareProfile` 添加 `cpu_brand` 或 `cpu_model`。
- CPU 品牌和型号检测代码可以继续存在，但本次只能作为私有原始事实，不能通过
  `HardwareProbe::inspect()` 的公共结果导出。
- 如需让 BurnCloud 消费 CPU 品牌、型号或架构，必须另开合约变更 Issue，在
  BurnCloud 权威合同中完成审查后再实现。

### 3. 明确 NVIDIA 错误语义

检测器必须区分下面两种情况：

| 情况 | `inspect()` 行为 |
| --- | --- |
| 平台确认没有 NVIDIA 硬件 | 成功，且不返回 NVIDIA accelerator |
| 平台确认有 NVIDIA 硬件，但 `nvidia-smi`、驱动或权限失败 | `Err(HardwareProbeError::DetectionFailed(...))` |

不得将第二种情况转换为 `accelerators: []`。实施前若不能以各平台公开信息可靠
确认 NVIDIA 硬件存在，则停止该部分实现并提出新的平台检测方案，不得猜测。

AMD、Apple 与 NVIDIA 的静态检测仍然只报告机器事实；不得引入模型选择、显存
预留、GPU 调度或运行时兼容性判断。

### 4. 隔离动态指标原型

`RuntimeMetricsProbe`、`RuntimeMetrics`、`AcceleratorMetrics` 和
`MetricsProbeError` 不属于当前已接受的 BurnCloud 硬件合约。

- 可保留在本仓库作为研究/原型 crate。
- 不得作为本次 `HardwareProbe` 集成 PR 的已接受公共接口。
- 不得使 `burncloud-hardware-probe` 的兼容路径依赖这些类型。
- 需要单独 Issue、独立合约审查和明确 owner 批准后，才能接入 BurnCloud。

### 5. 调整测试

删除或改写当前仅依赖本地复制合约的兼容性测试。替换为使用真实
`burncloud_node_runtime` 类型的编译期与运行期验证：

```rust
fn assert_hardware_probe<T: burncloud_node_runtime::HardwareProbe>() {}

assert_hardware_probe::<RealHardwareProbe>();

let probe: &dyn burncloud_node_runtime::HardwareProbe =
    &RealHardwareProbe::default();
let profile: burncloud_node_runtime::HardwareProfile = probe.inspect().await?;
```

测试至少覆盖：

- `RealHardwareProbe` 实现权威 trait。
- `inspect()` 返回权威 `HardwareProfile`。
- 测试不导入本地复制的 `HardwareProbe`。
- 无 NVIDIA 硬件时成功且无 NVIDIA 设备。
- 已知存在 NVIDIA 硬件但查询失败时返回权威 `DetectionFailed`。
- Windows、Linux、macOS 均经由同一权威 adapter 边界编译；平台解析单测继续
  覆盖各自命令或系统输出。
- 在 BurnCloud workspace 内执行真实编译和测试，而不是只在本独立 workspace
  中通过测试。

### 6. 删除复制合约与生成文件

1. `crates/burncloud-node-contracts` 不再承担 BurnCloud 合约定义。
2. 在完成真实依赖迁移后，删除该 crate，或将其完全从集成依赖图中隔离；不得
   继续演进为第二套 BurnCloud 合约。
3. 从 Git 移除 `target/` 下的全部已跟踪内容。
4. 在仓库根新增 `.gitignore`，至少忽略：

   ```gitignore
   /target/
   **/*.pdb
   **/*.exe
   ```

5. PR 提交前检查不包含缓存、二进制、构建输出或其他生成文件。

## 不在本次范围

- 修改 BurnCloud 权威 `HardwareProfile`、`HardwareProbe` 或错误类型。
- 在权威 profile 中公开 CPU 品牌、CPU 型号、CPU 架构。
- Model Resolver、模型 Variant 选择。
- GPU 调度、预留或售卖。
- Runtime/驱动下载、进程启动或 `ProcessManager` 修改。
- Router、Provider、Billing、Tenant、Marketplace 领域逻辑。
- 将 `RuntimeMetricsProbe` 宣称为已接受的 BurnCloud 合约。

## 验收清单

- [ ] `RealHardwareProbe` 直接实现 `burncloud_node_runtime::HardwareProbe`。
- [ ] `inspect()` 返回权威 `burncloud_node_runtime::HardwareProfile`。
- [ ] 兼容性测试仅导入权威 trait、profile 与错误类型。
- [ ] 本次未要求修改 BurnCloud 权威合约。
- [ ] 不依赖 `HardwareProfile::default()`。
- [ ] 无 NVIDIA GPU 与 NVIDIA 检测失败有不同测试结果。
- [ ] Windows/Linux/macOS 保持在同一 HardwareProbe 实现边界后。
- [ ] 动态指标原型不进入本次合约兼容路径。
- [ ] `target/` 未被 Git 跟踪，`.gitignore` 已覆盖构建产物。
- [ ] `cargo test`、权威合同兼容测试以及 BurnCloud workspace 的真实编译验证均通过。

## 停止条件

以下任一情况出现时停止实现并申请 BurnCloud 合约审查：

- 必须新增或修改权威 `HardwareProfile` 字段。
- 必须新增公开错误类型。
- 必须把 `RuntimeMetricsProbe` 纳入本次集成。
- 无法区分“没有 NVIDIA GPU”和“有 NVIDIA GPU 但检测失败”。
- 无法让实现直接依赖 `burncloud-node-runtime`。
- 需求扩展到模型、路由、供应商、计费、租户或 GPU 调度。
