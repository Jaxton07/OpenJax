# Phase 3 设计规格：Agent 循环层 — Native Tool Calling

> 日期：2026-03-28
> 状态：Draft
> 父文档：`docs/plan/refactor/tools/native-tool-calling-plan.md`
> 前置：Phase 1 (types.rs) ✅、Phase 2 (adapters) ✅

---

## 目标

用 Native Tool Calling 对话循环（`ModelResponse.content` → `tool_use` blocks → `tool_result` blocks）替换当前的 "Planner Prompt" 循环（JSON 文本 → dispatcher → `DispatchOutcome`）。dispatcher、`DecisionJsonStreamParser`、JSON 修复路径从 planner 热路径中移除。

## 实施策略

三个顺序子阶段，其中 3a 独立可编译；3b/3c 共享一次编译边界，需成组落地：

| 子阶段 | 范围 | 涉及文件 | 是否破坏编译 |
|--------|------|---------|-------------|
| 3a 基础 | 新增接口和函数 | `router_impl.rs`、`prompt.rs` | 否，纯新增 |
| 3b 流简化 | 去掉 JSON 解析 | `planner_stream_flow.rs` | 与 3c 成组提交，单独落地会破坏编译 |
| 3c 核心重写 | 循环 + 测试 | `planner.rs`、`planner_tool_action.rs`、`tests.rs` | 与 3b 一起恢复编译 |

**当前代码基线说明：**
- 本规格不是从空白 Phase 3 起草，而是建立在当前工作区已落地的 Phase 1/2 迁移代码之上继续推进。
- 具体前提：`openjax-core/src/model/types.rs` 已切到 `messages/content/tools/stop_reason` 结构，`anthropic_messages.rs` / `chat_completions.rs` 已开始发送和解析原生 `tool_use`。
- 因此 Phase 3 的任务不是重新设计 model 层，而是把仍停留在旧 JSON planner 路径上的 agent 循环收敛到与现有 model/adapters 一致的单一路径。

---

## 子阶段 3a：基础

### 3a.1 `tools/router_impl.rs` — 暴露 tool_specs

新增一个方法：

```rust
impl ToolRouter {
    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.specs.clone()
    }
}
```

`display_name_for()` 已存在（返回 `Option<String>`），无需重复。

### 3a.2 `agent/prompt.rs` — 新增函数（保留旧函数直到 3c）

**`build_system_prompt(skills_context: &str) -> String`**

从 `build_planner_input` 提取非 JSON-schema 内容：人设、行为规则、工具选择策略、技能上下文。去掉所有 JSON 格式指令、工具名枚举、action schema 规则（这些现在通过原生 `tools` 参数传递）。

**保留到 system prompt 的内容：**
- 人设："You are OpenJax, an all-purpose personal AI assistant."
- "If task can be answered now, respond with the final answer directly."
- "In final answer, avoid mentioning internal planning, hidden reasoning, or tool traces."
- "If required information is missing, ask one concise clarification question."
- "If verification already shows the requested content/changes are present, respond immediately."
- 工具选择策略（优先 read_file、edit_file_range 用于单文件、apply_patch 用于多文件）
- apply_patch 格式规则（参数格式化规则，不是工具发现）
- edit_file_range 参数规则
- Shell 工作区相对路径偏好
- 技能调用规则：skill 标记如 `/skill-name` 不是 shell 可执行文件
- 不重复策略："Do NOT repeat the same tool call with the same arguments."
- 技能上下文块

**从 prompt 中移除的内容（现在由原生 `tools` 参数处理）：**
- "Return ONLY valid JSON" 指令
- JSON schema（`{"action":"tool",...}`、`{"action":"final",...}`）
- 工具名枚举（`read_file|list_dir|grep_files|...`）
- "At most one action per response"
- "All values inside args MUST be JSON strings"

**`build_turn_messages(user_input: &str, history: &[HistoryItem], loop_recovery: Option<&str>) -> Vec<ConversationMessage>`**

构建 `Vec<ConversationMessage>` 用于模型请求：
- 如果 history 非空，注入 `<prior_conversation>` 文本摘要作为第一条 User 消息（格式与当前 `build_planner_input` 历史部分相同）
- 当前 `user_input` 作为最后一条 `ConversationMessage::User(vec![Text{ text: user_input }])`
- `loop_recovery` 如果存在，追加到 user_input 文本末尾

**关于 `tool_traces`：** `build_turn_messages` 不接受 `tool_traces` 参数。当前 turn 内的工具执行历史通过 `messages` 中的 `ConversationMessage`（tool_use / tool_result 对）自然传递。`commit_turn` 需要的 `tool_traces: Vec<String>` 在 `execute_native_tool_call` 中以 `"tool_name(args) → result_summary"` 格式收集。

**关于 loop recovery 刷新：**
- `build_turn_messages` 只负责构建“跨 turn 基线消息”（历史摘要 + 当前用户输入），不负责修改当前 turn 内已累计的 assistant/tool_result 消息。
- 3c 需要新增一个小型 helper（例如 `refresh_loop_recovery_in_messages(messages: &mut Vec<ConversationMessage>, user_input: &str, loop_recovery: Option<&str>)`），职责是仅更新“当前 turn 的最后一条用户文本消息”中的 recovery 段。
- 该 helper 不得重建整个 `messages`，也不得丢弃当前 turn 内已经追加的 `ConversationMessage::Assistant(...)` 与 `ConversationMessage::User(tool_result_blocks)`。

**保留的工具函数：** `truncate_for_prompt` 和 `summarize_user_input` — 仍被 `planner_tool_action.rs` 和 `planner_utils.rs` 引用。

---

## 子阶段 3b：流式简化

### 3b.1 `agent/planner_stream_flow.rs` — 移除 JSON 解析

**当前流程：**
1. 通过 `DecisionJsonStreamParser` 流式解析模型输出
2. 尝试 `parse_model_decision` 解析结果
3. 解析失败则尝试从流式消息重建 JSON
4. 仍然失败则 fallback 到 `model_client.complete()`（非流式）
5. 返回 `PlannerStreamResult { model_output: String, ... }`

**新流程：**
1. 流式处理 model deltas：Text → orchestrator、ToolUseStart/ArgsDelta/End → events、Reasoning → event
2. 从 Text deltas 收集流式文本
3. 直接返回 `PlannerStreamResult` 包含 `response: ModelResponse`
4. 无 JSON 解析，无 fallback 到 `complete()`

**新 `PlannerStreamResult`：**

```rust
pub(super) struct PlannerStreamResult {
    pub(super) response: ModelResponse,
    /// Text deltas 累积的文本（用于 final answer 流式传输）。
    /// 语义：response.text() 的流式版本，不等同于旧的 model_output（原始 JSON 字符串）。
    pub(super) streamed_text: String,
    pub(super) live_streamed: bool,
    pub(super) usage: Option<ModelUsage>,
}
```

**移除：** `DecisionJsonStreamParser` 导入和使用、`parse_model_decision` 调用、`action_hint` 字段、`model_output: String` 字段、fallback-to-complete 逻辑。

**保留：** `emit_synthetic_response_deltas`、TTFT 日志、`ResponseStreamOrchestrator`、所有 `StreamDelta` 事件处理。

**事件约束：**
- `ToolUseStart` → 发 `ToolCallStarted`
- `ToolArgsDelta` → 发 `ToolCallArgsDelta`
- `ToolUseEnd` → 发 `ToolCallReady`
- 对同一个 `tool_call_id`，流式阶段必须缓存 `tool_name`，后续 `ToolCallArgsDelta` / `ToolCallReady` 继续携带真实 `tool_name`，不得发空字符串
- 如果能从 `ToolRouter` 查到 display name，流式阶段也应复用同一份 `display_name`
- 执行阶段不得再次对同一个 `tool_call_id` 发 `ToolCallStarted`、`ToolCallArgsDelta`、`ToolCallReady`
- 执行阶段只负责 `ToolCallProgress`、`ToolCallCompleted`、`ToolCallFailed`

**错误处理：** 如果 `complete_stream` 失败，直接返回错误 — 不用 `complete()` 重试。

---

## 子阶段 3c：核心循环重写 + 测试

### 3c.1 `agent/planner.rs` — 新循环

**当前流程（简化）：**

```
while 未达限制:
    prompt = build_planner_input(...)
    result = request_planner_model_output(turn_id, &request, true, events)
    routed = dispatcher::route_model_output(result.model_output, ...)
    match routed:
        ToolBatch → execute_tool_batch_calls
        Tool → handle_tool_action
        Final → emit response, commit_turn, return
        Repair → 尝试 JSON 修复
        Error → emit error, return
```

**新流程：**

```
system_prompt = build_system_prompt(&skills_context)
messages = build_turn_messages(user_input, &history, initial_loop_recovery)
tool_specs = self.tools.tool_specs()

while 未达限制:
    refresh_loop_recovery_in_messages(messages, user_input, loop_detector.recovery_prompt())
    request = ModelRequest { stage: Planner, system_prompt, messages, tools: tool_specs, options }
    result = request_planner_model_output(turn_id, &request, true, events)
    response = result.response

    // 将助手响应追加到 messages
    messages.push(ConversationMessage::Assistant(response.content.clone()))

    if !response.has_tool_use():
        // 最终回答 — 发射事件、提交、返回
        let final_text = response.text()
        emit_final_response(...)
        commit_turn(user_input, tool_traces, final_text)
        return

    // 收集 tool_use blocks，发射 ToolCallsProposed
    // 注意：planner_stream_flow 已在流式阶段发射 Started/ArgsDelta/Ready
    // 执行阶段不得再次补发这些事件
    let tool_uses = response.tool_uses()
    emit ToolCallsProposed event
    let mut tool_result_blocks: Vec<UserContentBlock> = Vec::new()

    for tool_use in tool_uses:
        let AssistantContentBlock::ToolUse { id, name, input } = tool_use else { continue };

        // execute_native_tool_call 内部只发 Progress/Completed/Failed
        let outcome = execute_native_tool_call(
            turn_id, id, name, input, events, &mut tool_traces,
            &mut apply_patch_read_guard, &mut consecutive_duplicate_skips,
            &mut turn_engine, ...
        );

        match outcome:
            Aborted → emit error, return
            Result { content, ok } →
                tool_result_blocks.push(UserContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content,
                    is_error: !ok,
                })
                executed_count += 1

    // 所有工具执行完毕
    emit ToolBatchCompleted { total: tool_uses.len(), ... }
    messages.push(ConversationMessage::User(tool_result_blocks))
    // 继续循环
```

**关键变更：**
- `dispatcher::route_model_output()` — 不再调用
- `DecisionJsonStreamParser` — 不再使用
- JSON 修复路径 — 完全移除
- `DispatchOutcome` — 不再使用
- `build_planner_input` — 被 `build_system_prompt` + `build_turn_messages` 替代
- 工具执行使用现有 `ToolRouter::execute()`，`ToolCall` 从 `AssistantContentBlock::ToolUse` 构造

**迁移保留的逻辑：**
- `ApplyPatchReadGuard` — 在 `execute_native_tool_call` 中检查
- 重复工具调用检测 — 在 `execute_native_tool_call` 中检查
- 循环检测 `loop_detector` — 每次工具执行后检查
- `TurnEngine` 状态机事件 — `on_response_started/completed/failed`
- 速率限制 — 每次模型调用前 `apply_rate_limit()`
- 技能上下文构建
- `max_tool_calls_per_turn` / `max_planner_rounds_per_turn` 限制
- `tool_traces` 记录（用于 `commit_turn`）
- 自动压缩 `check_and_auto_compact`
- Flow trace 日志

### 3c.2 `agent/planner_tool_action.rs` — 新增 `execute_native_tool_call`

与 `handle_tool_action` 并列的新方法。逻辑相同，但输入格式改变：

**旧：** `handle_tool_action(turn_id, decision: &ModelDecision, ctx: &mut ToolActionContext)`

**新：** `execute_native_tool_call(turn_id, tool_call_id: &str, tool_name: &str, input: &Value, ctx: &mut ToolActionContext)`

关键差异：
- `args` 来自 `serde_json::Value`（原生 tool call input）而非 `HashMap<String, String>`（从 JSON 文本解析）
- 将 `Value` 转换为 `HashMap<String, String>`：`Value::String(s) → s`，其他值 → `serde_json::to_string(&v)`
- `tool_call_id` 由模型提供（原生），不再生成
- 审批处理保留
- 所有守卫保留（apply_patch_read_guard、重复检测、循环检测）
- 返回 `ToolExecOutcome` — `Result { content: String, ok: bool }` 或 `Aborted`
- 本方法不再调用 `emit_tool_call_started_sequence`，避免与流式阶段重复发 `Started/ArgsDelta/Ready`

**关于 `ToolActionContext`：** 复用 `planner.rs` 中现有的 `ToolActionContext` 结构体，相同字段。无需新 context 结构体。

**为何保留 `planner_tool_action.rs` 而不是并回 `planner.rs`：**
- 这是相对原始总计划的有意边界调整，不是遗漏清理。
- 当前 `planner_tool_action.rs` 已集中承载 apply_patch 守卫、重复调用检测、审批阻塞、loop detector、state epoch 更新等高风险执行逻辑；Phase 3 仅替换入参和事件语义，不重写这部分职责边界。
- 保留该模块可以避免 `planner.rs` 在主循环重写时继续膨胀，也降低“同时改循环编排和工具执行守卫”带来的回归风险。
- Phase 3 结束后的清理目标调整为：删除旧 `handle_tool_action`，保留 `planner_tool_action.rs` 作为独立执行模块；是否进一步合并文件，留待后续在代码规模和可读性都验证后再决定。

### 3c.3 将 ToolUse 转换为 ToolCall

`ToolRouter::execute()` 期望 `ToolExecutionRequest` 包含 `ToolCall { name, args: HashMap<String, String> }`。转换函数：

```rust
fn tool_use_to_call(name: &str, input: &Value) -> ToolCall {
    let args = match input {
        Value::Object(map) => map.iter().map(|(k, v)| {
            let s = match v {
                Value::String(s) => s.clone(),
                other => serde_json::to_string(other).unwrap_or_default(),
            };
            (k.clone(), s)
        }).collect(),
        _ => HashMap::new(),
    };
    ToolCall { name: name.to_string(), args }
}
```

### 3c.4 `tests.rs` — 更新 mock models

所有 mock `ModelClient` 实现必须返回原生 content blocks：

| Mock | 旧返回 | 新返回 |
|------|--------|--------|
| `ScriptedStreamingModel` | JSON 文本 `{"action":"final","message":"seed"}` | `ModelResponse { content: vec![Text{text:"seed"}], stop_reason: Some(EndTurn) }` |
| `ScriptedToolBatchModel` | JSON 文本 `{"action":"tool_batch",...}` | 首次调用：`vec![ToolUse{id,name,input}, ...]` + `StopReason::ToolUse`；第二次：`Text{text:"batch done"}` + `EndTurn` |
| `DuplicateToolLoopModel` | JSON 文本 `{"action":"tool",...}` | 每次调用：`ToolUse{...}` + `StopReason::ToolUse` |
| `ApprovalBlockedBatchModel` | 同 batch 模式 | 同 ToolUse 模式 |
| `ApprovalCancellationBatchModel` | 同 batch 模式 | 同 ToolUse 模式 |
| `PlannerFallbackModel` | 无效 JSON 文本（测试 fallback） | 改为测试正常流式返回（native 无 fallback） |
| `ScriptedToolBatchDependencyModel` | JSON 含 `depends_on` | ToolUse blocks（depends_on 在执行层处理） |

**删除的测试：**
- `planner_prompt_contains_apply_patch_verification_rule` — 依赖 `build_planner_input`
- `planner_prompt_contains_skills_section` — 依赖 `build_planner_input`
- `planner_stream_parse_failure_falls_back_to_complete_response` — native 无 fallback
- `normalizes_tool_name_in_action_with_top_level_args` — 依赖 `parse_model_decision`
- `keeps_explicit_tool_shape_unchanged` — 依赖 `parse_model_decision`
- `keeps_final_action_unchanged` — 依赖 `parse_model_decision`

**新增替代测试：**
- `build_system_prompt_contains_verification_rule`
- `build_system_prompt_contains_skills_section`
- `build_turn_messages_includes_prior_conversation_summary`
- `refresh_loop_recovery_only_updates_last_user_text`
- `planner_stream_tool_events_preserve_tool_name_across_args_delta_and_ready`

**保留不变的测试：**
- `duplicate_detection_*` — 纯 Agent 方法测试
- `parse_runtime_policies` — 纯配置解析
- `resolves_turn_limits_from_config_and_env` — 纯配置解析
- `aborts_after_consecutive_duplicate_skips` — 纯逻辑判断
- `summarize_user_input_*` — 纯工具函数

### 3c.5 清理 — 删除旧代码路径

3b/3c 成组完成、所有测试通过后：
- 从 `prompt.rs` 删除 `build_planner_input`
- 从 `prompt.rs` 删除 `build_json_repair_prompt`
- 从 `planner_tool_action.rs` 删除 `handle_tool_action`（被 `execute_native_tool_call` 替代）
- 标记 `dispatcher::route_model_output` 为 `#[deprecated]`
- 从 `Agent` 结构体移除 `dispatcher_config` 和 `tool_batch_v2_enabled`（或暂时保留为死字段）

**与原始总计划的差异说明：**
- 原始总计划曾设想在 Phase 3 末尾删除 `planner_tool_action.rs` 并并入 `planner.rs`。
- 当前规格明确不再以“文件合并”为 Phase 3 完成条件；Phase 3 的完成条件改为“旧 JSON planner 路径被移除，agent 循环与现有 native model/adapters 对齐，并保持工具执行守卫逻辑独立可测”。

---

## 涉及文件总结

| 文件 | 子阶段 | 变更类型 |
|------|--------|---------|
| `tools/router_impl.rs` | 3a | 新增 `tool_specs()` |
| `agent/prompt.rs` | 3a | 新增 `build_system_prompt`、`build_turn_messages` |
| `agent/planner_stream_flow.rs` | 3b | 重写：移除 JSON 解析，返回 ModelResponse |
| `agent/planner.rs` | 3c | 核心重写：native tool calling 循环 |
| `agent/planner_tool_action.rs` | 3c | 新增 `execute_native_tool_call` |
| `tests.rs` | 3c | 更新所有 mock，删除 6 个测试 |
| `agent/decision.rs` | 3c（清理） | 不再使用，标记废弃 |
| `dispatcher/mod.rs` | 3c（清理） | Planner 不再调用 |

---

## 风险与缓解

### 1. ToolBatchCompleted 事件兼容性

TUI 和 gateway 消费者期望此事件。新循环必须在所有 tool_use blocks 执行完毕后发射。

**缓解：** 在工具执行循环后发射 `ToolBatchCompleted { total, ... }`。

### 2. ToolCallsProposed 事件格式

现有消费者期望 `arguments: BTreeMap<String, String>`。原生 tool_use 的 `input` 是 `serde_json::Value`。

**缓解：** 用与 `tool_use_to_call` 相同的方式将 Value 扁平化为 String map。

### 3. 重复事件发射

流式阶段已发射 `ToolCallStarted`/`ToolCallReady`，执行阶段需要发射 `ToolCallCompleted`。

**缓解：** 流式阶段和执行阶段发射不同的事件类型，不会重复：
- **流式阶段：** `ToolCallStarted`（参数开始到达）→ `ToolCallArgsDelta`（参数进度）→ `ToolCallReady`（参数接收完毕）
- **执行阶段：** `ToolCallCompleted`（执行结果）

每个工具调用获得干净的事件序列：`ToolCallStarted` → `ToolCallReady` → `ToolCallCompleted`。

### 4. depends_on 处理

当前 batch 模型在 `planner_tool_batch.rs` 中有依赖解析。原生 tool calling 中模型在一个响应中返回所有 tool_uses，循环顺序执行。

**缓解：** 3c 中顺序执行 tool_uses（无需依赖解析，模型管理调用顺序）。批量并行执行是优化，后续可重新添加。

### 5. context_compressor 兼容性

`commit_turn` 期望 `tool_traces: Vec<String>`。

**缓解：** 新循环在 `execute_native_tool_call` 中以相同格式收集 `tool_traces`。`commit_turn` 接口不变。

### 6. ModelRequest::for_stage 兼容性

测试和其他调用者使用 `ModelRequest::for_stage()` 便捷构造器。

**缓解：** 保留 `for_stage()` 作为便捷构造器。新 planner 直接构造 `ModelRequest`。

---

## 测试策略

每个子阶段完成后：
```bash
cargo test -p openjax-core
```

3c 完成后（全量回归）：
```bash
cargo test -p openjax-core --test tools_sandbox_suite
cargo test -p openjax-core --test approval_suite
cargo test -p openjax-core --test streaming_suite
cargo test -p openjax-core --test skills_suite
cargo test -p openjax-core --test core_history_suite
cargo test -p openjax-core
```
