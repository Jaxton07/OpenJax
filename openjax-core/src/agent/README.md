# agent 模块

`openjax-core/src/agent/` 负责 `Agent` 的生命周期、单回合执行、规划循环、工具调用编排与事件输出。

## 目录与职责

- `bootstrap.rs`: `Agent::new/with_config/with_runtime/with_config_and_runtime`，组装模型客户端、工具路由与运行时策略。
- `context_compressor.rs`: 上下文压缩模块，负责 history 分割与 LLM 摘要生成。
- `turn.rs`: `submit`/`submit_with_sink` 入口，分发工具直调模式或自然语言规划模式。
- `planner.rs`: Native Tool Calling 回合主循环，基于 `ModelResponse.content` 驱动 `tool_use/final` 收敛。
- `planner_stream_flow.rs`: planner 阶段流式编排（stream request、delta 处理、工具事件发射、synthetic delta 发射）。
- `planner_tool_action.rs`: 单工具调用分支编排（重复调用保护、guard 拦截、执行与失败收敛），当前保持独立模块。
- `planner_tool_batch.rs`: `tool_batch` 执行调度（依赖图、并发执行、批次完成统计与错误事件）。
- `planner_utils.rs`: planner 复用的工具函数与策略辅助（trace 文本、tool error 分类、git diff 策略判定）。
- `execution.rs`: 单工具调用执行、重试、实时工具事件透传。
- `decision.rs`: 旧 JSON planner 解析辅助（保留为非主路径兼容/测试支持，不是默认执行路径）。
- `prompt.rs`: 构造 native loop 所需的 system prompt 与消息辅助文案。
- `runtime_policy.rs`: 解析审批与沙箱策略，优先级为环境变量 > 配置文件 > 默认值。
- `state.rs`: 历史记录、限速、重复工具调用检测。
- `events.rs`: 统一事件推送（写入返回 `Vec<Event>` + 可选 sink）。
- `lifecycle.rs`: `thread_id/depth/parent` 与子 Agent 生成（保留扩展位）。
- `tool_guard.rs`: `apply_patch` 成功/失败后强制 `read_file` 的防护规则。
- `tool_policy.rs`: 重复调用、审批阻断等策略文案与判定函数。

## 回合执行流程

1. `submit` 收到 `Op::UserTurn` 后写入用户历史，并发出 `TurnStarted`。
2. 若输入匹配 `tool:<name> key=value`，走 `execute_single_tool_call`。
3. 否则进入 `execute_natural_language_turn`（native tool calling 主循环）：
4. 基于 `build_system_prompt + build_turn_messages` 构造请求，调用 `planner_stream_flow` 获取 `ModelResponse.content`。
5. 若模型返回 `tool_use`，走 `planner_tool_action` 执行并将 `tool_result` 回写到对话消息；若无 `tool_use`，输出最终回复。
6. 发出 `TurnCompleted` 并返回本回合事件序列。

### Skills 注入链路

1. `bootstrap.rs` 启动时加载 `SkillRegistry` 和 `SkillRuntimeConfig`。
2. `planner.rs` 每轮根据 `user_input` 匹配 top-N skills。
3. `prompt.rs` 在 system prompt 中注入 `Available skills (auto-selected)` 上下文。
4. Skills 失败仅记录 warning，不会中断主回合执行。

## 非主路径说明

- Phase 3 之后，`openjax-core` 默认不再走旧 JSON planner 主链路。
- `decision.rs` 与 `dispatcher/` 的旧解析辅助仍可保留用于历史对照或测试语义，不应被视为当前默认执行入口。

## 关键约束

- 每回合最多工具调用次数（默认）：`10`（可配置）。
- 每回合最多 planner 轮数（默认）：`20`（可配置）。
- 连续重复调用跳过上限：`MAX_CONSECUTIVE_DUPLICATE_SKIPS = 2`。
- 历史窗口上限：`MAX_CONVERSATION_HISTORY_TURNS = 100`（仅计 Turn 变体，Summary 不占配额）。
- 自动压缩阈值：`0.75`（prompt tokens / context_window_size），低于此值不触发压缩。
- 默认请求限速：模型请求间隔最少 `1000ms`。

## 事件语义

常见顺序：

1. `TurnStarted`
2. `ResponseStarted` / `ResponseTextDelta*`（模型直出场景）
3. `ToolCallStarted`（命中工具时）
4. `ToolCallArgsDelta` / `ToolCallProgress`（工具流式增量）
5. `ApprovalRequested` / `ApprovalResolved`（按策略触发）
6. `ToolCallCompleted` 或 `ToolCallFailed`
7. `ResponseCompleted`（`AssistantMessage` 仅兼容旧链路）
8. `TurnCompleted`

`submit_with_sink` 会在返回 `Vec<Event>` 的同时，将同样的事件流实时推送到外部 sink。

## 策略来源

- 审批策略：通过 `agent.set_policy_runtime(Some(runtime))` 注入 `PolicyRuntime` 配置。
- 沙箱模式：`OPENJAX_SANDBOX_MODE` 或 `config.sandbox.mode`。
- Final writer 路径已停用，当前固定使用 planner-only 输出链路。

## 扩展建议

- [LoopDetector 循环检测机制](./docs/loop_detector_design.md)
- 增加新回合阶段时，优先在 `planner.rs` 维护状态机，避免在 `turn.rs` 膨胀逻辑。
- 新增事件类型后，确保 `events.rs` 的 sink 与返回路径保持一致。
- 任何工具策略变更，应同步更新 `tool_guard.rs` / `tool_policy.rs` 与相关集成测试。
