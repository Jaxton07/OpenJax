# Phase 3 Native Tool Calling 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用 Native Tool Calling 对话循环替换 "Planner Prompt" JSON 文本循环，移除 dispatcher/DecisionJsonStreamParser/JSON repair 依赖。

**Architecture:** 三个顺序子阶段（3a 基础 → 3b 流简化 → 3c 核心重写）。其中 3a 可独立提交；3b 和 3c 共享一次编译边界，必须成组落地，避免 `planner.rs` 与 `planner_stream_flow.rs` 字段/接口暂时失配。

**Execution Baseline:** 当前计划建立在工作区里已落地的 Phase 1/2 代码之上推进，而不是从旧 model 类型重新开始。也就是说，`openjax-core/src/model/types.rs`、`anthropic_messages.rs`、`chat_completions.rs` 的 native tool calling 结构迁移视为现有前提；Phase 3 的主要任务是让 agent 层停止消费旧 JSON planner 输出，转而对齐这套已存在的 model/adapters 形状。

**Tech Stack:** Rust, tokio, serde_json, openjax-protocol (Event types), openjax-policy

**Spec:** `docs/superpowers/specs/2026-03-28-phase3-native-tool-calling-design.md`

---

## 文件变更地图

| 文件 | 子阶段 | 变更 |
|------|--------|------|
| `openjax-core/src/tools/router_impl.rs` | 3a | 新增 `tool_specs()` |
| `openjax-core/src/agent/prompt.rs` | 3a | 新增 `build_system_prompt`、`build_turn_messages` |
| `openjax-core/src/agent/planner_stream_flow.rs` | 3b | 重写：移除 JSON 解析，返回 ModelResponse |
| `openjax-core/src/agent/planner_tool_action.rs` | 3c | 新增 `execute_native_tool_call` |
| `openjax-core/src/agent/planner.rs` | 3c | 核心重写：native tool calling 循环 |
| `openjax-core/src/tests.rs` | 3c | 更新 mock models，删除 6 个测试 |

---

## Task 1: router_impl.rs — 暴露 tool_specs

**Files:**
- Modify: `openjax-core/src/tools/router_impl.rs` (在 `display_name_for()` 方法后，约第 74 行)

- [ ] **Step 1: 添加 `tool_specs()` 方法**

在 `display_name_for()` 方法后添加：

```rust
/// 返回所有工具的 ToolSpec（传给 ModelRequest.tools）
pub fn tool_specs(&self) -> Vec<ToolSpec> {
    self.specs.clone()
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo build -p openjax-core`
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add openjax-core/src/tools/router_impl.rs
git commit -m "feat(core): 新增 ToolRouter::tool_specs() 方法"
```

---

## Task 2: prompt.rs — 新增 build_system_prompt

**Files:**
- Modify: `openjax-core/src/agent/prompt.rs`

**参考:** 当前 `build_planner_input()` (第 25-121 行) 的内容。新函数从其中提取非 JSON schema 部分。

- [ ] **Step 1: 添加 `build_system_prompt` 函数**

在 `build_planner_input` 函数之前添加新函数。内容提取自 `build_planner_input` 前半部分，去掉 JSON 格式规则和工具名枚举：

```rust
pub(crate) fn build_system_prompt(skills_context: &str) -> String {
    format!(
        "You are OpenJax, an all-purpose personal AI assistant.\n\n\
        Rules:\n\
        - If task can be answered now, respond with the final answer directly.\n\
        - In final answer, avoid mentioning internal planning, hidden reasoning, or tool traces unless the user explicitly asks.\n\
        - If required information is missing, ask one concise clarification question.\n\
        - If verification already shows the requested content/changes are present, respond immediately.\n\
        - Do NOT repeat the same tool call with the same arguments.\n\
        \n\
        Tool selection policy:\n\
        - Prefer read_file before edit_file_range or apply_patch (Update File) unless creating a brand-new file.\n\
        - Prefer edit_file_range for single-file edits when exact line range is known.\n\
        - For multi-file edits or file operations (add/delete/move/rename), use apply_patch.\n\
        - Prefer process_snapshot/system_load/disk_usage for process/host metrics over shell ps/top/df.\n\
        - For apply_patch, use this EXACT format:\n\
          *** Begin Patch\n\
          *** Update File: <filepath>\n\
          @@\n\
           context line (MUST start with space)\n\
          -line to remove (starts with -)\n\
          +line to add (starts with +)\n\
          *** End Patch\n\
          Operations: *** Add File:, *** Update File:, *** Delete File:, *** Move File: from -> to, *** Rename File: from -> to, *** Move to:\n\
          IMPORTANT: In Update File, every line after @@ MUST start with space (context), - (remove), or + (add).\n\
          IMPORTANT: When modifying existing files, preserve the source file's formatting and style.\n\
        - For edit_file_range, provide args: file_path, start_line, end_line, new_text.\n\
        - For shell, prefer workspace-relative commands; avoid absolute-path `cd` unless required.\n\
        - Skill markers like `/skill-name` are not shell executables; convert selected skills into concrete tool steps.\n\
        \n\
        Available skills (auto-selected):\n\
        {skills_context}"
    )
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo build -p openjax-core`
Expected: 编译成功（旧函数仍存在，无破坏）

- [ ] **Step 3: 提交**

```bash
git add openjax-core/src/agent/prompt.rs
git commit -m "feat(core): 新增 build_system_prompt 函数"
```

---

## Task 3: prompt.rs — 新增 build_turn_messages

**Files:**
- Modify: `openjax-core/src/agent/prompt.rs`

- [ ] **Step 1: 添加 `build_turn_messages` 函数**

在 `build_system_prompt` 之后添加：

```rust
pub(crate) fn build_turn_messages(
    user_input: &str,
    history: &[crate::HistoryItem],
    loop_recovery: Option<&str>,
) -> Vec<crate::model::ConversationMessage> {
    use crate::model::{ConversationMessage, UserContentBlock};

    let mut messages = Vec::new();

    // 历史摘要注入
    if !history.is_empty() {
        let mut turn_num = 0usize;
        let summary = history
            .iter()
            .map(|item| match item {
                crate::HistoryItem::Turn(r) => {
                    turn_num += 1;
                    let tools_section = if r.tool_traces.is_empty() {
                        String::new()
                    } else {
                        format!("\nTools:\n  {}", r.tool_traces.join("\n  "))
                    };
                    format!(
                        "[Turn {}]\nUser: {}{}\nAssistant: {}",
                        turn_num, r.user_input, tools_section, r.assistant_output
                    )
                }
                crate::HistoryItem::Summary(s) => s.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        messages.push(ConversationMessage::User(vec![UserContentBlock::Text {
            text: format!("<prior_conversation>\n{summary}\n</prior_conversation>"),
        }]));
    }

    // 当前用户输入（含 loop_recovery）
    let mut input_text = user_input.to_string();
    if let Some(recovery) = loop_recovery {
        input_text.push_str("\n\n");
        input_text.push_str(recovery);
    }
    messages.push(ConversationMessage::User(vec![UserContentBlock::Text {
        text: input_text,
    }]));

    messages
}
```

- [ ] **Step 2: 添加 recovery 刷新 helper**

在 `prompt.rs` 中与 `build_turn_messages` 相邻新增一个 helper，例如：

```rust
pub(crate) fn refresh_loop_recovery_in_messages(
    messages: &mut Vec<crate::model::ConversationMessage>,
    user_input: &str,
    loop_recovery: Option<&str>,
) {
    // 只更新“当前 turn 的最后一条 User/Text 消息”
    // 不得重建整个 messages，也不得丢弃中间已累计的 assistant/tool_result 消息
}
```

**要求：**
- 该 helper 只能改写当前 turn 的最后一条 `ConversationMessage::User(Text)`
- 历史摘要消息保持不变
- 当前 turn 已追加的 `Assistant(tool_use/text)` 和 `User(tool_result)` 消息必须完整保留

- [ ] **Step 3: 验证编译**

Run: `cargo build -p openjax-core`
Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add openjax-core/src/agent/prompt.rs
git commit -m "feat(core): 新增 build_turn_messages 函数"
```

---

## Task 4: planner_stream_flow.rs — 重写流式处理

**Files:**
- Modify: `openjax-core/src/agent/planner_stream_flow.rs` (247 行)

**目标：** 移除 `DecisionJsonStreamParser`、JSON 解析、fallback-to-complete 逻辑。直接返回 `ModelResponse`。

这是变更最集中的文件。新版本从 247 行简化到约 120 行。

- [ ] **Step 1: 重写 `PlannerStreamResult` 结构体**

将：
```rust
pub(super) struct PlannerStreamResult {
    pub(super) model_output: String,
    pub(super) streamed_message: String,
    pub(super) live_streamed: bool,
    pub(super) action_hint: Option<String>,
    pub(super) usage: Option<ModelUsage>,
}
```

替换为：
```rust
pub(super) struct PlannerStreamResult {
    pub(super) response: ModelResponse,
    pub(super) streamed_text: String,
    pub(super) live_streamed: bool,
    pub(super) usage: Option<ModelUsage>,
}
```

- [ ] **Step 2: 重写 `request_planner_model_output` 方法**

**移除：** `DecisionJsonStreamParser` 创建和使用、`parse_model_decision` 调用、`action_hint`、`model_output` 文本构建、fallback-to-complete 逻辑（第 148-211 行）。

**新逻辑：**
```rust
impl Agent {
    pub(super) async fn request_planner_model_output(
        &mut self,
        turn_id: u64,
        planner_request: &ModelRequest,
        emit_live_final_deltas: bool,
        events: &mut Vec<Event>,
    ) -> anyhow::Result<PlannerStreamResult> {
        let started_at = Instant::now();
        let (delta_tx, delta_rx) = tokio::sync::mpsc::unbounded_channel();
        let stream_future = self
            .model_client
            .complete_stream(planner_request, Some(delta_tx));

        let mut streamed_text = String::new();
        let mut response_started = false;
        let mut ttft_logged = false;
        let mut delta_event_count = 0u64;
        let mut last_live_delta_at: Option<Instant> = None;
        let mut tool_names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut stream_orchestrator =
            ResponseStreamOrchestrator::new(turn_id, openjax_protocol::StreamSource::ModelLive);

        let stream_result =
            run_stream_with_delta_handler(delta_rx, stream_future, |delta| match delta {
                StreamDelta::Text(text_delta) => {
                    if text_delta.is_empty() || !emit_live_final_deltas {
                        return;
                    }
                    if !response_started {
                        response_started = true;
                    }
                    if !ttft_logged {
                        ttft_logged = true;
                        info!(
                            turn_id = turn_id,
                            planner_stream_ttft_ms = started_at.elapsed().as_millis(),
                            "planner_stream_ttft"
                        );
                    }
                    streamed_text.push_str(&text_delta);
                    delta_event_count = delta_event_count.saturating_add(1);
                    last_live_delta_at = Some(Instant::now());
                    for event in stream_orchestrator.on_delta(&text_delta) {
                        self.push_event(events, event);
                    }
                }
                StreamDelta::ToolUseStart { id, name } => {
                    tool_names.insert(id.clone(), name.clone());
                    self.push_event(
                        events,
                        Event::ToolCallStarted {
                            turn_id,
                            tool_call_id: id,
                            tool_name: name,
                            target: None,
                            display_name: None,
                        },
                    );
                }
                StreamDelta::ToolArgsDelta { id, delta } => {
                    self.push_event(
                        events,
                        Event::ToolCallArgsDelta {
                            turn_id,
                            tool_call_id: id.clone(),
                            tool_name: tool_names.get(&id).cloned().unwrap_or_default(),
                            args_delta: delta,
                            display_name: None,
                        },
                    );
                }
                StreamDelta::ToolUseEnd { id } => {
                    self.push_event(
                        events,
                        Event::ToolCallReady {
                            turn_id,
                            tool_call_id: id.clone(),
                            tool_name: tool_names.get(&id).cloned().unwrap_or_default(),
                            display_name: None,
                        },
                    );
                }
                StreamDelta::Reasoning(reasoning_delta) => {
                    if reasoning_delta.is_empty() {
                        return;
                    }
                    let (preview, preview_truncated) = reasoning_preview(&reasoning_delta, 48);
                    info!(
                        target: AFTER_DISPATCH_LOG_TARGET,
                        turn_id = turn_id,
                        flow_prefix = AFTER_DISPATCH_PREFIX,
                        flow_node = "planner.reasoning.emit",
                        flow_route = "reasoning_delta",
                        flow_next = "gateway.reasoning_delta",
                        delta_len = reasoning_delta.chars().count(),
                        delta_preview = %preview,
                        delta_preview_truncated = preview_truncated,
                        "after_dispatcher_trace"
                    );
                    self.push_event(
                        events,
                        Event::ReasoningDelta {
                            turn_id,
                            content_delta: reasoning_delta,
                            stream_source: openjax_protocol::StreamSource::ModelLive,
                        },
                    );
                }
            })
            .await;

        // 直接使用流式结果，无 JSON 解析，无 fallback
        let response = stream_result?;
        let captured_usage = response.usage.clone();

        info!(
            turn_id = turn_id,
            planner_stream_total_ms = started_at.elapsed().as_millis(),
            live_streamed = response_started,
            delta_events = delta_event_count,
            tail_silence_ms = last_live_delta_at
                .map(|ts| ts.elapsed().as_millis() as u64)
                .unwrap_or(started_at.elapsed().as_millis() as u64),
            delta_chars = streamed_text.chars().count(),
            "planner_stream_completed"
        );

        Ok(PlannerStreamResult {
            response,
            streamed_text,
            live_streamed: response_started,
            usage: captured_usage,
        })
    }
}
```

**关键变更：**
- 移除 `use crate::agent::decision::{DecisionJsonStreamParser, parse_model_decision};` 导入
- 移除所有 `parser` 相关代码
- 移除 `fallback_reason` 和 fallback-to-complete 逻辑
- `stream_result?` 直接传播错误（无 retry）
- 返回 `PlannerStreamResult { response, streamed_text, live_streamed, usage }`
- 对同一个 `tool_call_id`，`ToolCallArgsDelta` / `ToolCallReady` 必须沿用 `ToolUseStart` 缓存的 `tool_name`

- [ ] **Step 3: 保留 `emit_synthetic_response_deltas` 不变**

`emit_synthetic_response_deltas` 方法保持原样，不被修改。

- [ ] **Step 4: 验证编译**

Run: `cargo build -p openjax-core`
Expected: 暂不单独验证通过。Task 4 属于 3b/3c 共享编译边界的一部分，需与 Task 5、Task 6 完成后统一执行 `cargo build -p openjax-core`。

**注意：** 不再接受“Task 4 单独提交但仓库暂时不编译”的执行方式。Task 4/5/6 必须作为一个可编译批次落地；中途如果出现编译失败，应继续推进到 Task 6 修平后再提交。

- [ ] **Step 5: 提交**

```bash
git add openjax-core/src/agent/planner_stream_flow.rs
git commit -m "refactor(core): 重写 planner_stream_flow 移除 JSON 解析"
```

---

## Task 5: planner_tool_action.rs — 新增 execute_native_tool_call

**Files:**
- Modify: `openjax-core/src/agent/planner_tool_action.rs` (345 行)

**目标：** 新增 `execute_native_tool_call` 方法，与 `handle_tool_action` 并列。逻辑完整复制自 `handle_tool_action`（第 23-344 行），仅修改入参格式。

**关键原则：** 此方法必须保留 `handle_tool_action` 的核心守卫和状态更新逻辑，包括 apply_patch 守卫、审批阻塞、loop_detector、state_epoch、skill_shell_misfire 跟踪；但事件发射生命周期不能再 1:1 复制，因为 native tool call 的 `Started/ArgsDelta/Ready` 已在流式阶段发出。

**边界决策说明：** 这里有意保留 `planner_tool_action.rs`，不按原始总计划把它并回 `planner.rs`。原因是该文件已经集中承载工具执行守卫和审批/循环等高风险逻辑；Phase 3 先完成“主循环语义迁移”，避免在同一轮里同时打散这些执行逻辑边界。Phase 3 的清理目标收敛为“删除旧 `handle_tool_action`，保留独立的 native 工具执行模块”。

- [ ] **Step 1: 定义返回类型**

在文件顶部（imports 之后）添加：

```rust
use serde_json::Value;
use std::collections::HashMap;

pub(super) enum NativeToolOutcome {
    Result { content: String, ok: bool },
    Aborted,
}
```

- [ ] **Step 2: 添加 `execute_native_tool_call` 方法**

在 `impl Agent` 块中 `handle_tool_action` 之后添加。以下代码 **完整** 复制自 `handle_tool_action` 的所有逻辑：

```rust
pub(super) async fn execute_native_tool_call(
    &mut self,
    turn_id: u64,
    tool_call_id: &str,
    tool_name: &str,
    input: &Value,
    events: &mut Vec<Event>,
    tool_traces: &mut Vec<String>,
    apply_patch_read_guard: &mut crate::agent::tool_guard::ApplyPatchReadGuard,
    consecutive_duplicate_skips: &mut usize,
    executed_count: &mut usize,
    turn_engine: &mut crate::agent::turn_engine::TurnEngine,
    skill_shell_misfire_count: &mut usize,
    saw_git_status_short: &mut bool,
    saw_git_diff_stat: &mut bool,
    diff_strategy: &mut &'static str,
) -> NativeToolOutcome {
    // 1. 将 serde_json::Value 转为 HashMap<String, String>
    let args: HashMap<String, String> = match input {
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| {
                let s = match v {
                    Value::String(s) => s.clone(),
                    other => serde_json::to_string(other).unwrap_or_default(),
                };
                (k.clone(), s)
            })
            .collect(),
        _ => HashMap::new(),
    };

    // 2. skill_shell_misfire / git / diff 跟踪（原 handle_tool_action 第 48-63 行）
    if let Some(cmd) = args.get("cmd")
        && crate::agent::planner_utils::looks_like_skill_trigger_shell_command(cmd)
    {
        *skill_shell_misfire_count = (*skill_shell_misfire_count).saturating_add(1);
    }
    if let Some(cmd) = args.get("cmd") {
        if crate::agent::planner_utils::is_git_status_short(cmd) {
            *saw_git_status_short = true;
        }
        if crate::agent::planner_utils::is_git_diff_stat(cmd) {
            *saw_git_diff_stat = true;
        }
        if let Some(next_strategy) = crate::agent::planner_utils::detect_diff_strategy(cmd) {
            *diff_strategy = crate::agent::planner_utils::merge_diff_strategy(diff_strategy, next_strategy);
        }
    }

    // 3. apply_patch 守卫检查（原 handle_tool_action 第 65-119 行）
    if let Some(message) = apply_patch_read_guard.block_user_message_for_tool(tool_name) {
        warn!(
            turn_id = turn_id,
            tool_call_id = %tool_call_id,
            reason = apply_patch_read_guard
                .block_log_reason_for_tool(tool_name)
                .unwrap_or("unknown"),
            "apply_patch blocked by read-before-repatch guard"
        );

        self.push_event(
            events,
            Event::ToolCallProgress {
                turn_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                progress_message: "executing".to_string(),
                display_name: self.tools.display_name_for(tool_name),
            },
        );
        self.push_event(
            events,
            Event::ToolCallFailed {
                turn_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                code: "guard_blocked".to_string(),
                message: message.to_string(),
                retryable: false,
                display_name: self.tools.display_name_for(tool_name),
            },
        );

        self.record_tool_call(tool_name, &args, false, message);
        tool_traces.push(format!(
            "tool={tool_name}; ok=false; output={}",
            truncate_for_prompt(message, self.skill_runtime_config.max_diff_chars_for_planner)
        ));
        self.emit_tool_call_completed(
            turn_id,
            tool_call_id,
            tool_name,
            false,
            message,
            events,
        );
        *executed_count += 1;
        *consecutive_duplicate_skips = 0;
        return NativeToolOutcome::Result {
            content: message.to_string(),
            ok: false,
        };
    }

    // 4. 重复调用检测（原 handle_tool_action 第 121-173 行）
    if self.is_duplicate_tool_call(tool_name, &args) {
        warn!(
            turn_id = turn_id,
            tool_name = %tool_name,
            args = ?args,
            "duplicate_tool_call detected, skipping"
        );
        let message = crate::agent::tool_policy::duplicate_tool_call_warning(tool_name, &args);
        self.push_event(
            events,
            Event::ResponseError {
                turn_id,
                code: "duplicate_tool_call_skipped".to_string(),
                message: message.clone(),
                retryable: true,
            },
        );
        tool_traces.push(format!(
            "tool={tool_name}; ok=skipped_duplicate; args={}; output={}",
            serde_json::to_string(&args).unwrap_or_default(),
            truncate_for_prompt(&message, self.skill_runtime_config.max_diff_chars_for_planner)
        ));
        *consecutive_duplicate_skips = (*consecutive_duplicate_skips).saturating_add(1);
        if crate::agent::tool_policy::should_abort_on_consecutive_duplicate_skips(
            *consecutive_duplicate_skips,
            crate::MAX_CONSECUTIVE_DUPLICATE_SKIPS,
        ) {
            let loop_message = crate::agent::tool_policy::duplicate_skip_abort_message(
                crate::MAX_CONSECUTIVE_DUPLICATE_SKIPS,
            );
            self.push_event(
                events,
                Event::ResponseError {
                    turn_id,
                    code: "duplicate_tool_call_loop_abort".to_string(),
                    message: loop_message.clone(),
                    retryable: true,
                },
            );
            tool_traces.push(format!(
                "tool={tool_name}; ok=aborted; args={}; output={}",
                serde_json::to_string(&args).unwrap_or_default(),
                truncate_for_prompt(&loop_message, self.skill_runtime_config.max_diff_chars_for_planner)
            ));
            turn_engine.on_failed();
            return NativeToolOutcome::Aborted;
        }
        return NativeToolOutcome::Result {
            content: message,
            ok: false,
        };
    }

    // 5. 构造 ToolCall
    let call = crate::tools::ToolCall {
        name: tool_name.to_string(),
        args: args.clone(),
    };

    let start_time = Instant::now();
    info!(
        turn_id = turn_id,
        tool_call_id = %tool_call_id,
        tool_name = %call.name,
        args = ?call.args,
        "tool_call started"
    );

    self.push_event(
        events,
        Event::ToolCallProgress {
            turn_id,
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            progress_message: "executing".to_string(),
            display_name: self.tools.display_name_for(tool_name),
        },
    );

    // 6. 执行工具 — 使用 execute_tool_with_live_events（原 handle_tool_action 第 199-341 行）
    match self
        .execute_tool_with_live_events(turn_id, tool_call_id, &call, events)
        .await
    {
        Ok(outcome) => {
            let output = outcome.output;
            let ok = outcome.success;
            apply_patch_read_guard.on_tool_success(tool_name);

            // 7. state_epoch 递增（原第 208-210 行）
            if crate::agent::planner_utils::is_mutating_tool(tool_name) {
                self.state_epoch = self.state_epoch.saturating_add(1);
            }

            let duration_ms = start_time.elapsed().as_millis();
            info!(
                turn_id = turn_id,
                tool_call_id = %tool_call_id,
                tool_name = tool_name,
                ok = ok,
                duration_ms = duration_ms,
                output_len = output.len(),
                "tool_call completed"
            );
            let trace = format!(
                "tool={tool_name}; ok={ok}; output={}",
                truncate_for_prompt(&output, self.skill_runtime_config.max_diff_chars_for_planner)
            );
            tool_traces.push(trace);

            // 8. loop_detector 检查（原第 231-265 行）
            let signal = self.loop_detector.check_and_advance(
                tool_name,
                &serde_json::to_string(&args).unwrap_or_default(),
            );
            match signal {
                crate::agent::loop_detector::LoopSignal::Warned => {
                    info!(turn_id, tool_name, "loop_detected: soft interrupt");
                    self.push_event(
                        events,
                        Event::LoopWarning {
                            turn_id,
                            tool_name: tool_name.to_string(),
                            consecutive_count: self.loop_detector.warn_threshold(),
                        },
                    );
                }
                crate::agent::loop_detector::LoopSignal::Halt => {
                    warn!(
                        turn_id,
                        tool_name, "loop_detected: hard halt after recovery failure"
                    );
                    self.push_event(
                        events,
                        Event::ResponseError {
                            turn_id,
                            code: "loop_halt".to_string(),
                            message: "检测到持续重复调用，已强制终止本回合。".to_string(),
                            retryable: true,
                        },
                    );
                    turn_engine.on_failed();
                    return NativeToolOutcome::Aborted;
                }
                crate::agent::loop_detector::LoopSignal::None => {}
            }

            self.record_tool_call(tool_name, &args, ok, &output);

            self.emit_tool_call_completed(
                turn_id,
                tool_call_id,
                tool_name,
                ok,
                &output,
                events,
            );
            *executed_count += 1;
            *consecutive_duplicate_skips = 0;
            NativeToolOutcome::Result { content: output, ok }
        }
        Err(err) => {
            let duration_ms = start_time.elapsed().as_millis();
            let err_text = err.to_string();
            apply_patch_read_guard.on_tool_failure(tool_name, &err_text);
            info!(
                turn_id = turn_id,
                tool_call_id = %tool_call_id,
                tool_name = tool_name,
                ok = false,
                duration_ms = duration_ms,
                error = %err_text,
                "tool_call completed"
            );
            let trace = format!(
                "tool={tool_name}; ok=false; output={}",
                truncate_for_prompt(&err_text, self.skill_runtime_config.max_diff_chars_for_planner)
            );
            tool_traces.push(trace);

            self.record_tool_call(tool_name, &args, false, &err_text);

            // 9. 审批阻塞检查（原第 321-339 行）
            self.emit_tool_call_failed(
                turn_id,
                tool_call_id,
                tool_name,
                &err_text,
                events,
            );
            self.emit_tool_call_completed(
                turn_id,
                tool_call_id,
                tool_name,
                false,
                &err_text,
                events,
            );
            *executed_count += 1;
            *consecutive_duplicate_skips = 0;

            if crate::agent::tool_policy::is_approval_blocking_error(&err_text) {
                let stop_message =
                    if err_text.to_ascii_lowercase().contains("approval timed out") {
                        crate::agent::tool_policy::approval_timed_out_stop_message()
                    } else {
                        crate::agent::tool_policy::approval_rejected_stop_message()
                    };
                self.push_event(
                    events,
                    Event::ResponseError {
                        turn_id,
                        code: "approval_blocked".to_string(),
                        message: stop_message,
                        retryable: false,
                    },
                );
                turn_engine.on_failed();
                return NativeToolOutcome::Aborted;
            }

            NativeToolOutcome::Result { content: err_text, ok: false }
        }
    }
}
```

**逐项对照 `handle_tool_action` 逻辑覆盖：**
| 逻辑块 | 原代码行号 | 新代码位置 |
|--------|-----------|-----------|
| Value→HashMap 转换 | N/A（新） | §1 |
| skill_shell_misfire 跟踪 | 48-52 | §2 |
| git/diff 跟踪 | 53-63 | §2 |
| apply_patch 守卫 | 65-119 | §3 |
| 重复调用检测 | 121-173 | §4 |
| ToolCall 构造 | 175-179 | §5 |
| ToolCallProgress 发射 | 190-197 | §5 |
| execute_tool_with_live_events | 199-201 | §6 |
| state_epoch 递增 | 208-210 | §7 |
| loop_detector 检查 | 231-265 | §8 |
| record_tool_call | 267 | §8 |
| emit_tool_call_completed | 269-276 | §8 |
| approval 阻塞检查 | 321-339 | §9 |

- [ ] **Step 3: 验证编译**

Run: `cargo build -p openjax-core`
Expected: 编译成功（新方法暂无调用者，但自身是完整的）

- [ ] **Step 4: 提交**

```bash
git add openjax-core/src/agent/planner_tool_action.rs
git commit -m "feat(core): 新增 execute_native_tool_call 方法"
```

---

## Task 6: planner.rs — 核心循环重写

**Files:**
- Modify: `openjax-core/src/agent/planner.rs` (651 行)

**这是最关键的改动。** 将 `execute_natural_language_turn` 从 "build_planner_input → dispatcher → DispatchOutcome" 改为 "build_system_prompt + build_turn_messages → ModelResponse → tool_use/tool_result 循环"。同时明确：loop recovery 提示必须每轮刷新，不能只在循环外初始化一次。

**目标行数：** ~400 行（从 651 行减少），因为移除了 dispatcher 分支匹配、JSON repair、DispatchOutcome 处理、ToolActionContext 构造。

- [ ] **Step 1: 重写 imports**

**删除：**
```rust
use crate::agent::planner_tool_batch::BatchExecutionResult;
use crate::agent::planner_utils::{summarize_log_preview, summarize_log_preview_json};
use crate::agent::prompt::build_planner_input;
use crate::dispatcher::{self, DispatchOutcome, ProbeInput};
```

**新增：**
```rust
use crate::agent::planner_tool_action::NativeToolOutcome;
use crate::agent::prompt::{build_system_prompt, build_turn_messages};
use crate::model::{AssistantContentBlock, ConversationMessage, ModelRequest, ModelStage, UserContentBlock};
```

**保留：**
```rust
use std::collections::BTreeMap;
use openjax_protocol::Event;
use tracing::{debug, info, warn};
use crate::Agent;
use crate::agent::tool_guard::ApplyPatchReadGuard;
use crate::agent::turn_engine::TurnEngine;
use crate::logger::AFTER_DISPATCH_LOG_TARGET;
use crate::model::ModelStage;
```

- [ ] **Step 2: 保留 `ToolActionContext` 结构体**

**不要删除 `ToolActionContext`** — 它仍被 `handle_tool_action` 使用。在 Task 8 清理阶段再删除。

- [ ] **Step 3: 重写 `execute_natural_language_turn` 方法**

完整重写。保留所有外围逻辑（rate limiting、loop_detector、TurnEngine、auto-compaction、flow trace logging），但核心循环替换为 native tool calling 模式。

```rust
impl Agent {
    pub(crate) async fn execute_natural_language_turn(
        &mut self,
        turn_id: u64,
        user_input: &str,
        events: &mut Vec<Event>,
    ) {
        let mut tool_traces: Vec<String> = Vec::new();
        let mut executed_count = 0usize;
        let mut planner_rounds = 0usize;
        let mut consecutive_duplicate_skips = 0usize;
        let mut apply_patch_read_guard = ApplyPatchReadGuard::default();
        let mut turn_engine = TurnEngine::new();
        let mut skill_shell_misfire_count = 0usize;
        let mut saw_git_status_short = false;
        let mut saw_git_diff_stat = false;
        let mut diff_strategy: &'static str = "none";

        self.loop_detector.reset();

        // 1. 构建 system_prompt 和初始 messages
        let selected_skills = if self.skill_runtime_config.enabled {
            self.skill_registry.select_for_input(user_input, self.skill_runtime_config.max_selected)
        } else { Vec::new() };
        let skills_context = if self.skill_runtime_config.enabled {
            crate::skills::build_skills_context(&selected_skills, self.skill_runtime_config.max_prompt_chars)
        } else { "(skills disabled)".to_string() };

        let system_prompt = build_system_prompt(&skills_context);
        let initial_loop_recovery = self.loop_detector.recovery_prompt();
        let mut messages = build_turn_messages(user_input, &self.history, initial_loop_recovery.as_deref());
        let tool_specs = self.tools.tool_specs();

        while executed_count < self.max_tool_calls_per_turn
            && planner_rounds < self.max_planner_rounds_per_turn
        {
            // 每轮根据最新 loop_detector 状态刷新 recovery 提示
            refresh_loop_recovery_in_messages(
                &mut messages,
                user_input,
                self.loop_detector.recovery_prompt().as_deref(),
            );
            planner_rounds += 1;

            self.apply_rate_limit().await;

            let request = ModelRequest {
                stage: ModelStage::Planner,
                system_prompt: Some(system_prompt.clone()),
                messages: messages.clone(),
                tools: tool_specs.clone(),
                options: Default::default(),
            };

            let planner_stream = match self
                .request_planner_model_output(turn_id, &request, true, events)
                .await
            {
                Ok(result) => result,
                Err(err) => {
                    warn!(turn_id, error = %err, "planner_stream_error");
                    self.push_event(events, Event::ResponseError {
                        turn_id,
                        code: "planner_stream_error".to_string(),
                        message: format!("模型请求失败: {err}"),
                        retryable: true,
                    });
                    turn_engine.on_failed();
                    return;
                }
            };

            // 更新 last_input_tokens
            if let Some(ref usage) = planner_stream.usage
                && let Some(tokens) = usage.input_tokens
            {
                self.last_input_tokens = Some(tokens);
            }
            self.check_and_auto_compact(turn_id, events).await;

            let response = planner_stream.response;

            // 追加 assistant 消息到 messages
            messages.push(ConversationMessage::Assistant(response.content.clone()));

            // 2. 判断分支：final answer 还是 tool calls
            if !response.has_tool_use() {
                // Final answer
                let final_text = if planner_stream.live_streamed && !planner_stream.streamed_text.is_empty() {
                    planner_stream.streamed_text.clone()
                } else {
                    response.text()
                };

                turn_engine.on_response_started();
                if planner_stream.live_streamed {
                    self.push_event(events, Event::ResponseCompleted {
                        turn_id,
                        content: final_text.clone(),
                        stream_source: openjax_protocol::StreamSource::ModelLive,
                    });
                } else {
                    self.push_event(events, Event::ResponseStarted {
                        turn_id,
                        stream_source: openjax_protocol::StreamSource::Synthetic,
                    });
                    self.emit_synthetic_response_deltas(turn_id, &final_text, events);
                    self.push_event(events, Event::ResponseCompleted {
                        turn_id,
                        content: final_text.clone(),
                        stream_source: openjax_protocol::StreamSource::Synthetic,
                    });
                }

                turn_engine.on_completed();
                self.commit_turn(user_input.to_string(), tool_traces, final_text);
                return;
            }

            // 3. 有 tool_use blocks — 执行
            let tool_uses: Vec<&AssistantContentBlock> = response.tool_uses();

            // 发射 ToolCallsProposed
            let proposals: Vec<openjax_protocol::ToolCallProposal> = tool_uses
                .iter()
                .map(|block| {
                    if let AssistantContentBlock::ToolUse { id, name, input } = block {
                        let arguments = match input {
                            serde_json::Value::Object(map) => map.iter().map(|(k, v)| {
                                let s = match v {
                                    serde_json::Value::String(s) => s.clone(),
                                    other => serde_json::to_string(other).unwrap_or_default(),
                                };
                                (k.clone(), s)
                            }).collect(),
                            _ => BTreeMap::new(),
                        };
                        openjax_protocol::ToolCallProposal {
                            tool_call_id: id.clone(),
                            tool_name: name.clone(),
                            arguments,
                            depends_on: Vec::new(),
                            concurrency_group: None,
                        }
                    } else {
                        openjax_protocol::ToolCallProposal {
                            tool_call_id: String::new(),
                            tool_name: String::new(),
                            arguments: BTreeMap::new(),
                            depends_on: Vec::new(),
                            concurrency_group: None,
                        }
                    }
                })
                .collect();
            self.push_event(events, Event::ToolCallsProposed {
                turn_id,
                tool_calls: proposals,
            });

            turn_engine.on_tool_batch_started();

            let mut tool_result_blocks: Vec<UserContentBlock> = Vec::new();
            let mut aborted = false;

            for block in &tool_uses {
                let AssistantContentBlock::ToolUse { id, name, input } = block else { continue };

                let outcome = self.execute_native_tool_call(
                    turn_id, id, name, input,
                    events, &mut tool_traces,
                    &mut apply_patch_read_guard,
                    &mut consecutive_duplicate_skips,
                    &mut executed_count,
                    &mut turn_engine,
                    &mut skill_shell_misfire_count,
                    &mut saw_git_status_short,
                    &mut saw_git_diff_stat,
                    &mut diff_strategy,
                ).await;

                match outcome {
                    NativeToolOutcome::Aborted => {
                        aborted = true;
                        break;
                    }
                    NativeToolOutcome::Result { content, ok } => {
                        tool_result_blocks.push(UserContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content,
                            is_error: !ok,
                        });
                    }
                }
            }

            if aborted {
                self.push_event(events, Event::ResponseError {
                    turn_id,
                    code: "approval_blocked".to_string(),
                    message: "tool batch interrupted by approval decision".to_string(),
                    retryable: false,
                });
                turn_engine.on_failed();
                return;
            }

            // 发射 ToolBatchCompleted
            // 注意：UserContentBlock 没有 is_error() 方法，使用 pattern matching
            self.push_event(events, Event::ToolBatchCompleted {
                turn_id,
                total: tool_uses.len() as u32,
                succeeded: tool_result_blocks.iter().filter(|r| matches!(r, UserContentBlock::ToolResult { is_error: false, .. })).count() as u32,
                failed: tool_result_blocks.iter().filter(|r| matches!(r, UserContentBlock::ToolResult { is_error: true, .. })).count() as u32,
            });

            // 追加 tool_result 到 messages
            messages.push(ConversationMessage::User(tool_result_blocks));
            turn_engine.on_response_resumed();
        }

        // 达到限制
        let message = if executed_count >= self.max_tool_calls_per_turn {
            format!("已达到单回合最多 {} 次工具调用限制。", self.max_tool_calls_per_turn)
        } else {
            format!("已达到单回合最多 {} 次规划轮次限制。", self.max_planner_rounds_per_turn)
        };
        self.push_event(events, Event::ResponseError {
            turn_id,
            code: "turn_limit_reached".to_string(),
            message: message.clone(),
            retryable: true,
        });
        if matches!(turn_engine.phase(), crate::agent::turn_engine::TurnEnginePhase::Planning) {
            turn_engine.on_failed();
        }
    }
}
```

**关键变更总结：**
- 删除所有 `dispatcher`、`DispatchOutcome`、`BatchExecutionResult` 引用
- 删除 `build_planner_input` 调用，改用 `build_system_prompt` + `build_turn_messages`
- `ToolActionContext` 不再使用 — 改为直接传参给 `execute_native_tool_call`
- `ToolBatchCompleted` 中的 `is_error()` 改用 `matches!` pattern matching
- `execute_native_tool_call` 传入完整的 13 个参数（含 skill_shell_misfire、git/diff 跟踪）
- 技能选择移到循环外（每 turn 一次，非每 round）

**验证 `commit_turn` 签名：** 当前签名应为 `commit_turn(&mut self, user_input: String, tool_traces: Vec<String>, assistant_output: String)` — 确认匹配。

- [ ] **Step 4: 删除不再使用的 `log_after_dispatch_step` 函数**

`log_after_dispatch_step` 函数（第 33-52 行）仅被 dispatcher 路径调用。删除整个函数。

- [ ] **Step 5: 验证编译**

Run: `cargo build -p openjax-core`
Expected: 编译可能因 tests.rs 中的旧 mock 引用失败，这将在 Task 7 修复。planner.rs 自身应编译成功。

- [ ] **Step 6: 提交**

```bash
git add openjax-core/src/agent/planner.rs
git commit -m "refactor(core): 重写 planner 循环为 native tool calling"
```

---

## Task 7: tests.rs — 更新 mock models

**Files:**
- Modify: `openjax-core/src/tests.rs` (927 行)

**这是工作量最大的文件。** 所有 mock `ModelClient` 的 `complete_stream` 和 `complete` 返回值需从 JSON 文本改为 native content blocks。

- [ ] **Step 1: 更新 `text_response` 辅助函数**

当前返回 `ModelResponse { content: vec![Text{text}], ..default() }`。这本身是正确的，但需要确保 `stop_reason` 被正确设置。

```rust
fn final_response(text: impl Into<String>) -> ModelResponse {
    ModelResponse {
        content: vec![AssistantContentBlock::Text { text: text.into() }],
        stop_reason: Some(StopReason::EndTurn),
        ..ModelResponse::default()
    }
}

fn tool_use_response(tool_calls: Vec<(&str, &str, serde_json::Value)>) -> ModelResponse {
    let content: Vec<AssistantContentBlock> = tool_calls
        .into_iter()
        .enumerate()
        .map(|(i, (name, id, input))| AssistantContentBlock::ToolUse {
            id: id.to_string(),
            name: name.to_string(),
            input,
        })
        .collect();
    ModelResponse {
        content,
        stop_reason: Some(StopReason::ToolUse),
        ..ModelResponse::default()
    }
}
```

- [ ] **Step 2: 更新 `ScriptedStreamingModel`**

`complete_stream` 和 `complete` 返回 `final_response("seed")` 而非 `text_response(r#"{"action":"final","message":"seed"}"#)`。

流式 delta 仍发送 `StreamDelta::Text("seed")` 文本片段，但 `ModelResponse` 的 `content` 是 `[Text{text:"seed"}]`、`stop_reason: Some(EndTurn)`。

- [ ] **Step 3: 更新 `ScriptedToolBatchModel`**

首次调用返回 `tool_use_response(vec![("list_dir", "call_1", json!({"path":"."})), ("system_load", "call_2", json!({}))])`。
第二次调用返回 `final_response("batch done")`。

- [ ] **Step 4: 更新 `ScriptedToolBatchDependencyModel`**

类似 batch model，但 `call_2` 的 `input` 包含完整参数。

- [ ] **Step 5: 更新 `PlannerFallbackModel`**

不再测试 "JSON 解析失败 → fallback"。改为测试正常流式返回：
- `complete_stream` 返回 `final_response("fallback final")`
- `complete` 也返回 `final_response("fallback final")`

- [ ] **Step 6: 更新 `DuplicateToolLoopModel`**

每次调用返回 `tool_use_response(vec![("shell", "call_1", json!({"cmd":"echo hi"}))])`。

- [ ] **Step 7: 更新 `ApprovalBlockedBatchModel`**

首次调用返回 `tool_use_response(vec![...3 个 tool_use...])`。第二次调用（不应到达）返回 `final_response("should not be reached")`。

- [ ] **Step 8: 更新 `ApprovalCancellationBatchModel`**

类似，返回 2 个 tool_use blocks。

- [ ] **Step 9: 删除不再适用的测试**

删除以下 6 个测试函数：
- `normalizes_tool_name_in_action_with_top_level_args`
- `keeps_explicit_tool_shape_unchanged`
- `keeps_final_action_unchanged`
- `planner_prompt_contains_apply_patch_verification_rule`
- `planner_prompt_contains_skills_section`
- `planner_stream_parse_failure_falls_back_to_complete_response`

同时删除不再需要的 imports（如 `parse_model_decision`、`normalize_model_decision`、`build_planner_input`）。

- [ ] **Step 10: 添加替代测试**

至少补上以下测试：
- `build_system_prompt_contains_verification_rule`
- `build_system_prompt_contains_skills_section`
- `build_turn_messages_includes_prior_conversation_summary`
- `refresh_loop_recovery_only_updates_last_user_text`
- `planner_stream_tool_events_preserve_tool_name_across_args_delta_and_ready`

- [ ] **Step 11: 验证所有测试通过**

Run: `cargo test -p openjax-core`
Expected: 所有测试通过

- [ ] **Step 12: 提交**

```bash
git add openjax-core/src/tests.rs
git commit -m "refactor(core): 更新 mock models 为 native tool calling"
```

---

## Task 8: 清理旧代码

**Files:**
- Modify: `openjax-core/src/agent/prompt.rs`
- Modify: `openjax-core/src/agent/planner_tool_action.rs`
- Modify: `openjax-core/src/agent/planner.rs`（删除 `ToolActionContext`）
- Modify: `openjax-core/src/dispatcher/mod.rs`（标记废弃）
- Modify: `openjax-core/src/agent/mod.rs`（删除 `planner_tool_batch` mod 声明，如存在）

- [ ] **Step 1: 从 `prompt.rs` 删除 `build_planner_input`**

确认无其他调用者后删除整个函数。用 `grep` 搜索确认：

Run: `grep -r "build_planner_input" openjax-core/src/`
Expected: 仅在 `prompt.rs` 中的定义处出现（planner.rs 已在 Task 6 中移除引用）

- [ ] **Step 2: 从 `prompt.rs` 删除 `build_json_repair_prompt`**

同样确认无引用后删除。搜索确认：

Run: `grep -r "build_json_repair_prompt" openjax-core/src/`
Expected: 仅在 `prompt.rs` 中的定义处出现

- [ ] **Step 3: 从 `planner_tool_action.rs` 删除 `handle_tool_action`**

确认 `handle_tool_action` 不再被任何地方调用后删除。同时删除相关 imports（`use crate::agent::decision::ModelDecision`）。

Run: `grep -r "handle_tool_action" openjax-core/src/`
Expected: 仅在 `planner_tool_action.rs` 中的定义处出现

- [ ] **Step 4: 从 `planner.rs` 删除 `ToolActionContext` 结构体**

`ToolActionContext`（第 19-31 行）不再被任何代码使用。删除整个结构体及其相关 import。

- [ ] **Step 5: 标记 `planner_tool_batch.rs` 为废弃**

`planner_tool_batch.rs` 不再被 planner.rs 调用（`execute_tool_batch_calls` 和 `BatchExecutionResult` 不再使用）。

**选项 A（推荐）：** 如果 `planner_tool_batch.rs` 没有其他调用者，直接删除文件并从 `mod.rs` 移除 `pub mod planner_tool_batch;`。

**选项 B：** 在文件顶部添加 `#[deprecated]` 注释，暂不删除。

Run: `grep -r "planner_tool_batch" openjax-core/src/`
Expected: 仅在 mod 声明和文件自身中出现

- [ ] **Step 6: 标记 `decision.rs` 为废弃**

`DecisionJsonStreamParser`、`parse_model_decision`、`normalize_model_decision`、`ModelDecision` 不再被 planner 路径使用。

Run: `grep -r "decision::" openjax-core/src/agent/`
Expected: 仅在 decision.rs 自身和可能的测试中出现。如果 planner_tool_action.rs 已在 Step 3 中清理了 `ModelDecision` import，则 agent/ 目录中不应有其他引用。

- [ ] **Step 7: 标记 `dispatcher/mod.rs` 为废弃**

在 `dispatcher/mod.rs` 的 `route_model_output` 函数上添加 `#[deprecated]`。

- [ ] **Step 8: 验证编译和测试**

Run: `cargo test -p openjax-core`
Expected: 所有测试通过

- [ ] **Step 9: 提交**

```bash
git add openjax-core/src/agent/prompt.rs openjax-core/src/agent/planner_tool_action.rs openjax-core/src/agent/planner.rs
git commit -m "refactor(core): 清理旧 Planner Prompt 代码路径"
```

---

## Task 9: 全量回归测试

**Files:** 无代码变更

- [ ] **Step 1: 运行 openjax-core 全量测试**

```bash
cargo test -p openjax-core
```
Expected: 全部通过

- [ ] **Step 2: 运行集成测试套件**

```bash
cargo test -p openjax-core --test tools_sandbox_suite
cargo test -p openjax-core --test approval_suite
cargo test -p openjax-core --test streaming_suite
cargo test -p openjax-core --test skills_suite
cargo test -p openjax-core --test core_history_suite
```
Expected: 全部通过

- [ ] **Step 3: 运行 clippy**

```bash
cargo clippy -p openjax-core --all-targets -- -D warnings
```
Expected: 无 warning

- [ ] **Step 4: 最终提交**

如果需要修复任何 clippy 警告：
```bash
git add -A
git commit -m "chore(core): Phase 3 native tool calling 完成"
```
