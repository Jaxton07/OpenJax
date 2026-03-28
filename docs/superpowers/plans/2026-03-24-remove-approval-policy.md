# Remove approval_policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **MCP:** 优先使用 `rustrover-index` MCP 做符号/引用定位，避免全量 grep。

**Goal:** 删除 `ApprovalPolicy` 运行时闸门，所有工具（含 shell）审批决策统一收敛到 Policy Center，新增 `ApprovalKind` 事件字段区分 normal/escalation 审批。

**Architecture:** 协议先行（Phase 1 additive）→ Policy 层补全（Phase 2）→ Core 硬切（Phase 3）→ TUI 适配（Phase 4）→ 文档清理（Phase 5）。Shell 工具命令分析由决策函数重构为风险标签提取器，orchestrator 统一调用 policy center。

**Tech Stack:** Rust 2024 edition、`openjax-protocol`、`openjax-policy`、`openjax-core`、`ui/tui`（Ratatui）

**Spec:** `docs/superpowers/specs/2026-03-24-remove-approval-policy-design.md`

---

## 文件地图

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `openjax-protocol/src/lib.rs` | 修改 | 新增 `ApprovalKind` 枚举；`Event::ApprovalRequested` 加 `approval_kind` 字段 |
| `openjax-policy/src/store.rs` | 修改 | `PolicyStore::new()` 内置 `system:destructive_escalate` 规则 |
| `openjax-policy/src/schema.rs` | 修改 | `DecisionKind` 新增 `as_str()` 方法 |
| `openjax-policy/src/runtime.rs` | 修改 | `PolicyHandle` 新增 `default_decision()` 方法 |
| `openjax-core/src/tools/context.rs` | 修改 | 删除 `ApprovalPolicy` 枚举及 `ToolTurnContext.approval_policy` 字段 |
| `openjax-core/src/tools/router.rs` | 修改 | 删除 `ToolRuntimeConfig.approval_policy` |
| `openjax-core/src/tools/tool_builder.rs` | 修改 | 删除 `CreateToolInvocationParams.approval_policy` |
| `openjax-core/src/tools/mod.rs` | 修改 | 删除 `pub use context::ApprovalPolicy` |
| `openjax-core/src/lib.rs` | 修改 | 删除 `pub use tools::ApprovalPolicy` |
| `openjax-core/src/agent/bootstrap.rs` | 修改 | 删除构造函数中 `approval_policy` 参数；新增 `policy_default_decision_name()` |
| `openjax-core/src/agent/lifecycle.rs` | 修改 | 删除 `spawn_sub_agent` 中 `approval_policy` 传递 |
| `openjax-core/src/agent/runtime_policy.rs` | 修改 | 删除 `parse_approval_policy`、`resolve_approval_policy`、`OPENJAX_APPROVAL_POLICY` |
| `openjax-core/src/config.rs` | 修改 | 删除 `SandboxConfig.approval_policy`、config template 中对应行 |
| `openjax-core/src/tools/system/process_snapshot.rs` | 修改 | 测试构造 `ToolTurnContext` 去掉 `approval_policy` |
| `openjax-core/src/tools/system/system_load.rs` | 修改 | 同上 |
| `openjax-core/src/tools/system/disk_usage.rs` | 修改 | 同上 |
| `openjax-core/src/sandbox/policy.rs` | 修改 | 替换本地 `PolicyDecision` → `DecisionKind`；`decide_shell_policy` 重构为 `extract_shell_risk_tags` |
| `openjax-core/src/sandbox/degrade.rs` | 修改 | 删除 `ApprovalPolicy::Never` 检查；新增 `query_policy_center_for_degrade` |
| `openjax-core/src/tools/orchestrator.rs` | 修改 | 删除 `is_shell_like_tool` gate、`merge_policy_center_outcome`、`map_policy_center_decision`、`decision_rank`；统一 shell/non-shell 路径；填写 `approval_kind` |
| `openjax-core/src/tools/context.rs` | 修改 | `ToolInvocation::policy_descriptor()` 删除 shell 分支 |
| `openjax-core/tests/approval_events_suite.rs` | 修改 | 新增 `approval_kind` 字段断言 |
| `openjax-core/tests/policy_center_suite.rs` | 修改 | 新增 shell 工具行为测试、degrade 测试 |
| 约 16 个测试文件 | 修改 | 批量删除 `ToolTurnContext` 构造中的 `approval_policy` 字段 |
| `ui/tui/src/app/mod.rs` | 修改 | `set_runtime_info` 签名变更 |
| `ui/tui/src/state/app_state.rs` | 修改 | `approval_policy` 字段 → `policy_default` |
| `ui/tui/src/runtime.rs` | 修改 | 调用 `policy_default_decision_name()` |

---

## Task 1：Protocol — 新增 ApprovalKind 枚举和 approval_kind 字段

**Files:**
- Modify: `openjax-protocol/src/lib.rs:183-202`

- [ ] **Step 1: 在 `Event::ApprovalRequested` 上方新增 `ApprovalKind` 枚举**

在 `openjax-protocol/src/lib.rs` 找到 `ApprovalRequested` 定义上方，插入：

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    Normal,
    Escalation,
}
```

- [ ] **Step 2: 在 `Event::ApprovalRequested` 末尾加 `approval_kind` 字段**

在 `degrade_reason: Option<String>` 后插入：

```rust
#[serde(default)]
pub approval_kind: Option<ApprovalKind>,
```

- [ ] **Step 3: 编译验证**

```bash
zsh -lc "cargo build -p openjax-protocol"
```

Expected: 编译通过（additive 变更，现有代码不受影响）

- [ ] **Step 4: 运行 protocol 测试**

```bash
zsh -lc "cargo test -p openjax-protocol"
```

Expected: 全绿

- [ ] **Step 5: Commit**

```bash
git add openjax-protocol/src/lib.rs
git commit -m "feat(protocol): 新增 ApprovalKind 枚举和 approval_kind 字段

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 2：approval_events_suite — 补充 approval_kind 断言

**Files:**
- Modify: `openjax-core/tests/approval_events_suite.rs`

- [ ] **Step 1: 查看现有测试中 `ApprovalRequested` 的断言**

用 rustrover-index 或搜索找到 `approval_events_suite.rs` 中所有断言 `Event::ApprovalRequested` 的位置。

- [ ] **Step 2: 在现有 ask 场景断言中加 `approval_kind: Some(ApprovalKind::Normal)`**

对所有 `Event::ApprovalRequested { .. }` 解构断言，补充：

```rust
// 在 let Event::ApprovalRequested { approval_kind, .. } = event 之后
assert_eq!(approval_kind, &Some(openjax_protocol::ApprovalKind::Normal));
```

注意：此时 orchestrator 尚未填写该字段，测试会失败——这是预期的 failing test，留待 Task 9 修复。

- [ ] **Step 3: 运行测试确认失败（预期行为）**

```bash
zsh -lc "cargo test -p openjax-core --test approval_events_suite -- --nocapture"
```

Expected: 断言 `approval_kind` 的测试失败，其余通过

- [ ] **Step 4: Commit**

```bash
git add openjax-core/tests/approval_events_suite.rs
git commit -m "test(core): 新增 approval_kind 字段断言（预期失败，待 orchestrator 修复）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 3：openjax-policy — 内置 system:destructive_escalate 规则

**Files:**
- Modify: `openjax-policy/src/store.rs`

- [ ] **Step 1: 写失败测试**

在 `openjax-policy/src/store.rs` 的 `#[cfg(test)]` 块中加：

```rust
#[test]
fn policy_store_has_builtin_destructive_escalate_rule() {
    use crate::schema::DecisionKind;
    let store = PolicyStore::new(DecisionKind::Ask, vec![]);
    let rule = store.rules.iter().find(|r| r.id == "system:destructive_escalate");
    assert!(rule.is_some(), "system:destructive_escalate rule must exist");
    let rule = rule.unwrap();
    assert_eq!(rule.decision, DecisionKind::Escalate);
    assert_eq!(rule.priority, 1000);
    assert!(rule.risk_tags_all.contains(&"destructive".to_string()));
}

#[test]
fn destructive_command_triggers_escalate_via_policy_center() {
    use crate::schema::{DecisionKind, PolicyInput};
    use crate::engine::decide;
    let store = PolicyStore::new(DecisionKind::Ask, vec![]);
    let input = PolicyInput {
        tool_name: "shell".to_string(),
        action: "exec".to_string(),
        session_id: None,
        actor: None,
        resource: None,
        capabilities: vec![],
        risk_tags: vec!["destructive".to_string()],
        policy_version: 0,
    };
    let decision = decide(&store, &input);
    assert_eq!(decision.kind, DecisionKind::Escalate);
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
zsh -lc "cargo test -p openjax-policy -- policy_store_has_builtin"
```

Expected: FAIL

- [ ] **Step 3: 在 `PolicyStore::new()` 中插入系统规则**

```rust
pub fn new(default_decision: DecisionKind, mut rules: Vec<PolicyRule>) -> Self {
    // 插入系统内置规则（高优先级，不可被用户规则覆盖）
    let system_destructive = PolicyRule {
        id: "system:destructive_escalate".to_string(),
        decision: DecisionKind::Escalate,
        priority: 1000,
        tool_name: None,
        action: None,
        session_id: None,
        actor: None,
        resource: None,
        capabilities_all: vec![],
        risk_tags_all: vec!["destructive".to_string()],
        reason: "destructive commands always require escalation approval".to_string(),
    };
    rules.push(system_destructive);
    Self { default_decision, rules }
}
```

注意：`PolicyRule` 的字段名以代码实际定义为准，用 rustrover-index 确认 `openjax-policy/src/schema.rs` 中 `PolicyRule` 的字段。

- [ ] **Step 4: 运行测试确认通过**

```bash
zsh -lc "cargo test -p openjax-policy --tests"
```

Expected: 全绿

- [ ] **Step 5: Commit**

```bash
git add openjax-policy/src/store.rs
git commit -m "feat(policy): 内置 system:destructive_escalate 系统规则

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 4：openjax-policy — PolicyHandle::default_decision()

**Files:**
- Modify: `openjax-policy/src/runtime.rs`

- [ ] **Step 1: 写失败测试**

在 `openjax-policy/src/runtime.rs` 的 `#[cfg(test)]` 块中加：

```rust
#[test]
fn policy_handle_exposes_default_decision() {
    use crate::schema::DecisionKind;
    use crate::store::PolicyStore;
    let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));
    assert_eq!(runtime.handle().default_decision(), DecisionKind::Ask);

    let runtime2 = PolicyRuntime::new(PolicyStore::new(DecisionKind::Deny, vec![]));
    assert_eq!(runtime2.handle().default_decision(), DecisionKind::Deny);
}
```

- [ ] **Step 2: 运行确认失败**

```bash
zsh -lc "cargo test -p openjax-policy -- policy_handle_exposes"
```

Expected: FAIL — `no method named default_decision`

- [ ] **Step 3: 在 `openjax-policy/src/schema.rs` 中为 `DecisionKind` 新增 `as_str()` 方法**

在 `impl DecisionKind` 块中加（若无该 impl 块则新建）：

```rust
pub fn as_str(&self) -> &'static str {
    match self {
        DecisionKind::Allow => "allow",
        DecisionKind::Ask => "ask",
        DecisionKind::Escalate => "escalate",
        DecisionKind::Deny => "deny",
    }
}
```

- [ ] **Step 4: 在 `PolicyHandle` 上新增 `default_decision()` 方法**

在 `impl PolicyHandle` 块中加：

```rust
pub fn default_decision(&self) -> DecisionKind {
    self.snapshot.store.default_decision.clone()
}
```

- [ ] **Step 5: 运行测试确认通过**

```bash
zsh -lc "cargo test -p openjax-policy --tests"
```

Expected: 全绿

- [ ] **Step 6: Commit**

```bash
git add openjax-policy/src/schema.rs openjax-policy/src/runtime.rs
git commit -m "feat(policy): DecisionKind 新增 as_str()，PolicyHandle 新增 default_decision() 方法

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 5：Core 硬切 — 删除 ApprovalPolicy 类型声明

**Files:**
- Modify: `openjax-core/src/tools/context.rs`
- Modify: `openjax-core/src/tools/mod.rs`
- Modify: `openjax-core/src/lib.rs`

> ⚠️ 此 Task 执行后编译会报错，这是预期的。Task 6-8 将修复所有引用。

- [ ] **Step 1: 删除 `context.rs` 中的 `ApprovalPolicy` 枚举**

删除 `context.rs` 中以下内容（约 lines 252-276）：

```rust
/// 批准策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPolicy {
    AlwaysAsk,
    OnRequest,
    Never,
}

impl ApprovalPolicy {
    pub fn from_env() -> Self { ... }
    pub fn as_str(&self) -> &'static str { ... }
}
```

- [ ] **Step 2: 删除 `ToolTurnContext.approval_policy` 字段**

在 `ToolTurnContext` 结构体中删除：
```rust
pub approval_policy: ApprovalPolicy,
```

同时在 `ToolTurnContext::default()` 中删除：
```rust
approval_policy: ApprovalPolicy::OnRequest,
```

同时在 `ToolTurnContext` 的 `Debug` 实现中删除：
```rust
.field("approval_policy", &self.approval_policy)
```

- [ ] **Step 3: 删除 `mod.rs` 中的 re-export**

在 `openjax-core/src/tools/mod.rs` 的 `pub use context::{ ... }` 中删除 `ApprovalPolicy`。

- [ ] **Step 4: 删除 `lib.rs` 中的 pub use**

在 `openjax-core/src/lib.rs` 删除：
```rust
pub use tools::ApprovalPolicy;
```

- [ ] **Step 5: 确认编译错误（不要修复，留给 Task 6-8）**

```bash
zsh -lc "cargo build -p openjax-core 2>&1 | head -40"
```

Expected: 大量 `cannot find type ApprovalPolicy` 编译错误——这是预期的。

- [ ] **Step 6: Commit（带 WIP 标记）**

```bash
git add openjax-core/src/tools/context.rs openjax-core/src/tools/mod.rs openjax-core/src/lib.rs
git commit -m "refactor(core)[WIP]: 删除 ApprovalPolicy 类型声明（编译未修复）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 6：Core 硬切 — 修复 tools/ 和 agent/ 编译

**Files:**
- Modify: `openjax-core/src/tools/router.rs`
- Modify: `openjax-core/src/tools/tool_builder.rs`
- Modify: `openjax-core/src/agent/bootstrap.rs`
- Modify: `openjax-core/src/agent/lifecycle.rs`
- Modify: `openjax-core/src/agent/runtime_policy.rs`
- Modify: `openjax-core/src/config.rs`

- [ ] **Step 1: `tools/router.rs` — 删除 `ToolRuntimeConfig.approval_policy`**

在 `ToolRuntimeConfig` 结构体中删除：
```rust
pub approval_policy: ApprovalPolicy,
```

在 `Default` 实现和 `with_config` 中删除对应初始化行。

删除文件顶部的 `use crate::tools::context::ApprovalPolicy;`（如有）。

- [ ] **Step 2: `tools/tool_builder.rs` — 删除 `CreateToolInvocationParams.approval_policy`**

删除结构体字段：
```rust
pub approval_policy: ApprovalPolicy,
```

删除 `create_tool_invocation` 函数中的赋值：
```rust
approval_policy: params.approval_policy,
```

- [ ] **Step 3: `agent/runtime_policy.rs` — 删除 approval_policy 相关函数**

删除以下函数（完整函数体）：
- `parse_approval_policy()`
- `resolve_approval_policy()`

删除 `resolve_approval_policy` 中对 `OPENJAX_APPROVAL_POLICY` 环境变量的读取。

删除文件顶部对 `tools::ApprovalPolicy` 的 `use` 引用。

- [ ] **Step 4: `config.rs` — 删除 `SandboxConfig.approval_policy`**

在 `SandboxConfig` 结构体中删除：
```rust
// Approval policy: always_ask | on_request | never
#[serde(default)]
pub approval_policy: Option<String>,
```

在 `DEFAULT_CONFIG_TEMPLATE` 常量中删除：
```
approval_policy = "on_request"
```

- [ ] **Step 5: `agent/bootstrap.rs` — 重构构造函数签名**

修改 `with_runtime` 签名，删除 `approval_policy` 参数：
```rust
// 旧：
pub fn with_runtime(approval_policy: tools::ApprovalPolicy, sandbox_mode: tools::SandboxMode, cwd: PathBuf) -> Self

// 新：
pub fn with_runtime(sandbox_mode: tools::SandboxMode, cwd: PathBuf) -> Self
```

修改 `with_config_and_runtime` 签名：
```rust
// 旧：
pub fn with_config_and_runtime(config: Config, approval_policy: tools::ApprovalPolicy, sandbox_mode: tools::SandboxMode, cwd: PathBuf) -> Self

// 新：
pub fn with_config_and_runtime(config: Config, sandbox_mode: tools::SandboxMode, cwd: PathBuf) -> Self
```

修改 `Agent::new()` — 直接调用 `with_config_and_runtime(config, sandbox_mode, cwd)`，不再传 `approval_policy`。

修改 `Agent::with_config()` — 删除 `resolve_approval_policy` 调用。

删除 `approval_policy_name()` 方法。

在 `ToolRuntimeConfig` 初始化处删除 `approval_policy` 字段。

删除 tracing `info!` 宏中的 `approval_policy = ...` 日志字段。

**新增 `policy_default_decision_name()` 方法**（在删除 `approval_policy_name()` 之后）：

```rust
pub fn policy_default_decision_name(&self) -> &'static str {
    self.policy_runtime
        .as_ref()
        .map(|r| r.handle().default_decision().as_str())
        .unwrap_or("ask")
}
```

- [ ] **Step 6: `agent/lifecycle.rs` — 修复 `spawn_sub_agent`**

将：
```rust
let mut sub_agent = Agent::with_runtime(
    self.tool_runtime_config.approval_policy,
    self.tool_runtime_config.sandbox_mode,
    self.cwd.clone(),
);
```

改为：
```rust
let mut sub_agent = Agent::with_runtime(
    self.tool_runtime_config.sandbox_mode,
    self.cwd.clone(),
);
```

- [ ] **Step 7: 编译检查（确认 tools/ 和 agent/ 通过）**

```bash
zsh -lc "cargo build -p openjax-core 2>&1 | grep -v 'openjax-core/tests/' | grep -v 'src/tests.rs' | head -40"
```

Expected: 主要剩余错误来自 `openjax-core/tests/`、`src/tests.rs` 和 `src/tools/system/`（下一步处理）。注意 `src/tests.rs` 是生产路径文件，不在 `tests/` 目录下，上述 grep 过滤两者。

- [ ] **Step 8: Commit**

```bash
git add openjax-core/src/tools/router.rs openjax-core/src/tools/tool_builder.rs \
  openjax-core/src/agent/bootstrap.rs openjax-core/src/agent/lifecycle.rs \
  openjax-core/src/agent/runtime_policy.rs openjax-core/src/config.rs
git commit -m "refactor(core): 删除 approval_policy 构造参数和 agent 相关字段

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 7：Core 硬切 — 批量修复 system/ 和测试文件

**Files:**
- Modify: `openjax-core/src/tools/system/process_snapshot.rs`
- Modify: `openjax-core/src/tools/system/system_load.rs`
- Modify: `openjax-core/src/tools/system/disk_usage.rs`
- Modify: 约 16 个测试文件（见下方清单）

- [ ] **Step 1: 修复 3 个 system 生产文件**

在以下三个文件中找到测试构造 `ToolTurnContext { ... }` 的位置，删除 `approval_policy: ApprovalPolicy::OnRequest,` 字段：

- `openjax-core/src/tools/system/process_snapshot.rs`
- `openjax-core/src/tools/system/system_load.rs`
- `openjax-core/src/tools/system/disk_usage.rs`

删除对应的 `use crate::tools::context::ApprovalPolicy;` import（如有）。

- [ ] **Step 2: 特殊处理 `approval_events_suite.rs` — 修复 `Agent::with_runtime` 调用签名**

`approval_events_suite.rs` 不仅使用 `approval_policy:` 字段，还在 `Agent::with_runtime()` 调用中将 `ApprovalPolicy::AlwaysAsk` 作为第一个参数，用于确保测试中所有工具都走审批流程。删除 `ApprovalPolicy` 后，须改用 `PolicyRuntime` 注入来保留这一行为：

```rust
// 旧写法：
let mut agent = Agent::with_runtime(
    ApprovalPolicy::AlwaysAsk,
    SandboxMode::WorkspaceWrite,
    workspace.clone(),
);

// 新写法（注入 PolicyRuntime，default_decision = Ask，确保所有工具触发审批）：
use openjax_policy::{runtime::PolicyRuntime, store::PolicyStore, schema::DecisionKind};
let policy_runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));

let mut agent = Agent::with_runtime(
    SandboxMode::WorkspaceWrite,
    workspace.clone(),
);
agent.set_policy_runtime(Some(policy_runtime));  // 签名为 set_policy_runtime(&mut self, runtime: Option<PolicyRuntime>)
```

同时删除文件顶部的 `use openjax_core::ApprovalPolicy;` 及 `ApprovalPolicy::AlwaysAsk` 等引用。

- [ ] **Step 3: 批量修复测试文件**

以下测试文件均含 `approval_policy:` 字段构造，逐一删除该字段和相关 import：

```
openjax-core/tests/policy_center_suite.rs
openjax-core/tests/tools_sandbox/m3_sandbox.rs
openjax-core/tests/tools_sandbox/m4_apply_patch.rs
openjax-core/tests/tools_sandbox/m5_edit_file_range.rs
openjax-core/tests/tools_sandbox/m9_system_tools.rs
openjax-core/tests/approval/m5_approval_handler.rs
openjax-core/tests/approval/m8_approval_event_emission.rs
openjax-core/tests/approval_events_suite.rs
openjax-core/tests/streaming/m6_submit_stream.rs
openjax-core/tests/streaming/m7_backward_compat_submit.rs
openjax-core/tests/streaming/m21_tool_streaming_events.rs
openjax-core/tests/streaming/m23_assistant_message_decommission_guardrails.rs
openjax-core/tests/core_history/m11_context_compression.rs
openjax-core/tests/core_history/m22_history_turn_record.rs
openjax-core/tests/skills/m20_skills_shell_trigger_guard.rs
openjax-core/src/tests.rs
openjaxd/tests/protocol_integration.rs
```

对每个文件：
1. 删除 `approval_policy: ApprovalPolicy::OnRequest` 或 `ApprovalPolicy::Never` 字段
2. 删除 `use openjax_core::tools::context::ApprovalPolicy;` 或 `use openjax_core::ApprovalPolicy;` 等 import

- [ ] **Step 4: 修复 `openjaxd/tests/protocol_integration.rs` — 替换 `OPENJAX_APPROVAL_POLICY=never` 注入**

该文件通过进程级 `.env("OPENJAX_APPROVAL_POLICY", "never")` 防止审批阻断 daemon 测试。删除 env var 支持后，须改为前置构造全允许策略：

将：
```rust
.env("OPENJAX_APPROVAL_POLICY", "never")
```

删除，并在测试启动前通过 gateway API 或配置注入一个 `DecisionKind::Allow` 默认的 `PolicyRuntime`。若该测试直接启动子进程而非嵌入式测试，则仅删除该 `.env()` 调用，并通过 policy API endpoint（`POST /policy/draft` + `POST /policy/publish`）在测试 setup 阶段发布一条 `allow` 规则。用 rustrover-index 确认该测试的实际架构，选择合适方案。

- [ ] **Step 5: 编译验证（全量）**

```bash
zsh -lc "cargo build -p openjax-core && cargo build -p openjaxd"
```

Expected: 编译通过（所有 `ApprovalPolicy` 引用已清除）

- [ ] **Step 6: 运行快速冒烟测试（部分可能失败，观察是否只有行为相关的失败）**

```bash
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite -- --nocapture 2>&1 | tail -20"
```

- [ ] **Step 7: Commit**

```bash
git add openjax-core/src/tools/system/ openjax-core/tests/ openjax-core/src/tests.rs openjaxd/tests/
git commit -m "refactor(core): 批量删除测试文件中的 approval_policy 字段

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 8：sandbox/policy.rs — 重构为风险标签提取器

**Files:**
- Modify: `openjax-core/src/sandbox/policy.rs`

- [ ] **Step 1: 替换本地 `PolicyDecision` 枚举**

删除文件顶部的本地 `PolicyDecision` 枚举定义：
```rust
pub enum PolicyDecision { Allow, AskApproval, AskEscalation, Deny }
```

在文件顶部的 `use` 中加入：
```rust
use openjax_policy::schema::DecisionKind;
```

将文件中所有本地类型引用替换：
- `PolicyDecision::Allow` → `DecisionKind::Allow`
- `PolicyDecision::AskApproval` → `DecisionKind::Ask`
- `PolicyDecision::AskEscalation` → `DecisionKind::Escalate`
- `PolicyDecision::Deny` → `DecisionKind::Deny`

同时更新 `PolicyTrace.decision` 字段类型从 `PolicyDecision` 改为 `DecisionKind`。

- [ ] **Step 2: 重构 `decide_shell_policy` → `extract_shell_risk_tags`**

将函数 `decide_shell_policy(sandbox_policy, command, require_escalated, capabilities, risks) -> PolicyDecision` 重构为：

```rust
/// 从 shell 命令内容提取风险标签，供 policy center 做决策
pub fn extract_shell_risk_tags(command: &str, require_escalated: bool) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let lower = command.to_ascii_lowercase();

    // 破坏性命令 → destructive（由 system:destructive_escalate 规则处理）
    let destructive_patterns = ["rm -rf /", "mkfs", "dd if=", ":(){:|:&};:"];
    if destructive_patterns.iter().any(|p| lower.contains(p)) {
        tags.push("destructive".to_string());
    }

    // 提权请求
    if require_escalated {
        tags.push("require_escalated".to_string());
    }

    // sudo
    if lower.contains("sudo ") {
        tags.push("privilege_escalation".to_string());
    }

    // 网络操作
    let network_tokens = ["curl ", "wget ", "ssh ", "scp ", "nc ", "nmap ", "ping ", "dig "];
    if network_tokens.iter().any(|t| lower.contains(t)) {
        tags.push("network".to_string());
    }

    // 文件系统写操作
    let write_tokens = [">", ">>", "tee ", "rm ", "mv ", "cp ", "chmod ", "chown ",
                        "git add ", "git commit", "git merge", "git rebase"];
    if write_tokens.iter().any(|t| lower.contains(t)) {
        tags.push("fs_write".to_string());
    }

    tags
}
```

- [ ] **Step 3: 删除已无用的私有函数**

删除以下函数（其逻辑已内联到 `extract_shell_risk_tags`）：
- `analyze_shell_invocation()` — 原 shell 分析入口，已无调用
- `detect_capabilities()` — capabilities 已改为 risk_tags
- 原 `decide_shell_policy()` — 已重命名

保留：
- `extract_shell_command()` — orchestrator 需要提取命令字符串
- `truncate_preview()` — approval event 使用
- `preferred_backend()` — sandbox 选择使用
- `normalize_command()` — 可保留或内联

- [ ] **Step 4: 删除 `evaluate_tool_invocation_policy` 中的 approval_policy overlay 块（lines 102-139）和 shell 死代码（lines 93-101）**

删除后 `evaluate_tool_invocation_policy` 只保留：
```rust
pub fn evaluate_tool_invocation_policy(invocation: &ToolInvocation, is_mutating: bool) -> PolicyOutcome {
    // non-shell mutating 工具
    let decision = if is_mutating {
        DecisionKind::Ask
    } else {
        DecisionKind::Allow
    };
    let reason = match decision {
        DecisionKind::Ask => "mutating tool requires approval".to_string(),
        _ => "allowed by default".to_string(),
    };
    PolicyOutcome {
        trace: PolicyTrace {
            decision,
            reason,
            risk_tags: Vec::new(),
            capabilities: Vec::new(),
        },
        approval_context: None,
    }
}
```

注意：此函数在 Task 9 中会被进一步简化（orchestrator 统一路径后可能不再调用它）。

- [ ] **Step 5: 更新文件内 unit tests**

`sandbox/policy.rs` 内的 `#[cfg(test)]` 测试用 `ApprovalPolicy::OnRequest` 构造 `ToolTurnContext`，删除该字段。测试用例中原来断言 `PolicyDecision::AskApproval` 改为 `DecisionKind::Ask` 等。

- [ ] **Step 6: 编译并运行 sandbox 相关测试**

```bash
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
```

- [ ] **Step 7: Commit**

```bash
git add openjax-core/src/sandbox/policy.rs
git commit -m "refactor(core): sandbox/policy.rs 重构为风险标签提取器，替换本地 PolicyDecision

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 9：orchestrator.rs — 统一决策路径 + 填写 approval_kind

**Files:**
- Modify: `openjax-core/src/tools/orchestrator.rs`

- [ ] **Step 1: 删除 `merge_policy_center_outcome`、`map_policy_center_decision`、`decision_rank`**

删除这三个函数的完整定义。

- [ ] **Step 2: 重构 `approval_reason` — 删除 `approval_policy` 参数**

将：
```rust
fn approval_reason(outcome: &PolicyOutcome, approval_policy: ApprovalPolicy) -> String {
    if let Some(context) = &outcome.approval_context {
        return context.reason.clone();
    }
    if matches!(approval_policy, ApprovalPolicy::AlwaysAsk) {
        return "approval policy requires confirmation".to_string();
    }
    outcome.trace.reason.clone()
}
```

简化为：
```rust
fn approval_reason(outcome: &PolicyOutcome) -> String {
    outcome.approval_context.as_ref()
        .map(|ctx| ctx.reason.clone())
        .unwrap_or_else(|| outcome.trace.reason.clone())
}
```

- [ ] **Step 3: 重写 `orchestrator::run()` 主体 — 统一 shell/non-shell 路径**

删除 `if !is_shell_like_tool(...)` 分支结构，替换为：

```rust
// 统一决策路径（shell 和 non-shell 均走此处）
let is_mutating = self.sandbox_manager.is_mutating_operation(&invocation.tool_name);
let policy_center_decision = evaluate_policy_center_decision(&invocation);

if matches!(policy_center_decision.kind, PolicyCenterDecisionKind::Deny) {
    return Err(FunctionCallError::Internal(policy_center_decision.reason.clone()));
}

let has_reusable_approval = self.should_reuse_mutating_approval(&invocation, is_mutating);
let requires_approval = matches!(
    policy_center_decision.kind,
    PolicyCenterDecisionKind::Ask | PolicyCenterDecisionKind::Escalate
) && !has_reusable_approval;

if requires_approval {
    let approval_kind = match policy_center_decision.kind {
        PolicyCenterDecisionKind::Escalate => ApprovalKind::Escalation,
        _ => ApprovalKind::Normal,
    };
    // ... 构造 approval event，填写 approval_kind: Some(approval_kind)
    // ... 等待审批
}
```

- [ ] **Step 4: 为 shell 工具构造 PolicyInput（使用 extract_shell_risk_tags）**

修改 `evaluate_policy_center_decision()` 函数，对 shell 工具特殊处理：

```rust
fn evaluate_policy_center_decision(invocation: &ToolInvocation) -> openjax_policy::PolicyDecision {
    // shell 工具：提取命令风险标签
    let (descriptor, extra_risk_tags) = if is_shell_like_tool(&invocation.tool_name) {
        if let Some((command, require_escalated)) = extract_shell_command_from_invocation(invocation) {
            let risk_tags = crate::sandbox::policy::extract_shell_risk_tags(&command, require_escalated);
            (None, risk_tags)
        } else {
            (None, vec!["invalid_shell_payload".to_string()])
        }
    } else {
        (invocation.policy_descriptor(), vec![])
    };

    // 构造 PolicyInput
    let mut input = invocation.to_policy_center_input(descriptor.as_ref(), /* version */);
    input.risk_tags.extend(extra_risk_tags);

    // 查询 policy center（逻辑不变）
    if let Some(runtime) = invocation.turn.policy_runtime.as_ref() {
        return runtime.handle().decide(&input);
    }
    let rules = descriptor.as_ref()
        .map(|d| vec![d.allow_rule_for_tool(&invocation.tool_name)])
        .unwrap_or_default();
    let runtime = PolicyRuntime::new(PolicyStore::new(PolicyCenterDecisionKind::Ask, rules));
    runtime.handle().decide(&input)
}
```

需要新增辅助函数 `extract_shell_command_from_invocation`（从 `sandbox/policy.rs` 的 `extract_shell_command` 迁移或直接调用）。

- [ ] **Step 5: 在审批事件中填写 approval_kind**

在 `Event::ApprovalRequested { ... }` 发送处加：
```rust
approval_kind: Some(approval_kind),
```

- [ ] **Step 6: 清理 `is_shell_like_tool` 函数**

- **保留** `orchestrator.rs` 中的 `is_shell_like_tool`（Task 9 Step 4 的 `evaluate_policy_center_decision()` 仍需调用它）
- **删除** `sandbox/policy.rs` 中的 `is_shell_like_tool`——Task 8 删除了 `analyze_shell_invocation` 后它在 `policy.rs` 中已无调用方，会触发 `clippy -D warnings` 死代码警告。

```bash
zsh -lc "grep -n 'is_shell_like_tool' openjax-core/src/sandbox/policy.rs"
# 确认无调用方后删除该函数定义
```

- [ ] **Step 7: 运行核心测试**

```bash
zsh -lc "cargo test -p openjax-core --test policy_center_suite"
zsh -lc "cargo test -p openjax-core --test approval_events_suite"
```

Expected: `approval_events_suite` 中 `approval_kind` 断言此时应通过

- [ ] **Step 8: Commit**

```bash
git add openjax-core/src/tools/orchestrator.rs
git commit -m "refactor(core): orchestrator 统一决策路径，填写 approval_kind 字段

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 10：context.rs — 删除 policy_descriptor() shell 分支

**Files:**
- Modify: `openjax-core/src/tools/context.rs`

- [ ] **Step 1: 删除 `ToolInvocation::policy_descriptor()` 中的 shell 分支**

找到 `policy_descriptor()` 的 match 臂中：
```rust
"shell" | "exec_command" => { ... }
```

删除整个 shell 匹配臂（shell 工具的 PolicyInput 现在由 orchestrator 统一构造）。

- [ ] **Step 2: 删除 `shell_payload_requires_escalated()` 私有函数**

此函数的逻辑已内联到 `sandbox/policy.rs::extract_shell_risk_tags()`，可安全删除。

确认无其他调用方（通过 rustrover-index 查找引用）。

- [ ] **Step 3: 编译并运行测试**

```bash
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core --test policy_center_suite"
```

- [ ] **Step 4: Commit**

```bash
git add openjax-core/src/tools/context.rs
git commit -m "refactor(core): 删除 policy_descriptor() shell 分支

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 11：degrade.rs — 重设计降级审批路径

**Files:**
- Modify: `openjax-core/src/sandbox/degrade.rs`

- [ ] **Step 1: 删除 `ApprovalPolicy::Never` 检查**

删除：
```rust
let approval_policy = invocation.turn.approval_policy;
if matches!(approval_policy, ApprovalPolicy::Never) {
    return Err(FunctionCallError::Internal(...));
}
```

及相关 `use crate::tools::ApprovalPolicy;` import。

- [ ] **Step 2: 新增 `query_policy_center_for_degrade` 辅助函数**

```rust
fn query_policy_center_for_degrade(
    invocation: &ToolInvocation,
    command: &str,
) -> openjax_policy::PolicyDecision {
    use crate::sandbox::policy::extract_shell_risk_tags;
    use openjax_policy::{runtime::PolicyRuntime, schema::DecisionKind, store::PolicyStore};

    // 提取命令风险标签 + 加 sandbox_degrade 标签
    let mut risk_tags = extract_shell_risk_tags(command, false);
    risk_tags.push("sandbox_degrade".to_string());

    let mut input = invocation.to_policy_center_input(None, 0);
    input.risk_tags = risk_tags;

    if let Some(runtime) = invocation.turn.policy_runtime.as_ref() {
        let handle = runtime.handle();
        input.policy_version = handle.policy_version();
        return handle.decide(&input);
    }

    // fallback：无 runtime 时默认 Ask → 触发 Escalation 审批
    let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));
    runtime.handle().decide(&input)
}
```

注意：`ToolInvocation` 可能没有 `to_policy_center_input` 方法或参数不同，用 rustrover-index 确认实际签名，按实际调整。

- [ ] **Step 3: 重写 `request_degrade_approval` 开头逻辑**

将原来的 `ApprovalPolicy::Never` 检查替换为：

```rust
let decision = query_policy_center_for_degrade(invocation, command);

if matches!(decision.kind, openjax_policy::schema::DecisionKind::Deny) {
    return Err(FunctionCallError::Internal(format!(
        "sandbox backend unavailable and policy denied degrade: {backend} {reason}"
    )));
}
```

- [ ] **Step 4: 在 degrade 审批事件中填写 `approval_kind: Some(Escalation)`**

找到 `sink.send(Event::ApprovalRequested { ... })` 处，加：
```rust
approval_kind: Some(openjax_protocol::ApprovalKind::Escalation),
```

- [ ] **Step 5: 编译验证**

```bash
zsh -lc "cargo build -p openjax-core"
```

Expected: 编译通过

- [ ] **Step 6: Commit**

```bash
git add openjax-core/src/sandbox/degrade.rs
git commit -m "refactor(core): degrade 路径重设计，用 policy center 替代 ApprovalPolicy::Never

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 12：新增 shell 工具行为测试

**Files:**
- Modify: `openjax-core/tests/policy_center_suite.rs`

- [ ] **Step 1: 在 `policy_center_suite.rs` 顶部添加 `make_shell_invocation` 测试辅助函数**

```rust
fn make_shell_invocation(
    command: &str,
    runtime: &openjax_policy::runtime::PolicyRuntime,
    event_tx: tokio::sync::mpsc::UnboundedSender<openjax_protocol::Event>,
) -> ToolInvocation {
    use openjax_core::tools::shell::ShellType;
    ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: format!("call-shell-{}", command.len()),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "cmd": command }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 99,
            session_id: Some("sess_shell_test".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(RejectApproval),
            event_sink: Some(event_tx),
            policy_runtime: Some(runtime.clone()),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    }
}
```

注意：`ToolTurnContext` 字段以 Task 5-7 删除 `approval_policy` 后的实际结构为准；用 rustrover-index 确认 shell 工具的 payload 参数名（`cmd` 或 `command`）。

- [ ] **Step 2: 新增 shell allow 测试**

```rust
#[tokio::test]
async fn shell_tool_allow_rule_skips_approval() {
    let registry = Arc::new(ToolRegistry::new());
    let orchestrator = ToolOrchestrator::new(registry);

    // allow 规则：exec action
    let store = PolicyStore::new(
        DecisionKind::Ask,
        vec![PolicyRule {
            id: "test:shell_allow".to_string(),
            decision: DecisionKind::Allow,
            priority: 10,
            tool_name: Some("shell".to_string()),
            action: Some("exec".to_string()),
            ..Default::default()
        }],
    );
    let runtime = PolicyRuntime::new(store);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let invocation = make_shell_invocation("echo hello", &runtime, tx);

    // 注册 echo 工具（或用 mock）
    // ...
    let _ = orchestrator.run(invocation).await;

    // 验证没有 ApprovalRequested 事件
    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert!(!events.iter().any(|e| matches!(e, Event::ApprovalRequested { .. })));
}
```

- [ ] **Step 3: 新增 shell ask 触发 Normal 审批测试**

```rust
#[tokio::test]
async fn shell_tool_ask_rule_emits_normal_approval_event() {
    // 规则：network risk_tag → Ask
    // 运行 curl 命令（risk_tags: ["network"]）
    // 断言 ApprovalRequested { approval_kind: Some(Normal) }
}
```

- [ ] **Step 4: 新增 shell escalate 测试**

```rust
#[tokio::test]
async fn shell_tool_escalate_rule_emits_escalation_event() {
    // 规则：require_escalated risk_tag → Escalate
    // 运行带 require_escalated: true 的命令
    // 断言 ApprovalRequested { approval_kind: Some(Escalation) }
}
```

- [ ] **Step 5: 新增 destructive 命令触发 Escalation 测试**

```rust
#[tokio::test]
async fn destructive_command_triggers_escalation_via_system_rule() {
    // 无自定义规则（system:destructive_escalate 规则内置）
    // 运行 "rm -rf /" 命令
    // 断言 ApprovalRequested { approval_kind: Some(Escalation) }
}
```

- [ ] **Step 6: 新增 shell deny 测试**

```rust
#[tokio::test]
async fn shell_tool_deny_rule_returns_error_without_approval() {
    // 规则：shell → Deny
    // 运行任意 shell 命令
    // 断言返回 Err，无 ApprovalRequested 事件
}
```

- [ ] **Step 7: 新增 degrade escalation 测试（stub sandbox backend）**

```rust
#[tokio::test]
async fn degrade_escalate_emits_escalation_approval_event() {
    // mock sandbox backend 不可用
    // 验证 ApprovalRequested { approval_kind: Some(Escalation), risk_tags 含 "sandbox_degrade" }
}
```

- [ ] **Step 8: 运行全部新增测试**

```bash
zsh -lc "cargo test -p openjax-core --test policy_center_suite -- --nocapture"
```

Expected: 全绿

- [ ] **Step 9: Commit**

```bash
git add openjax-core/tests/policy_center_suite.rs
git commit -m "test(core): 新增 shell 工具 policy center 行为测试和 degrade escalation 测试

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 13：TUI 适配

**Files:**
- Modify: `ui/tui/src/app/mod.rs`
- Modify: `ui/tui/src/state/app_state.rs`
- Modify: `ui/tui/src/runtime.rs`

- [ ] **Step 1: `state/app_state.rs` — 字段重命名**

找到 `approval_policy` 字段，重命名为 `policy_default`（类型保持 `String`）。

更新所有引用该字段的 `app_state.rs` 内部代码。

- [ ] **Step 2: `app/mod.rs` — 更新 `set_runtime_info` 签名**

将：
```rust
pub fn set_runtime_info(&mut self, model: String, approval_policy: String, sandbox_mode: String, cwd: &Path)
```

改为：
```rust
pub fn set_runtime_info(&mut self, model: String, policy_default: String, sandbox_mode: String, cwd: &Path)
```

函数体中 `self.state.approval_policy = approval_policy` 改为 `self.state.policy_default = policy_default`。

更新 UI 渲染代码，将显示文本从 `approval_policy: on_request` 变为 `policy: ask`（查找渲染 approval_policy 的 widget 代码）。

- [ ] **Step 3: `runtime.rs` — 更新调用**

将：
```rust
app.set_runtime_info(
    guard.model_backend_name().to_string(),
    guard.approval_policy_name().to_string(),
    guard.sandbox_mode_name().to_string(),
    cwd.as_path(),
);
```

改为：
```rust
app.set_runtime_info(
    guard.model_backend_name().to_string(),
    guard.policy_default_decision_name().to_string(),
    guard.sandbox_mode_name().to_string(),
    cwd.as_path(),
);
```

- [ ] **Step 4: 编译并运行 TUI 测试**

```bash
zsh -lc "cargo build -p tui_next && cargo test -p tui_next --tests"
```

Expected: 编译通过，测试全绿

- [ ] **Step 5: Commit**

```bash
git add ui/tui/src/app/mod.rs ui/tui/src/state/app_state.rs ui/tui/src/runtime.rs
git commit -m "feat(tui): 用 policy_default 替代 approval_policy 展示

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 14：文档清理 + 全量回归

**Files:**
- Modify: `CLAUDE.md`
- Modify: `openjax-core/README.md`
- Modify: `openjaxd/README.md`
- Modify: `docs/config.md`（如存在）
- Modify: `openjax-core/src/tools/docs/`（如存在）

- [ ] **Step 1: 搜索全仓 approval_policy 文档残留**

```bash
grep -r "approval_policy\|OPENJAX_APPROVAL_POLICY\|ApprovalPolicy" \
  --include="*.md" --include="*.toml" --include="*.json" \
  /Users/ericw/work/code/ai/openJax | grep -v "docs/superpowers"
```

- [ ] **Step 2: 逐一删除文档中的引用**

- `CLAUDE.md`：删除 `OPENJAX_APPROVAL_POLICY` 环境变量条目
- `openjax-core/README.md`：更新示例代码，删除 `ApprovalPolicy::OnRequest` 参数
- `openjaxd/README.md`：删除测试环境变量注入说明

- [ ] **Step 3: 全量回归基线**

```bash
zsh -lc "cargo fmt -- --check"
zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"
zsh -lc "cargo test -p openjax-protocol"
zsh -lc "cargo test -p openjax-policy --tests"
zsh -lc "cargo test -p openjax-core --test policy_center_suite"
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core --test approval_events_suite"
zsh -lc "cargo test -p openjax-core --test approval_suite"
zsh -lc "cargo test -p openjax-core --test streaming_suite"
zsh -lc "cargo test -p openjax-core --test core_history_suite"
zsh -lc "cargo test -p tui_next --tests"
```

Expected: 全部通过

- [ ] **Step 4: 验证 DoD — 全仓无 ApprovalPolicy 残留**

```bash
grep -r "ApprovalPolicy\|approval_policy\|OPENJAX_APPROVAL_POLICY" \
  --include="*.rs" --include="*.toml" --include="*.md" \
  /Users/ericw/work/code/ai/openJax/openjax-core \
  /Users/ericw/work/code/ai/openJax/openjax-protocol \
  /Users/ericw/work/code/ai/openJax/openjax-policy \
  /Users/ericw/work/code/ai/openJax/openjaxd \
  /Users/ericw/work/code/ai/openJax/openjax-gateway \
  /Users/ericw/work/code/ai/openJax/ui/tui \
  | grep -v "docs/superpowers"
```

Expected: 无输出（`docs/superpowers` 内的规划文档除外）

- [ ] **Step 5: 最终 commit**

```bash
git add -p  # 逐文件确认
git commit -m "docs: 清理 approval_policy 文档残留，完成硬切

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## 快速参考：回归命令

```bash
# Phase 1 回归
zsh -lc "cargo test -p openjax-protocol"

# Phase 2 回归
zsh -lc "cargo test -p openjax-policy --tests"
zsh -lc "cargo test -p openjax-core --test policy_center_suite"
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core --test approval_events_suite"
zsh -lc "cargo test -p openjax-core --test approval_suite"

# 全量回归
zsh -lc "make core-full && cargo test -p tui_next --tests"
```
