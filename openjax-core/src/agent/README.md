# agent 模块

`openjax-core/src/agent/` 负责 `Agent` 的生命周期、单回合执行、规划循环、工具调用编排与事件输出。

## 目录与职责

- `bootstrap.rs`: `Agent::new/with_config/with_runtime/with_config_and_runtime`，组装模型客户端、工具路由与运行时策略。
- `turn.rs`: `submit`/`submit_with_sink` 入口，分发工具直调模式或自然语言规划模式。
- `planner.rs`: 自然语言回合主循环，驱动 `planner -> tool/final` 决策，含 JSON 修复、重复调用保护、final writer。
- `execution.rs`: 单工具调用执行、重试、实时工具事件透传。
- `decision.rs`: 解析并规范化模型决策 JSON。
- `prompt.rs`: 构造 planner/final/repair prompt。
- `runtime_policy.rs`: 解析审批与沙箱策略，优先级为环境变量 > 配置文件 > 默认值。
- `state.rs`: 历史记录、限速、重复工具调用检测。
- `events.rs`: 统一事件推送（写入返回 `Vec<Event>` + 可选 sink）。
- `lifecycle.rs`: `thread_id/depth/parent` 与子 Agent 生成（保留扩展位）。
- `tool_guard.rs`: `apply_patch` 成功/失败后强制 `read_file` 的防护规则。
- `tool_policy.rs`: 重复调用、审批阻断等策略文案与判定函数。

## 回合执行流程

1. `submit` 收到 `Op::UserTurn` 后写入用户历史，并发出 `TurnStarted`。
2. 若输入匹配 `tool:<name> key=value`，走 `execute_single_tool_call`。
3. 否则进入 `execute_natural_language_turn`：
4. 生成 planner prompt，调用模型获得 JSON 决策（必要时进行一次 JSON repair）。
5. 决策为 `tool` 时调用工具并记录 trace；决策为 `final` 时输出最终回复。
6. 发出 `TurnCompleted` 并返回本回合事件序列。

### Skills 注入链路

1. `bootstrap.rs` 启动时加载 `SkillRegistry` 和 `SkillRuntimeConfig`。
2. `planner.rs` 每轮根据 `user_input` 匹配 top-N skills。
3. `prompt.rs` 在 planner prompt 中注入 `Available skills (auto-selected)` 上下文。
4. Skills 失败仅记录 warning，不会中断主回合执行。

## 关键约束

- 每回合最多工具调用次数：`MAX_TOOL_CALLS_PER_TURN = 5`。
- 每回合最多 planner 轮数：`MAX_PLANNER_ROUNDS_PER_TURN = 10`。
- 连续重复调用跳过上限：`MAX_CONSECUTIVE_DUPLICATE_SKIPS = 2`。
- 历史窗口上限：`MAX_CONVERSATION_HISTORY_ITEMS = 20`。
- 默认请求限速：模型请求间隔最少 `1000ms`。

## 事件语义

常见顺序：

1. `TurnStarted`
2. `ToolCallStarted`（命中工具时）
3. `ApprovalRequested` / `ApprovalResolved`（按策略触发）
4. `ToolCallCompleted` 或 `AssistantMessage`
5. `TurnCompleted`

`submit_with_sink` 会在返回 `Vec<Event>` 的同时，将同样的事件流实时推送到外部 sink。

## 策略来源

- 审批策略：`OPENJAX_APPROVAL_POLICY` 或 `config.sandbox.approval_policy`。
- 沙箱模式：`OPENJAX_SANDBOX_MODE` 或 `config.sandbox.mode`。
- Final writer 开关：`OPENJAX_FINAL_WRITER`（默认 `off`，即 planner-only）。

## 扩展建议

- 增加新回合阶段时，优先在 `planner.rs` 维护状态机，避免在 `turn.rs` 膨胀逻辑。
- 新增事件类型后，确保 `events.rs` 的 sink 与返回路径保持一致。
- 任何工具策略变更，应同步更新 `tool_guard.rs` / `tool_policy.rs` 与相关集成测试。
