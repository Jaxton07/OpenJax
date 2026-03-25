# openjax-policy

`openjax-policy` 是 OpenJax 的统一策略中心模块，负责把“工具权限判断”收敛为一套可复用、可审计、可版本化的运行时能力。

## 1. 目标与职责

- 提供统一的策略输入/规则/决策模型（schema）
- 提供确定性规则匹配引擎（engine）
- 提供版本化策略运行时（runtime）
- 支持按 `session_id` 的会话级 overlay（overlay）
- 提供审计记录结构（audit）

## 2. 模块结构

- `src/schema.rs`
  - `PolicyInput`：策略输入（工具名、action、session、capabilities、risk_tags 等）
  - `PolicyRule`：规则定义（匹配条件 + decision + priority + reason）
  - `PolicyDecision`：决策结果（`kind`、`matched_rule_id`、`policy_version`、`reason`）
  - `DecisionKind`：`allow | ask | escalate | deny`
- `src/engine.rs`
  - `decide(...)`：规则匹配主入口
  - 优先级规则：`priority` -> `specificity` -> 保守性（`allow < ask < escalate < deny`）-> `rule_id` 稳定 tie-break
- `src/runtime.rs`
  - `PolicyRuntime`：版本化快照运行时
  - `publish(...)`：发布新策略版本
  - `set_session_overlay(...)` / `clear_session_overlay(...)`：会话 overlay 管理
  - `handle().decide(...)`：在快照一致性下执行决策
- `src/store.rs`
  - `PolicyStore`：默认决策 + 规则集合
- `src/overlay.rs`
  - `SessionOverlay` 与 overlay map
- `src/audit.rs`
  - `PolicyAuditRecord`：用于记录 `policy_version` / `matched_rule_id` / `reason`

## 3. 与其他模块关系

- `openjax-gateway`
  - 通过 policy API 管理 draft/publish/overlay
  - 在 turn 执行前将当前 `PolicyRuntime` 注入 `openjax-core`
- `openjax-core`
  - 工具执行时读取注入的 runtime 做策略决策
  - 审批事件透传 `policy_version` 与 `matched_rule_id`

## 4. 决策流程

### 4.1 流水线概览

```
ToolInvocation
    │
    ▼
ToolOrchestrator::run()
    │
    ├─ [1] evaluate_policy_center_decision(invocation)
    │       │
    │       ├─ shell 工具：先 extract_shell_risk_tags() 注入命令级标签到 descriptor
    │       │
    │       ├─ 有 policy_runtime → PolicyHandle::decide(input)
    │       │       ├─ session overlay 规则优先匹配
    │       │       └─ 全局 store 规则 → default_decision（兜底）
    │       │
    │       └─ 无 policy_runtime（fallback）
    │               ├─ has_risk = true  → Ask（临时 runtime）
    │               └─ has_risk = false → Allow rule for this tool
    │
    ├─ [2] Deny → 立即返回 Err
    │
    ├─ [3] Ask / Escalate → 发 ApprovalRequested 事件，等待用户审批
    │       ├─ Ask      → ApprovalKind::Normal
    │       └─ Escalate → ApprovalKind::Escalation
    │
    └─ [4] Allow → 直接进入沙箱执行
```

### 4.2 规则匹配优先级

```
1. session overlay 规则（会话级，优先级最高）
       ↓ 无匹配
2. 全局 store 规则（按 priority 降序，高值优先）
       ↓ 无匹配
3. default_decision（Ask / Allow / Deny，PolicyStore 构造时传入）
```

`PolicyRule` 匹配条件（所有非 `None` 字段全部满足才算命中）：

- `tool_name`、`action`、`session_id`、`actor`、`resource`（精确匹配）
- `capabilities_all`：`input.capabilities` 必须包含规则所列全部能力
- `risk_tags_all`：`input.risk_tags` 必须包含规则所列全部标签

### 4.3 内置系统规则

| 规则 ID | priority | decision | 触发条件 |
|---------|----------|----------|---------|
| `system:destructive_escalate` | **1000**（最高） | `Escalate` | `risk_tags` 含 `"destructive"` |

该规则在 `PolicyStore::new()` 时自动插入，不依赖调用方配置。

## 5. 工具权限声明（PolicyDescriptor）

每个工具在 `ToolInvocation::policy_descriptor()` 中声明自身的静态属性，Policy Center 以此构造 `PolicyInput`：

| 工具 | action | capabilities | risk_tags（初始） | 默认行为 |
|------|--------|-------------|-------------------|----------|
| `read_file` / `list_dir` / `grep_files` | `read` | `[fs_read]` | `[]` | **Allow** |
| `process_snapshot` / `system_load` / `disk_usage` | `observe` | `[process_exec]` | `[]` | **Allow** |
| `apply_patch` / `edit_file_range` | `write` | `[fs_write]` | `[mutating]` | **Ask** |
| `shell` / `exec_command` | `exec` | `[process_exec]` | `[]`（命令级动态注入） | 见下节 |
| 未知工具（无 descriptor） | `invoke` | `[]` | `[unknown_tool_descriptor]` | **Ask** |

## 6. Shell 工具的命令级风险分析

Shell 的 descriptor 初始 `risk_tags=[]`，orchestrator 调用 `extract_shell_risk_tags()` 分析命令内容后动态注入：

| 命令示例 | 注入的 risk_tags | 最终决策 |
|---------|-----------------|---------|
| `ls`, `ps`, `echo` 等 | `[]` | **Allow** |
| `curl …` / `wget …` | `[network]` | **Ask** |
| `git commit`, `rm -rf /tmp/xxx`, `cp`, `mv` 等 | `[write]` | **Ask** |
| `sudo …` | `[privilege_escalation]` | **Ask** |
| `require_escalated=true` 参数 | `[require_escalated]` | **Ask** |
| `rm -rf /`, `mkfs`, `dd if=`, `:(){:\|:&};:` | `[destructive]` | **Escalate**（`system:destructive_escalate` 规则触发） |

## 7. 无 `policy_runtime` 时的 Fallback

| 情形 | has_risk | 最终决策 |
|------|----------|---------|
| 无 descriptor（未知工具） | `true`（视为有风险） | Ask |
| descriptor 有 risk_tags（如 `mutating`、`network`、`write`） | `true` | Ask |
| descriptor 无 risk_tags（只读/只观测工具，或安全 shell 命令） | `false` | Allow |

## 8. 审批复用机制

- **非 shell 的 mutating 工具**（如 `apply_patch`）：同一 turn 内首次审批通过后，后续同 turn 内的调用自动复用，无需再次弹出审批。
- **shell 工具**：每次调用都独立审批，不复用。

## 9. 本地开发与验证

在仓库根目录运行：

```bash
zsh -lc "cargo build -p openjax-policy"
zsh -lc "cargo test -p openjax-policy --tests"
```

## 10. 设计约束

- 规则匹配必须确定性，不依赖输入顺序
- 决策结果必须可审计（含版本号与命中规则）
- 默认策略采用保守语义（未命中时遵循 default decision）
- 会话 overlay 只影响目标会话，不影响全局策略
