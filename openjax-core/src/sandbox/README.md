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
  - 编排流程：`capabilities 检测 -> runtime -> degrade -> result -> audit`
  - 注：主审批决策（Allow/Ask/Escalate/Deny）已在 `ToolOrchestrator` 中完成，`execute_shell` 被调用时主审批已通过
- `policy.rs`
  - `detect_capabilities(command)`：命令能力检测（fs_read/fs_write/network 等）
  - `extract_shell_risk_tags(command, require_escalated)`：纯函数，提取风险标签（供 orchestrator 注入 Policy Center 输入）
  - `preferred_backend(sandbox_policy)`：根据沙箱策略选择 runtime 后端
  - `evaluate_tool_invocation_policy()`：辅助函数（mutating/non-mutating 判断），不参与主审批流程
  - `PolicyTrace`：构造降级审批判断所用的上下文
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

shell 工具的完整执行链路分两层：

**第一层：orchestrator 主审批（在进入 sandbox 前）**

1. `ToolOrchestrator::run()` 调用 `evaluate_policy_center_decision()`。
2. Policy Center（`openjax-policy`）做出 Allow / Ask / Escalate / Deny 决策。
   - `Deny` → 立即返回 Err，不进入 sandbox。
   - `Ask / Escalate` → 发出 `ApprovalRequested` 事件，等待用户审批；拒绝则返回 Err。
   - `Allow` → 审批通过，继续调用 `registry.dispatch()`。
3. `ShellCommandHandler::handle()` 解析参数后调用 `sandbox::execute_shell()`。

**第二层：sandbox 隔离执行（主审批已通过）**

4. `policy.rs` 的 `detect_capabilities()` 检测命令能力，构造 `policy_trace`（仅用于降级审批判断）。
5. `runtime/mod.rs` 选择 backend 并执行命令。
6. 若 backend 不可用，`degrade.rs` 处理降级二次审批与 fallback。
7. `result.rs` 产出结果分类，`audit.rs` 记录审计信息。

## Strategy Rules

- 主审批决策（`openjax-policy` + orchestrator）
  - `Deny`：直接拒绝，不进入 sandbox。
  - `Ask / Escalate`：等待用户审批，通过后才进入 sandbox。
  - `Allow`：直接进入 sandbox 执行。
- sandbox 内部降级路径（`execute_in_sandbox` 返回 `Err`，即 backend 不可用）
  - `OPENJAX_SANDBOX_DEGRADE_POLICY=deny`: 直接失败。
  - `OPENJAX_SANDBOX_DEGRADE_POLICY=ask_then_allow`:
    - 普通只读命令且 `policy_trace.decision=Allow` 且无写/网络能力：自动降级到 `none_escalated`。
    - `ProcessObserve`（`ps/top/pgrep`）或有写/网络能力的命令：先二次审批，通过后降级执行。
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
