# Sandbox Subsystem (`openjax-core/src/sandbox`)

`sandbox` 是 shell 安全执行子系统，负责策略评估、运行时隔离执行、降级审批、结果分类与审计。

## File Tree

```text
openjax-core/src/sandbox
├── README.md
├── mod.rs
├── audit.rs
├── classifier.rs
├── degrade.rs
├── policy.rs
├── result.rs
├── types.rs
└── runtime
    ├── mod.rs
    ├── linux.rs
    ├── macos.rs
    ├── none.rs
    └── windows.rs
```

## Module Responsibilities

- `mod.rs`
  - façade 入口：`execute_shell(...)`
  - 编排流程：`policy -> runtime -> degrade -> result -> audit`
- `policy.rs`
  - 命令风险识别与能力映射
  - 输出 `PolicyDecision/PolicyOutcome/PolicyTrace`
- `runtime/mod.rs`
  - runtime 调度与后端选择
  - 公共执行请求/响应类型
- `runtime/macos.rs`
  - macOS `sandbox-exec` (seatbelt) 后端实现
- `runtime/linux.rs`
  - Linux backend 实现（当前为 `bwrap` 路径）
- `runtime/none.rs`
  - 非沙箱执行（降级 fallback）
- `runtime/windows.rs`
  - Windows backend 占位（后续扩展点）
- `degrade.rs`
  - backend 不可用或拒绝时的审批请求与事件发送
- `result.rs`
  - shell 执行结果分类（`success/partial_success/failure`）
  - fatal stderr 判定（例如 `Operation not permitted`）
- `classifier.rs`
  - 命令类别识别（包含 `ProcessObserve`）
- `audit.rs`
  - 统一审计日志输出
- `types.rs`
  - sandbox 子系统共享类型

## Execution Flow

1. `tools/handlers/shell.rs` 解析参数后调用 `sandbox::execute_shell`.
2. `policy.rs` 生成策略决策与能力集合。
3. `runtime/mod.rs` 选择 backend 并执行命令。
4. 若 backend 不可用，`degrade.rs` 处理审批与 fallback。
5. `result.rs` 产出结果分类，`audit.rs` 记录审计信息。

## Strategy Rules

- 策略判定（`policy.rs`）
  - `PolicyDecision::Deny`: 直接拒绝，不执行 runtime。
  - `PolicyDecision::AskApproval/AskEscalation`: 进入审批流程（非 shell 由 orchestrator 处理，shell 降级审批由 sandbox 处理）。
  - `PolicyDecision::Allow`: 允许进入 runtime。
- backend 不可用时（`execute_in_sandbox` 返回 `Err`）
  - `OPENJAX_SANDBOX_DEGRADE_POLICY=deny`: 直接失败。
  - `OPENJAX_SANDBOX_DEGRADE_POLICY=ask_then_allow`:
    - 普通只读命令且 `policy=Allow`：允许自动降级到 `none_escalated`。
    - `ProcessObserve`（`ps/top/pgrep`）或非 Allow 场景：先审批，审批通过后再降级执行。
- runtime 成功但输出可疑时
  - 若命中 `exit_code=0 + fatal stderr`（如 `Operation not permitted`），会设置 `runtime_allowed=false`。
  - 对 `ProcessObserve` 命令，该场景会触发审批后降级重试。
- 审批超时
  - 默认超时 5 分钟（`300000ms`）。
  - 可通过 `OPENJAX_APPROVAL_TIMEOUT_MS` 覆盖。
  - 超时会返回 `ApprovalTimedOut`，并发送 `ApprovalResolved(approved=false)` 以收敛 UI 状态。

## ProcessObserve Scope

- 当前识别为 `ProcessObserve` 的命令前缀：
  - `ps `
  - `top `
  - `pgrep `
- 这些命令在 macOS seatbelt 下若被拒绝，会优先走“审批后降级”路径，而不是静默自动降级。

## Compatibility Notes

- `tools/policy.rs` 和 `tools/sandbox_runtime.rs` 目前为兼容转发层（re-export）。
- shell 输出保留历史字段，并新增：
  - `runtime_allowed`
  - `runtime_deny_reason`
