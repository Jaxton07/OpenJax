# 沙箱和批准机制

本文档描述工具系统的权限与审批行为，并给出与 `openjax-core/src/sandbox/` 的对应关系。

## 总览

工具执行由两层控制：

1. **工具编排层（ToolOrchestrator）**
- 根据工具类型和策略决定是否触发审批事件
- 对非 shell 工具执行常规审批流程

2. **shell 沙箱层（sandbox façade）**
- `tools/handlers/shell.rs` 调用 `sandbox::execute_shell(...)`
- 执行链路：`policy -> runtime -> degrade -> result -> audit`
- 支持 backend 降级审批与 `runtime_allowed/runtime_deny_reason` 结果语义

## 核心策略类型

```rust
pub enum SandboxPolicy {
    None,
    ReadOnly,
    Write,
    DangerFullAccess,
}

pub enum ApprovalPolicy {
    AlwaysAsk,
    OnRequest,
    Never,
}
```

环境变量：

- `OPENJAX_SANDBOX_MODE`
- `OPENJAX_APPROVAL_POLICY`
- `OPENJAX_SANDBOX_DEGRADE_POLICY`

## shell 工具说明

`shell/exec_command` 属于高风险工具，实际限制和降级行为由 `openjax-core/src/sandbox/README.md` 定义。

- `ProcessObserve`（`ps/top/pgrep`）在 runtime 拒绝时会触发审批后降级重试
- 输出包含：
  - `result_class`
  - `runtime_allowed`
  - `runtime_deny_reason`
  - `backend` / `degrade_reason`

## 系统类只读工具（process_snapshot/system_load/disk_usage）

这些工具在 `openjax-core/src/tools/system/` 中实现，不走 shell 命令执行路径：

- 无任意命令拼接
- 参数白名单
- 返回结构化 JSON
- 默认视为非 mutating

这类工具用于替代易受平台差异与沙箱拒绝影响的 `ps/top/df` 场景。

## 推荐实践

1. 进程与系统观测优先使用 `process_snapshot/system_load/disk_usage`。
2. 仅在系统工具无法覆盖时再使用 `shell`。
3. 生产环境默认 `workspace_write + on_request`。
4. 调试 shell 拒绝问题时，优先查看 `sandbox audit` 日志和 `runtime_deny_reason`。

## 参考

- [Sandbox 子系统文档](../../sandbox/README.md)
- [工具架构文档](./architecture.md)
- [工具列表](./tools-list.md)
