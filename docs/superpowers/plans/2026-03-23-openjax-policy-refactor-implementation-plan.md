# OpenJax Policy Center Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 以独立 `openjax-policy` 模块统一 OpenJax 全工具权限决策，落地持久化规则、会话覆盖和热更新发布，并保持 TUI 默认直连 core。

**Architecture:** 新增 `openjax-policy` crate 承担规则模型、匹配引擎、版本运行时与审计元数据；`openjax-core` 在工具执行前构造 `PolicyInput` 请求决策；`openjax-gateway` 提供策略 CRUD、发布、会话 overlay API。M1 不改 TUI 传输链路，策略一致性通过共享 policy crate 实现。

**Tech Stack:** Rust 2024、Tokio、Serde、Axum、SQLite（沿用 gateway 持久化模式）、Cargo workspace tests。

---

## File Structure（先定边界）

### 新增文件

- `openjax-policy/Cargo.toml`
- `openjax-policy/src/lib.rs`
- `openjax-policy/src/schema.rs`
- `openjax-policy/src/engine.rs`
- `openjax-policy/src/store.rs`
- `openjax-policy/src/overlay.rs`
- `openjax-policy/src/runtime.rs`
- `openjax-policy/src/audit.rs`
- `openjax-policy/tests/policy_engine_suite.rs`
- `openjax-policy/tests/policy_runtime_suite.rs`
- `openjax-core/tests/policy_center_suite.rs`
- `openjax-gateway/tests/policy_api_suite.rs`

### 修改文件

- `Cargo.toml`（workspace members 增加 `openjax-policy`）
- `openjax-core/Cargo.toml`（依赖 `openjax-policy`）
- `openjax-core/src/tools/orchestrator.rs`
- `openjax-core/src/tools/context.rs`
- `openjax-core/src/tools/handlers/shell.rs`
- `openjax-core/src/sandbox/mod.rs`
- `openjax-gateway/Cargo.toml`（依赖 `openjax-policy`）
- `openjax-gateway/src/lib.rs`（注册新 policy 路由）
- `openjax-gateway/src/handlers/mod.rs`
- `openjax-gateway/src/handlers/session.rs`（会话 overlay 入口接线）
- `openjax-gateway/src/state/runtime.rs`
- `openjax-gateway/src/state/config.rs`（策略 runtime 初始化）
- `openjax-protocol/src/lib.rs`（审批/审计事件字段扩展）
- `openjax-core/src/tools/docs/extension-guide.md`
- `openjax-core/src/tools/docs/README.md`
- `AGENTS.md`（工具接入流程增加权限声明门禁）

### 说明

- 不修改 `ui/tui` 链路依赖：保持 `ui/tui -> openjax-core`。
- 不在本计划中引入 “TUI 走 gateway 可选模式”。

---

### Task 1: Scaffold `openjax-policy` Crate 与基础契约

**Files:**
- Create: `openjax-policy/Cargo.toml`
- Create: `openjax-policy/src/lib.rs`
- Create: `openjax-policy/src/schema.rs`
- Test: `openjax-policy/tests/policy_engine_suite.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: 写失败测试（schema 最小契约）**

```rust
#[test]
fn decision_has_required_metadata_fields() {
    let d = PolicyDecision::ask("rule-1", 1, "default ask");
    assert_eq!(d.matched_rule_id.as_deref(), Some("rule-1"));
    assert_eq!(d.policy_version, 1);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `zsh -lc "cargo test -p openjax-policy --test policy_engine_suite"`
Expected: FAIL（crate 或类型未定义）

- [ ] **Step 3: 写最小实现让测试通过**

```rust
pub enum DecisionKind { Allow, Ask, Deny, Escalate }
pub struct PolicyDecision {
    pub kind: DecisionKind,
    pub matched_rule_id: Option<String>,
    pub policy_version: u64,
    pub reason: String,
}
```

- [ ] **Step 4: 再跑测试确认通过**

Run: `zsh -lc "cargo test -p openjax-policy --test policy_engine_suite"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add Cargo.toml openjax-policy/Cargo.toml openjax-policy/src/lib.rs openjax-policy/src/schema.rs openjax-policy/tests/policy_engine_suite.rs
git commit -m "✨ feat(policy): scaffold openjax-policy schema and decision contract"
```

### Task 2: 实现规则匹配引擎（优先级/具体度/保守冲突）

**Files:**
- Create: `openjax-policy/src/engine.rs`
- Modify: `openjax-policy/src/schema.rs`
- Test: `openjax-policy/tests/policy_engine_suite.rs`

- [ ] **Step 1: 写失败测试（冲突裁决）**

```rust
#[test]
fn same_priority_conflict_prefers_safer_decision() {
    // allow 与 deny 同级冲突，应收敛到 deny
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `zsh -lc "cargo test -p openjax-policy --test policy_engine_suite same_priority_conflict_prefers_safer_decision"`
Expected: FAIL（匹配器未实现）

- [ ] **Step 3: 最小实现 engine**

```rust
pub fn decide(input: &PolicyInput, rules: &[PolicyRule], default: DecisionKind) -> PolicyDecision {
    // 1) 匹配规则 2) priority 排序 3) specificity 比较 4) 安全序收敛
}
```

- [ ] **Step 4: 全量 engine 测试**

Run: `zsh -lc "cargo test -p openjax-policy --test policy_engine_suite"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add openjax-policy/src/engine.rs openjax-policy/src/schema.rs openjax-policy/tests/policy_engine_suite.rs
git commit -m "✨ feat(policy): add deterministic rule matching engine"
```

### Task 3: 实现 runtime/store/overlay（发布与热更新）

**Files:**
- Create: `openjax-policy/src/store.rs`
- Create: `openjax-policy/src/overlay.rs`
- Create: `openjax-policy/src/runtime.rs`
- Create: `openjax-policy/src/audit.rs`
- Test: `openjax-policy/tests/policy_runtime_suite.rs`

- [ ] **Step 1: 写失败测试（in-flight 版本一致性）**

```rust
#[tokio::test]
async fn inflight_call_keeps_original_policy_version() {
    // publish 前取句柄；publish 后仍使用旧 version
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `zsh -lc "cargo test -p openjax-policy --test policy_runtime_suite inflight_call_keeps_original_policy_version"`
Expected: FAIL

- [ ] **Step 3: 实现 runtime 原子快照**

```rust
pub struct PolicyRuntime {
    current: ArcSwap<PolicySnapshot>,
}
```

- [ ] **Step 4: 增加 overlay 行为测试并通过**

Run: `zsh -lc "cargo test -p openjax-policy --test policy_runtime_suite"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add openjax-policy/src/store.rs openjax-policy/src/overlay.rs openjax-policy/src/runtime.rs openjax-policy/src/audit.rs openjax-policy/tests/policy_runtime_suite.rs
git commit -m "✨ feat(policy): add versioned runtime, store and session overlay"
```

### Task 4: 接入 `openjax-core` 统一决策入口（全工具）

**Files:**
- Modify: `openjax-core/Cargo.toml`
- Modify: `openjax-core/src/tools/context.rs`
- Modify: `openjax-core/src/tools/orchestrator.rs`
- Modify: `openjax-core/src/tools/handlers/shell.rs`
- Modify: `openjax-core/src/sandbox/mod.rs`
- Test: `openjax-core/tests/policy_center_suite.rs`

- [ ] **Step 1: 写失败测试（未声明工具默认 ask）**

```rust
#[tokio::test]
async fn unknown_tool_without_descriptor_defaults_to_ask() {
    // 调用 mock tool，断言触发审批
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `zsh -lc "cargo test -p openjax-core --test policy_center_suite"`
Expected: FAIL

- [ ] **Step 3: 接入 `PolicyDescriptor` 与 `PolicyInput` 构造**

```rust
let input = policy::PolicyInput::from_invocation(&invocation, descriptor);
let decision = policy_runtime.decide(input)?;
```

- [ ] **Step 4: 跑核心回归测试**

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test approval_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test approval_events_suite"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add openjax-core/Cargo.toml openjax-core/src/tools/context.rs openjax-core/src/tools/orchestrator.rs openjax-core/src/tools/handlers/shell.rs openjax-core/src/sandbox/mod.rs openjax-core/tests/policy_center_suite.rs
git commit -m "♻️ refactor(core): route all tool authorization through openjax-policy"
```

### Task 5: 接入 `openjax-gateway` 策略管理 API 与会话 overlay

**Files:**
- Modify: `openjax-gateway/Cargo.toml`
- Modify: `openjax-gateway/src/lib.rs`
- Modify: `openjax-gateway/src/handlers/mod.rs`
- Create: `openjax-gateway/src/handlers/policy.rs`
- Modify: `openjax-gateway/src/state/runtime.rs`
- Modify: `openjax-gateway/src/state/config.rs`
- Test: `openjax-gateway/tests/policy_api_suite.rs`

- [ ] **Step 1: 写失败测试（publish 后版本递增）**

```rust
#[tokio::test]
async fn publish_returns_incremented_policy_version() {
    // POST /policy/publish -> version+1
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `zsh -lc "cargo test -p openjax-gateway --test policy_api_suite publish_returns_incremented_policy_version"`
Expected: FAIL

- [ ] **Step 3: 实现 policy 路由与 handler**

```rust
POST /api/v1/policy/rules
PUT  /api/v1/policy/rules/:rule_id
DELETE /api/v1/policy/rules/:rule_id
POST /api/v1/policy/publish
PUT  /api/v1/sessions/:session_id/policy-overlay
```

- [ ] **Step 4: 运行 gateway 测试**

Run: `zsh -lc "cargo test -p openjax-gateway --test policy_api_suite"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add openjax-gateway/Cargo.toml openjax-gateway/src/lib.rs openjax-gateway/src/handlers/mod.rs openjax-gateway/src/handlers/policy.rs openjax-gateway/src/state/runtime.rs openjax-gateway/src/state/config.rs openjax-gateway/tests/policy_api_suite.rs
git commit -m "✨ feat(gateway): add policy management, publish and session overlay APIs"
```

### Task 6: 协议与审计字段对齐（可追溯）

**Files:**
- Modify: `openjax-protocol/src/lib.rs`
- Modify: `openjax-core/src/tools/orchestrator.rs`
- Modify: `openjax-gateway/src/event_mapper/approval.rs`
- Test: `openjax-core/tests/approval_events_suite.rs`

- [ ] **Step 1: 写失败测试（审批事件携带 version/rule_id）**

```rust
#[test]
fn approval_event_contains_policy_metadata() {
    // assert payload has policy_version and matched_rule_id
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `zsh -lc "cargo test -p openjax-core --test approval_events_suite"`
Expected: FAIL

- [ ] **Step 3: 最小实现字段透传**

```rust
ApprovalRequested { policy_version: Option<u64>, matched_rule_id: Option<String>, ... }
```

- [ ] **Step 4: 回归测试**

Run: `zsh -lc "cargo test -p openjax-core --test approval_events_suite"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add openjax-protocol/src/lib.rs openjax-core/src/tools/orchestrator.rs openjax-gateway/src/event_mapper/approval.rs
git commit -m "✨ feat(protocol): include policy version and rule id in approval events"
```

### Task 7: 文档与接入流程更新（权限声明门禁）

**Files:**
- Modify: `openjax-core/src/tools/docs/extension-guide.md`
- Modify: `openjax-core/src/tools/docs/README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: 写文档验收清单（先写测试式条目）**

```md
- 新工具必须实现 PolicyDescriptor
- 新工具必须覆盖 allow/ask|escalate/deny 三类测试
- 缺失权限声明不视为接入完成
```

- [ ] **Step 2: 人工校验文档一致性**

Run: `zsh -lc "rg -n \"PolicyDescriptor|接入完成|权限声明\" AGENTS.md openjax-core/src/tools/docs/extension-guide.md openjax-core/src/tools/docs/README.md"`
Expected: 三个文件均命中核心约束

- [ ] **Step 3: 提交**

```bash
git add AGENTS.md openjax-core/src/tools/docs/extension-guide.md openjax-core/src/tools/docs/README.md
git commit -m "📝 docs(tools): enforce permission declaration as tool onboarding gate"
```

### Task 8: 全量验证与收尾

**Files:**
- Modify: `docs/superpowers/specs/2026-03-23-openjax-policy-refactor-design.md`（如需补充落地状态）

- [ ] **Step 1: 运行格式化与 lint**

Run: `zsh -lc "cargo fmt -- --check"`
Expected: PASS

Run: `zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"`
Expected: PASS

- [ ] **Step 2: 运行关键测试矩阵**

Run: `zsh -lc "cargo test -p openjax-policy --tests"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test policy_center_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test approval_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test approval_events_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-gateway --test policy_api_suite"`
Expected: PASS

- [ ] **Step 3: 发布总结提交**

```bash
git add .
git commit -m "✅ chore(policy): complete policy-center migration for tool authorization"
```

---

## 执行注意事项

1. 若任一步骤出现测试红灯，先使用 `@superpowers/systematic-debugging` 定位根因，再继续推进。
2. 不做 spec 外扩展（YAGNI），尤其不引入 TUI 经 gateway 的新链路。
3. 文件若接近 800 行，优先在同任务内拆分职责，避免继续膨胀。
4. 每个任务完成后都要留下可验证证据（命令 + 结果）。

