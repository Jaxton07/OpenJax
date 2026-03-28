# Phase 3: Agent Loop — Native Tool Calling

> Date: 2026-03-28
> Status: Draft
 fixes applied
 spec Reviewer feedback
 see CHANGELOG below
> Parent: `docs/plan/refactor/tools/native-tool-calling-plan.md`
> Prerequisites: Phase 1 (types.rs) ✅, Phase 2 (adapters) ✅

---

## Goal

 Replace the custom "Planner Prompt" loop (JSON text → dispatcher → DispatchOutcome) with a native tool calling loop (ModelResponse.content → tool_use blocks → tool_result blocks). The dispatcher, DecisionJsonStreamParser, and JSON repair paths are removed from the planner's hot path.

 The approach

 Three sequential sub-phases, each independently compilable: | Sub-phase | Scope | Files | Breaks anything? |
|-----------|-------|-------|-----------------|
| 3a | Foundation | `router_impl.rs`, `prompt.rs` | No — pure additions |
| 3b | Stream simplification | `planner_stream_flow.rs` | Planner still uses old path |
| 3c | Core loop + tests | `planner.rs`, `planner_tool_action.rs`, `tests.rs` | Core rewrite |

---

## Sub-phase 3a: Foundation

 ### 3a.1 `tools/router_impl.rs` — expose tool_specs

 Add one method: ```rust
impl ToolRouter {
    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.specs.clone()
    }
```

 `display_name_for()` already exists ( returns `Option<String>` ). No other changes.

 ### 3a.2 `agent/prompt.rs` — new functions alongside old ones Add two new functions. Keep `build_planner_input` and `build_json_repair_prompt` alive (3c deletes them). **`build_system_prompt(skills_context: &str) -> String`** Extract the non-JSON-schema content from `build_planner_input`: persona, behavior rules, tool selection policy, skills context. Strip all JSON format instructions, tool name enumeration, and action schema rules (those are now conveyed via native `tools` parameter).

 Content (preserved from `build_planner_input`, minus JSON schema):
 - Persona: "You are OpenJax, an all-purpose personal AI assistant."
- "If task can be answered now, respond with the final answer directly."
 - "In final answer, avoid mentioning internal planning, hidden reasoning, or tool traces unless the user explicitly asks."
 - "If required information is missing, ask one concise clarification question."
 - "If verification already shows the requested content/changes are present, respond immediately."
 - Tool selection policy (read_file before edit, apply_patch for multi-file, etc.)
- apply_patch format rules (argument formatting, not tool discovery)
- edit_file_range argument rules
- Shell workspace-relative preference
- Skills invocation rule: skill markers like `/skill-name` are not shell executables
- No-repeat policy: "Do NOT repeat the same tool call with the same arguments." - "If verification already show the requested content/changes are present, respond immediately." - Skills context block **Removed from prompt (now handled by native `tools` parameter):**
- "Return ONLY valid JSON" instruction
- JSON schema (`{"action":"tool",...}`, `{"action":"final",...}` )
- Tool name enumeration (`read_file|list_dir|grep_files|process_snapshot|system_load|disk_usage|shell|apply_patch|edit_file_range`)
- "At most one action per response"
 - "All values inside args MUST be JSON strings" - "At most one action per response" - **`build_turn_messages(user_input: &str, history: &[HistoryItem], loop_recovery: Option<&str>) -> Vec<ConversationMessage>`** Build the `Vec<ConversationMessage>` for the model request: - If history is non-empty, inject first message as `<prior_conversation>` text summary (same format as current `build_planner_input` history section) - Current `user_input` as the last `ConversationMessage::User(vec![Text { text: user_input }])`  - `loop_recovery` appended to user_input if present **Note:** `tool_traces` (current turn's tool execution history) is not passed to `build_planner_input` but the in the new loop they they are naturally in `messages` via tool_use/tool_result ConversationMessage pairs `commit_turn` still collects `tool_traces` as `Vec<String>` in the same way ( same format as current code: `handle_tool_action` / `execute_tool_batch_calls`), for `commit_turn`. In new loop, `tool_traces` collected from `execute_native_tool_call` by formatting `"tool_name(args) → result_string"` for each executed tool. **This does be `commit_turn` recording.** **Do NOT add this as a parameter to `build_turn_messages`.** `build_turn_messages` handles `history` -> tool_traces not `tool_traces` are injected via `<prior_conversation>` section, which has the `TurnRecord.tool_traces`, and in the loop, `tool_traces` are part of `ConversationMessage` (tool_result blocks).

 ---

## Sub-phase 3b: Stream Flow Simplification

 ### 3b.1 `agent/planner_stream_flow.rs` — remove JSON parsing **Current flow:**
1. Stream model output through `DecisionJsonStreamParser`
 2. Try `parse_model_decision` on result
 3. If parse fails, try reconstructing from streamed message
 4. If still fails, fallback to `model_client.complete()` (non-streaming)
 5. Return `PlannerStreamResult { model_output: String, ... }`

**New flow:**
1. Stream model deltas: Text → orchestrator, ToolUseStart/ArgsDelta/End → events, Reasoning → event) 2. Collect streamed text from Text deltas
3. Return `PlannerStreamResult` with `response: ModelResponse` directly
 4. No JSON parsing, no fallback to `complete()`

**Event emission strategy (critical):**
The the the **stream phase** (`planner_stream_flow.rs`), only `ToolUseStart`, `ToolCallArgsDelta`, `ToolCallReady` are streaming events. The execution loop in `planner.rs` emits `ToolCallCompleted`. This avoids duplicates `ToolCallStarted`:
- **Stream phase**: emits `ToolCallArgsDelta` + `ToolCallReady` (arg progress + completion notification)
- **Execution phase**: emits `ToolCallCompleted` (result + ok/error)

- **Neither phase** re-emits `ToolCallStarted` — this event is a separate dedicated event for "tool call submitted to model for execution" that precedes the streaming. This is NOT `ToolCallStarted` from the current codebase — it's a new behavior to this flow. **ToolCallStarted` is the stream phase signals for "tool call arguments arriving" to TUI can show a progress. **ToolCallReady` in stream phase signals "arguments complete, ready to execute". **ToolCallCompleted` in execution phase signal "execution done, here's the result".

This design avoids emitting duplicate events types.

 Each tool call gets a clean event triple: `ToolCallStarted` (stream) → `ToolCallReady` (stream) → `ToolCallCompleted` (execution).
Note: Some existing TUI consumers code may emit itsToolCallStarted` separately before execution. In such cases, both the stream event and the separate `ToolCallStarted` would be emitted. If such a consumer exists, it should be updated to subscribe to `ToolCallReady` instead, or the new planner should emit `ToolCallStarted` in the execution phase instead of the stream phase. For Phase 3, we'll keep the stream phase emissions (`ToolCallStarted` + `ToolCallReady`) and NOT re remit `ToolCallStarted` in the execution phase, since existing TUI code already subscri to stream events.

**New `PlannerStreamResult`:**
```rust
pub(super) struct PlannerStreamResult {
    pub(super) response: ModelResponse,
    /// Text accumulated from StreamDelta::Text deltas (for final answer streaming).

    /// Unlike old `model_output` (raw JSON string), this is `response.text()` (parsed `ModelResponse`).
    pub(super) streamed_text: String,
    pub(super) live_streamed: bool,
    pub(super) usage: Option<ModelUsage>,
}
```
**Removed:** `DecisionJsonStreamParser` import and usage, `parse_model_decision` call, `action_hint` field, `model_output: String` field, fallback-to-complete logic.
**Kept:** `emit_synthetic_response_deltas`, TTFT logging, `ResponseStreamOrchestrator`, all `StreamDelta` event handling.
**Error handling:** If `complete_stream` fails, return error directly — no retry with `complete()`.

---

## Sub-phase 3c: Core Loop Rewrite + Tests

 ### 3c.1 `agent/planner.rs` — new loop

**Current flow (simplified):**
```
while under limits:
    prompt = build_planner_input(...)
    result = request_planner_model_output(turn_id, &request, true, events)
    routed = dispatcher::route_model_output(result.model_output, ...)
    match routed:
        ToolBatch → execute_tool_batch_calls
        Tool → handle_tool_action
        Final → emit response, commit_turn, return
        Repair → attempt JSON repair
        Error → emit error, return
```
**New flow:**
```
system_prompt = build_system_prompt(&skills_context)
messages = build_turn_messages(user_input, &history, loop_recovery)
tool_specs = self.tools.tool_specs()

while under limits:
    request = ModelRequest { stage: Planner, system_prompt, messages, tools: tool_specs, options }
    result = request_planner_model_output(turn_id, &request, true, events)
    response = result.response

    // Append assistant response to messages
    messages.push(ConversationMessage::Assistant(response.content.clone()))

    if !response.has_tool_use():
        // Final answer — emit events, commit, return
        let final_text = response.text()
        emit_final_response(...)
        commit_turn(user_input, tool_traces, final_text)
        return

    // Collect tool_use blocks, emit ToolCallsProposed
    // NOTE: planner_stream_flow already emitted ToolCallStarted/ToolCallReady during streaming.
    // Here we only emit ToolCallCompleted.
    let tool_uses: Vec<&AssistantContentBlock> = response.tool_uses()
    emit ToolCallsProposed event
    let mut tool_result_blocks: Vec<UserContentBlock> = Vec::new()

    for tool_use in tool_uses:
        let AssistantContentBlock::ToolUse { id, name, input } = tool_use else { continue };

        let display_name = self.tools.display_name_for(name);
        let target = extract_target(name, input);

        // execute_native_tool_call emits ToolCallCompleted internally
        let outcome = execute_native_tool_call(
            turn_id, id, name, input, events, &mut tool_traces,
            &mut apply_patch_read_guard, &mut consecutive_duplicate_skips,
            &mut turn_engine, ...
        );

        match outcome:
            Aborted → emit error, return
            Result { content, ok } =>
                tool_result_blocks.push(UserContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content,
                    is_error: !ok,
                })
                executed_count += 1

    // Check all tools completed
    emit ToolBatchCompleted { total: tool_uses.len(), ... }
    messages.push(ConversationMessage::User(tool_result_blocks))
    // continue loop
```

**Key changes:**
- `dispatcher::route_model_output()` — no longer called
- `DecisionJsonStreamParser` — no longer used
- JSON repair path — removed entirely
- `DispatchOutcome` — not used
- `build_planner_input` — replaced by `build_system_prompt` + `build_turn_messages`
- Tool execution uses existing `ToolRouter::execute()` with `ToolCall` constructed from `AssistantContentBlock::ToolUse`

**Preserved logic (migrated to new loop):**
- `ApplyPatchReadGuard` — checked in `execute_native_tool_call`
- Duplicate tool call detection — checked in `execute_native_tool_call`
- Loop detection via `loop_detector` — checked after each tool execution
- `TurnEngine` state machine events — `on_response_started/completed/failed`
- Rate limiting — `apply_rate_limit()` before each model call
- Skill context construction
- `max_tool_calls_per_turn` / `max_planner_rounds_per_turn` limits
- `tool_traces` recording for `commit_turn`
- Auto-compaction (`check_and_auto_compact`)
- Flow trace logging

 ### 3c.2 `agent/planner_tool_action.rs` — new `execute_native_tool_call`

New method alongside `handle_tool_action`. Logic is identical but input changes:

**Old:** `handle_tool_action(turn_id, decision: &ModelDecision, ctx: &mut ToolActionContext)`

**New:** `execute_native_tool_call(turn_id, tool_call_id: &str, tool_name: &str, input: &Value, ctx: &mut ToolActionContext)`

Key differences:
- `args` comes from `serde_json::Value` (native tool call input) instead of `HashMap<String, String>` (parsed from JSON text)
- Convert `Value` to `HashMap<String, String>` by flattening: `Value::String(s) → s`, other values → `serde_json::to_string(&v)`
- `tool_call_id` provided by model (native), not generated
- Approval handling preserved
- All guards preserved (apply_patch_read_guard, duplicate detection, loop detection)
- Returns `ToolExecOutcome` — `Result { content: String, ok: bool }` or `Aborted`

**Note on `ToolActionContext`:** Reuses existing `ToolActionContext` struct from `planner.rs` with the same fields. No new context struct needed. ### 3c.3 Converting ToolUse to ToolCall for ToolRouter
 `ToolRouter::execute()` expects `ToolExecutionRequest` with a `ToolCall { name, args: HashMap<String, String> }`. Conversion: ```rust
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

### 3c.4 `tests.rs` — update mock models

All mock `ModelClient` implementations must return native content blocks:

| Mock | Old return | New return |
|------|-----------|------------|
| `ScriptedStreamingModel` | JSON text `{"action":"final","message":"seed"}` | `ModelResponse { content: vec![Text{text:"seed"}], stop_reason: Some(EndTurn) }` |
| `ScriptedToolBatchModel` | JSON text `{"action":"tool_batch",...}` | `ModelResponse { content: vec![ToolUse{id,name,input}, ...], stop_reason: Some(ToolUse) }` on first call, `Text{text:"batch done"}` + `EndTurn` on second |
| `DuplicateToolLoopModel` | JSON text `{"action":"tool",...}` | `ToolUse{...}` + `StopReason::ToolUse` every call |
| `ApprovalBlockedBatchModel` | Same batch pattern | Same ToolUse pattern |
| `ApprovalCancellationBatchModel` | Same batch pattern | Same ToolUse pattern |
| `PlannerFallbackModel` | Invalid JSON text (tests fallback) | Tests normal streaming response (no fallback mechanism in native) |
| `ScriptedToolBatchDependencyModel` | JSON with `depends_on` | ToolUse blocks with `depends_on` handled at execution level |

**Deleted tests:**
- `planner_prompt_contains_apply_patch_verification_rule` — depends on `build_planner_input`
- `planner_prompt_contains_skills_section` — depends on `build_planner_input`
- `planner_stream_parse_failure_falls_back_to_complete_response` — no fallback in native
 `normalizes_tool_name_in_action_with_top_level_args` — depends on `parse_model_decision`
- `keeps_explicit_tool_shape_unchanged` — depends on `parse_model_decision`
- `keeps_final_action_unchanged` — depends on `parse_model_decision`

**Preserved tests (unchanged):**
- `duplicate_detection_*` — pure Agent method tests
- `parse_runtime_policies` — pure config parsing
- `resolves_turn_limits_from_config_and_env` — pure config parsing
- `aborts_after_consecutive_duplicate_skips` — pure logic
- `summarize_user_input_*` — pure utility function

 ### 3c.5 Cleanup — delete old code paths

 After 3c.4 passes all tests:
 - Delete `build_planner_input` from `prompt.rs`
- Delete `build_json_repair_prompt` from `prompt.rs`
- Delete `handle_tool_action` from `planner_tool_action.rs` (replaced by `execute_native_tool_call`)
- Mark `dispatcher::route_model_output` as `#[deprecated]`
- Remove `dispatcher_config` and `tool_batch_v2_enabled` from `Agent` struct (or leave as dead fields for now) ---

## Files Changed Summary

 | File | Sub-phase | Change Type |
|------|-----------|-------------|
| `tools/router_impl.rs` | 3a | Add `tool_specs()` |
| `agent/prompt.rs` | 3a | Add `build_system_prompt`, `build_turn_messages` |
| `agent/planner_stream_flow.rs` | 3b | Rewrite: remove JSON parsing, return ModelResponse |
| `agent/planner.rs` | 3c | Core rewrite: native tool calling loop |
| `agent/planner_tool_action.rs` | 3c | Add `execute_native_tool_call` |
| `tests.rs` | 3c | Update all mocks, delete 6 tests |
| `agent/decision.rs` | 3c (cleanup) | Unused, mark deprecated |
| `dispatcher/mod.rs` | 3c (cleanup) | No longer called from planner |

## Risks and Mitigations

 1. **ToolBatchCompleted event compatibility**: TUI and gateway consumers expect this event. New loop must still emit it after all tool_use blocks complete. → Emit `ToolBatchCompleted { total, ... }` after the tool execution loop.

2. **ToolCallsProposed event**: Existing consumers expect `arguments: BTreeMap<String, String>`. Native tool_use has `input: serde_json::Value`. → Flatten Value to String map in the same way as tool_use_to_call conversion.

 ### 3. **Duplicate event emission**: Stream phase emits `ToolCallStarted`/`ToolCallReady`, execution phase emits `ToolCallCompleted`. Current TUI code subscri to stream-phase `ToolCallStarted`. → Keep stream emissions unchanged; execution does NOT re-emit `ToolCallStarted`. Each tool gets a clean triple: `ToolCallStarted` (stream) → `ToolCallReady` (stream) → `ToolCallCompleted` (execution).

 ### 4. **depends_on handling**: Current batch model has dependency resolution in `planner_tool_batch.rs`. In native tool calling, the model returns all tool_uses in one response and the loop executes them sequentially. → For 3c, execute tool_uses sequentially (no dependency resolution needed since model manages ordering). Batch parallel execution is an an optimization that can be re-added later.

5. **context_compressor compatibility**: `commit_turn` expects `tool_traces: Vec<String>`. The new loop records this the same way — in `execute_native_tool_call`, format `"tool_name(key=args) → result_summary"`. No change to `commit_turn` interface.

 ### 6. **ModelRequest.for_stage compatibility**: Tests and other callers use `ModelRequest::for_stage()`. The new planner constructs `ModelRequest` directly. → Keep `for_stage()` as convenience constructor for tests.

 ---

## Testing Strategy

 After each sub-phase:
```bash
cargo test -p openjax-core
```

 After 3c completion (full regression):
```bash
cargo test -p openjax-core --test tools_sandbox_suite
cargo test -p openjax-core --test approval_suite
cargo test -p openjax-core --test streaming_suite
cargo test -p openjax-core --test skills_suite
cargo test -p openjax-core --test core_history_suite
cargo test -p openjax-core
```
