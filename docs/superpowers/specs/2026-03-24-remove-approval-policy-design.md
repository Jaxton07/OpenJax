# 设计文档：移除 approval_policy，审批决策统一收敛到 Policy Center

- 日期：2026-03-24
- 状态：已审查
- 路径：`docs/superpowers/specs/2026-03-24-remove-approval-policy-design.md`

---

## 1. 背景与问题

当前系统存在两套并行的审批决策机制：

1. **运行时审批策略**：`ApprovalPolicy::AlwaysAsk | OnRequest | Never`，通过 `OPENJAX_APPROVAL_POLICY` 环境变量或 `config.toml` 配置，作为全局闸门叠加在工具决策之上
2. **Policy Center 决策**：`DecisionKind::allow | ask | escalate | deny`，基于规则匹配，支持版本化和 session overlay

并行机制带来三个问题：

- 决策路径分叉：同一工具调用的最终结果可能来自两个不同层，难以预测
- Shell 工具例外：`orchestrator.rs` 中 shell 工具完全绕过 policy center，走独立的 sandbox 路径，导致"单一决策源"目标无法实现
- degrade 闭环缺失：sandbox 后端失败后的提权审批路径不受 policy center 控制

## 2. 目标

1. **删除 `ApprovalPolicy`**：彻底移除该类型、所有运行时字段、环境变量、配置项
2. **Shell 工具统一纳入 policy center**：所有工具（含 shell）走同一决策入口
3. **degrade 提权闭环**：sandbox 执行失败后，带 `sandbox_degrade` 风险标签重查 policy center，由 policy center 决定是否触发提权审批
4. **新增 `approval_kind` 字段**：`Event::ApprovalRequested` 中携带 `Normal | Escalation`，供 UI 和审计使用
5. **消除架构污点**：删除 `merge_policy_center_outcome()`、本地 `PolicyDecision` 枚举等二义性结构

## 3. 目标状态语义

### 3.1 决策源

唯一决策源：Policy Center（`openjax-policy::DecisionKind`）

| 决策 | 语义 |
|------|------|
| `allow` | 直接进入 sandbox 执行 |
| `ask` | 发 ApprovalRequested { approval_kind: Normal }，等待审批 |
| `escalate` | 发 ApprovalRequested { approval_kind: Escalation }，等待提权审批 |
| `deny` | 立即返回错误，不可被审批放行 |

### 3.2 职责边界

- `sandbox_mode`：仅负责执行隔离边界（`workspace_write` / `danger_full_access`），不参与审批决策
- `deny` 始终最高优先级，`sandbox_mode = danger_full_access` 不可绕过 `deny`
- 无规则命中时默认 `ask`（由 `PolicyStore::default_decision` 控制，现有行为保留）

### 3.3 Shell 工具风险标签提取

Shell 命令分析逻辑从决策函数重构为风险标签提取器 `extract_shell_risk_tags(command, require_escalated) -> Vec<String>`。提取结果填入 `PolicyInput.risk_tags`，由 policy center 做最终决策。

| 命令特征 | risk_tags |
|---------|-----------|
| `rm -rf /`、`mkfs`、`dd if=` 等 | `["destructive"]` |
| `sudo ` | `["privilege_escalation"]` |
| `require_escalated: true` | `["require_escalated"]` |
| `curl`、`wget` 等 | `["network"]` |
| `>`、`tee`、`git commit` 等 | `["fs_write"]` |

**`destructive` 标签的处理**：policy center 在 `PolicyStore` 初始化时内置一条系统级规则：

```
id: "system:destructive_escalate"
risk_tags_all: ["destructive"]
decision: Escalate
priority: 1000   // 高于所有用户规则
```

这确保了 `rm -rf /` 等高危命令**始终触发提权审批**（用户可批准或拒绝），而非一刀切禁止——合法的高危操作仍可在用户确认后执行。此规则在 `openjax-policy::PolicyStore::new()` 中内置，不依赖调用方传入。

**关于沙箱先行（sandbox-first）**：理想的长期架构是"先让沙箱执行，被拦截后再触发审批"，而非预扫描命令。macOS 上 Anthropic 的 `sandbox-runtime` 已通过 `startMacOSSandboxLogMonitor` 实现了实时 violation 日志监听，技术上可行；Linux 因 seccomp 返回 EPERM 与普通错误混淆，需要额外工程。**本次不做**，作为独立后续任务规划（云端部署场景下可评估 Docker 容器作为沙箱后端）。

**`policy_descriptor()` shell 分支的处理**：`ToolInvocation::policy_descriptor()` 中的 shell 分支（当前只提取 `require_escalated` 一个标签）在 Phase 2.2 中废弃并删除。shell 工具的 `PolicyInput` 完全由 orchestrator 统一路径中的 `extract_shell_risk_tags` 构造，不再走 `policy_descriptor()` 路径。

## 4. 执行序列（方案 A：协议先行）

### Phase 1：openjax-protocol 变更

**目标**：additive 变更，不破坏现有编译和测试

新增枚举：
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalKind {
    Normal,      // DecisionKind::Ask 触发
    Escalation,  // DecisionKind::Escalate 触发（含 degrade 场景）
}
```

`Event::ApprovalRequested` 新增字段：
```rust
pub approval_kind: Option<ApprovalKind>,
// Option 保证向后兼容，None 语义等同于 Normal
```

写入规则：

| 场景 | 写入方 | 值 |
|------|--------|-----|
| orchestrator ask | orchestrator.rs | `Some(Normal)` |
| orchestrator escalate | orchestrator.rs | `Some(Escalation)` |
| degrade escalate | degrade.rs | `Some(Escalation)` |

**同步更新 `approval_events_suite`**：Phase 1 完成后，立即在 `approval_events_suite` 中为现有审批事件测试增加 `approval_kind` 字段断言，确保新字段被正确填充。

**回归**：
```bash
cargo test -p openjax-protocol
cargo test -p openjax-core --test approval_events_suite
```

---

### Phase 2：openjax-core 硬切

#### 2.1 删除清单

| 文件 | 删除内容 |
|------|---------|
| `tools/context.rs` | `ApprovalPolicy` 枚举（含 `from_env`、`as_str`）、`ToolTurnContext.approval_policy` 字段 |
| `tools/router.rs` | `ToolRuntimeConfig.approval_policy`、`Default` 与 `with_config` 中的初始化 |
| `tools/tool_builder.rs` | `CreateToolInvocationParams.approval_policy`、`create_tool_invocation` 中的赋值 |
| `tools/orchestrator.rs` | `merge_policy_center_outcome()`、`map_policy_center_decision()`、`decision_rank()`、`approval_reason(..., approval_policy)` 中的 `approval_policy` 分支 |
| `sandbox/policy.rs` | 本地 `PolicyDecision` 枚举（替换为 `DecisionKind`）、`evaluate_tool_invocation_policy` 中 `approval_policy` overlay 块（原 lines 102-139）、shell 分支死代码（原 lines 93-101） |
| `sandbox/degrade.rs` | `ApprovalPolicy::Never` 检查逻辑 |
| `agent/bootstrap.rs` | `with_runtime(approval_policy, ...)` 签名去掉 `approval_policy` 参数、`approval_policy_name()`、`with_config_and_runtime` 中的参数 |
| `agent/lifecycle.rs` | `spawn_sub_agent()` 中 `approval_policy` 传递 |
| `agent/runtime_policy.rs` | `parse_approval_policy()`、`resolve_approval_policy()`、`OPENJAX_APPROVAL_POLICY` 读取 |
| `config.rs` | `SandboxConfig.approval_policy` 字段、`DEFAULT_CONFIG_TEMPLATE` 中 `approval_policy = "on_request"` 行 |
| `lib.rs` | `pub use tools::ApprovalPolicy` |
| `tools/system/process_snapshot.rs` | 测试构造 `ToolTurnContext` 中的 `approval_policy` 字段 |
| `tools/system/system_load.rs` | 测试构造 `ToolTurnContext` 中的 `approval_policy` 字段 |
| `tools/system/disk_usage.rs` | 测试构造 `ToolTurnContext` 中的 `approval_policy` 字段 |

#### 2.2 Shell 工具接入 policy center

**orchestrator.rs 重构**：

删除 `if !is_shell_like_tool(...)` 分支结构，改为统一路径：

```
所有工具
  └─ build_policy_input(invocation)
       ├─ shell 工具：调 extract_shell_risk_tags(command, require_escalated) → Vec<String>
       │              构造 PolicyInput { tool_name, action: "exec", risk_tags, ... }
       │              （废弃 policy_descriptor() 的 shell 分支，不再调用）
       └─ non-shell：用 PolicyDescriptor 构造 PolicyInput（现有逻辑保留）
  └─ policy_center.decide(input) → DecisionKind
  └─ deny  → 返回错误
  └─ ask/escalate → emit ApprovalRequested { approval_kind } → 等待审批
  └─ allow / approved → sandbox 执行
```

**审批复用（`approved_mutating_turns`）**：shell 工具进入统一路径后，`should_reuse_mutating_approval` 和 `record_mutating_approval` 中的 `is_shell_like_tool` 排除检查**保留**——shell 命令每次内容不同，不复用审批。删除 `is_shell_like_tool` 的仅限于 orchestrator 的路由 gate，不涉及复用逻辑。

**sandbox/policy.rs 重构**：

`decide_shell_policy()` 重命名为 `extract_shell_risk_tags()`，签名变更：

```rust
// 旧签名（决策函数）：
fn decide_shell_policy(sandbox_policy, command, require_escalated, capabilities, risks) -> PolicyDecision

// 新签名（风险提取器）：
pub fn extract_shell_risk_tags(command: &str, require_escalated: bool) -> Vec<String>
```

返回 `Vec<String>` 风险标签，不做决策。同时删除 `detect_capabilities()`、`analyze_shell_invocation()` 等仅服务于决策逻辑的辅助函数（capabilities 检测的结果直接映射为 risk_tags）。

**`ToolInvocation::policy_descriptor()` 调整**：删除 shell 工具分支（`"shell" | "exec_command"` 的匹配臂），shell 工具的 PolicyInput 由 orchestrator 统一构造。

**本地 `PolicyDecision` 替换**：

`sandbox/policy.rs`、`PolicyTrace`、`PolicyOutcome` 中所有 `PolicyDecision` 字段替换为 `openjax_policy::schema::DecisionKind`：
- `PolicyDecision::Allow` → `DecisionKind::Allow`
- `PolicyDecision::AskApproval` → `DecisionKind::Ask`
- `PolicyDecision::AskEscalation` → `DecisionKind::Escalate`
- `PolicyDecision::Deny` → `DecisionKind::Deny`

`map_policy_center_decision()` 因映射不再需要而删除。

#### 2.3 openjax-policy：内置系统规则 + 暴露 default_decision 接口

在 `openjax-policy::PolicyStore::new()` 中内置系统级 `destructive → escalate` 规则：

```rust
const SYSTEM_RULE_DESTRUCTIVE_ESCALATE: PolicyRule = PolicyRule {
    id: "system:destructive_escalate",
    decision: DecisionKind::Escalate,
    priority: 1000,
    risk_tags_all: vec!["destructive"],
    // 其余字段为 None/空，匹配所有工具
};
```

该规则在 `PolicyStore::new()` 时自动插入，不可被 session overlay 覆盖（overlay 机制优先级上限低于 1000）。`destructive` 命令触发提权审批而非直接拒绝，用户确认后可执行。

在 `PolicyHandle`（`openjax-policy/src/runtime.rs`）上新增方法：

```rust
pub fn default_decision(&self) -> DecisionKind {
    self.snapshot.store.default_decision.clone()
}
```

此方法供 `Agent::policy_default_decision_name()` 调用（Phase 3）。

#### 2.4 degrade 路径重设计（degrade.rs）

```rust
pub async fn request_degrade_approval(
    invocation: &ToolInvocation,
    command: &str,
    backend: &str,
    reason: &str,
) -> Result<bool, FunctionCallError> {
    // 1. 带 sandbox_degrade 风险标签重查 policy center
    //    直接使用 invocation.turn.policy_runtime（无需新增参数）
    let decision = query_policy_center_for_degrade(invocation, command);

    // 2. deny → 直接返回错误，不走审批
    if matches!(decision.kind, DecisionKind::Deny) {
        return Err(FunctionCallError::Internal(format!(
            "sandbox backend unavailable and policy denied degrade: {backend} {reason}"
        )));
    }

    // 3. escalate（含 ask fallback）→ 走审批流
    //    emit ApprovalRequested { approval_kind: Some(Escalation) }
    //    超时/拒绝 → 返回错误
    //    通过 → Ok(true)
}
```

`query_policy_center_for_degrade()` 内部构造 `PolicyInput`：
- `risk_tags`: 原有命令 risk_tags + `["sandbox_degrade"]`
- 若 `policy_runtime` 为 None，沿用现有 fallback（临时 runtime，默认 `Ask`）：此时 degrade 一律触发 `Escalation` 审批，不会静默 deny 或 allow

**回归**：
```bash
cargo test -p openjax-policy --tests
cargo test -p openjax-core --test policy_center_suite
cargo test -p openjax-core --test tools_sandbox_suite
cargo test -p openjax-core --test approval_events_suite
cargo test -p openjax-core --test approval_suite
```

---

### Phase 3：TUI 适配

**前提**：Phase 2.3 中 `PolicyHandle::default_decision()` 方法已暴露。

**Agent 新增方法**（`agent/bootstrap.rs`）：
```rust
pub fn policy_default_decision_name(&self) -> &'static str {
    self.policy_runtime
        .as_ref()
        .map(|r| r.handle().default_decision().as_str())
        .unwrap_or("ask")  // 无 runtime 时与 orchestrator fallback 一致
}
```

**`set_runtime_info()` 签名变更**（`tui/src/app/mod.rs`）：
```rust
// 旧：fn set_runtime_info(model, approval_policy, sandbox_mode, cwd)
// 新：fn set_runtime_info(model, policy_default, sandbox_mode, cwd)
```

**`tui/src/runtime.rs` 调用改为**：
```rust
app.set_runtime_info(
    guard.model_backend_name().to_string(),
    guard.policy_default_decision_name().to_string(),  // 展示: "policy: ask"
    guard.sandbox_mode_name().to_string(),
    cwd.as_path(),
);
```

**`tui/src/state/app_state.rs`**：字段 `approval_policy` → `policy_default`，UI 展示从 `approval_policy: on_request` 变为 `policy: ask`。

**回归**：`cargo test -p tui_next --tests`

---

### Phase 4：文档清理 + 全量回归

**删除所有 `approval_policy` 文档引用**：
- `CLAUDE.md`（`OPENJAX_APPROVAL_POLICY` 环境变量条目）
- `openjax-core/README.md`（示例代码中的 `ApprovalPolicy::OnRequest`）
- `openjax-core/src/agent/README.md`
- `openjaxd/README.md`
- `docs/config.md`
- `openjax-core/src/tools/docs/` 下相关文档

**增补 policy center 决策说明**：工具接入必须声明 policy descriptor，新工具必须覆盖 `allow/ask/escalate/deny` 四个决策路径的测试。

**全量回归基线**：
```bash
cargo fmt -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p openjax-protocol
cargo test -p openjax-policy --tests
cargo test -p openjax-core --test policy_center_suite
cargo test -p openjax-core --test tools_sandbox_suite
cargo test -p openjax-core --test approval_events_suite
cargo test -p openjax-core --test approval_suite
cargo test -p openjax-core --test streaming_suite
cargo test -p openjax-core --test core_history_suite
cargo test -p tui_next --tests
```

## 5. 新增测试覆盖（policy_center_suite 扩展）

| 测试用例 | 断言 |
|---------|------|
| shell + `allow` 规则 → 直接执行 | 无 `ApprovalRequested` 事件 |
| shell + `ask` 规则 → 触发审批 | `ApprovalRequested { approval_kind: Some(Normal) }` |
| shell + `escalate` 规则 → 触发提权 | `ApprovalRequested { approval_kind: Some(Escalation) }` |
| shell + `deny` 规则 → 立即失败 | 返回错误，无审批事件 |
| shell + `destructive` 命令 → escalate | `system:destructive_escalate` 触发，`ApprovalRequested { approval_kind: Some(Escalation) }` |
| degrade 场景 → policy escalate | `ApprovalRequested { approval_kind: Some(Escalation) }` |
| degrade 场景 → policy deny | 返回错误，无审批事件 |
| degrade + policy_runtime=None | 触发 Escalation 审批（fallback Ask → Escalation） |
| `approval_kind` 字段在所有 ask 审批事件中存在 | `approval_kind: Some(Normal)`，不为 None |
| `approval_kind` 字段在所有 escalate 审批事件中存在 | `approval_kind: Some(Escalation)`，不为 None |

## 6. 影响范围

- **openjax-protocol**：新增 `ApprovalKind` 枚举，`Event::ApprovalRequested` 加字段
- **openjax-policy**：内置 `system:destructive_deny` 系统规则，`PolicyHandle` 新增 `default_decision()` 方法
- **openjax-core**：约 19 个生产文件（含 `tools/system/` 下 3 个）、约 16 个测试文件需修改
- **ui/tui**：3 个文件（`runtime.rs`、`app/mod.rs`、`state/app_state.rs`）
- **openjaxd**：删除测试中 `OPENJAX_APPROVAL_POLICY` 注入
- **文档**：全仓约 189 处命中，其中文档约 150 处

## 7. 完成判定（DoD）

**代码层**：
- 全仓无 `ApprovalPolicy` 类型定义与调用
- 全仓无 `approval_policy` 运行时字段
- 全仓无 `OPENJAX_APPROVAL_POLICY`
- 无本地 `PolicyDecision` 枚举（已替换为 `DecisionKind`）
- 无 `merge_policy_center_outcome()` 函数
- `openjax-policy::PolicyStore` 内置 `system:destructive_escalate` 系统规则

**行为层**：
- Shell 工具审批行为由 policy center 驱动（有测试验证）
- `destructive` 命令被系统规则触发提权审批（`Escalation`），用户确认后可执行（有测试验证）
- degrade 场景可触发 `Escalation` 审批（有测试验证）
- `approval_kind` 字段在所有审批事件中正确填充

**质量层**：
- `fmt / clippy / tests` 全绿
- `approval_events_suite` 断言 `approval_kind` 字段

## 8. 风险与注意事项

1. **测试编译批量失败**：约 16 个测试文件 + 3 个生产文件（`tools/system/` 下）构造 `ToolTurnContext` 时包含 `approval_policy` 字段，Phase 2 删除后需批量修复
2. **shell 工具行为变化**：现有依赖 `ApprovalPolicy::Never` 绕过审批的测试需要重构为 policy center `allow` 规则驱动
3. **degrade 路径测试缺失**：当前无 degrade 提权审批的集成测试，Phase 2 必须补齐
4. **`system:destructive_escalate` 规则不可被 session overlay 覆盖**：overlay 优先级机制需确认上限低于 1000，否则用户可能意外绕过该规则（变为 allow）
5. **不允许新增兼容分支或临时 fallback**：本次为硬切，中间态编译失败是预期的
