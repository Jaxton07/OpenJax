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

## 4. 决策流程（简版）

1. 构造 `PolicyInput`
2. 命中会话 overlay（若 `session_id` 存在）
3. 若 overlay 无命中则回落到全局规则
4. 无规则命中时采用 `PolicyStore.default_decision`
5. 输出 `PolicyDecision`，附带版本与命中规则

## 5. 本地开发与验证

在仓库根目录运行：

```bash
zsh -lc "cargo build -p openjax-policy"
zsh -lc "cargo test -p openjax-policy --tests"
```

## 6. 设计约束

- 规则匹配必须确定性，不依赖输入顺序
- 决策结果必须可审计（含版本号与命中规则）
- 默认策略采用保守语义（未命中时遵循 default decision）
- 会话 overlay 只影响目标会话，不影响全局策略
