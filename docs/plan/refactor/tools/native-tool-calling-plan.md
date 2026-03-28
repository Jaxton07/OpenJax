# OpenJax Native Tool Calling 迁移计划

> 创建时间：2026-03-27
> 状态：历史计划（Historical Context）
> 替代：tool-optimization-plan.md（P1/P2/P3/P5 被本计划吸收，P4/P6/P7 保留为独立子任务）
> 当前收口基线请改看：
> - `docs/superpowers/specs/2026-03-28-native-tool-calling-remaining-phases-design.md`
> - `docs/superpowers/plans/2026-03-28-native-tool-calling-remaining-phases.md`

> 说明：本文保留“迁移发起时”的目标与分解，用于历史追踪。后续 Phase 4-6 的执行口径以上述 remaining phases spec/plan 为准，不再以本文作为 active implementation baseline。

---

## 一、背景与动机（历史视角）

在本计划创建时，OpenJax 的工具调用采用"Planner Prompt"架构：将工具名枚举、格式规则、对话历史全部拼进一条超长 user message，让模型输出自定义 JSON（`{"action":"tool","tool":"..","args":{}}`），再由 OpenJax 解析执行，结果以文本形式拼回下一轮 prompt。

这与 Claude Code 的 Native Tool Calling 架构存在本质差异：

| 维度 | 当前（Planner Prompt） | 目标（Native Tool Calling） |
|------|----------------------|--------------------------|
| 工具传递 | 硬编码在 prompt 文本里 | 通过 API `tools` 参数传递 |
| 模型决策 | 输出自定义 JSON 字符串 | 输出结构化 `tool_use` 块 |
| 工具结果 | 拼入下一轮 user message | `tool_result` content block |
| JSON 修复 | 需要 repair 机制 | 不需要（模型原生结构化输出） |
| 多工具并行 | 模拟实现 | 模型原生支持 |
| 参数类型 | 提示词和代码不一致 | 模型严格按 JSON Schema 输出 |
| model/TUI 数据共用 | shell output 同一字符串 | 通过分离通道彻底解耦 |

迁移目标：让 OpenJax 的工具调用质量、顺滑度对齐 Claude Code。

---

## 二、现有代码关键事实（迁移基准）

### 已有的"预留位置"（无需从零开始）

```rust
// model/types.rs — 已有但未使用
pub struct ModelRequest {
    pub tool_results: Vec<ToolResult>,  // 预留，当前 build_request() 忽略
}
pub struct CapabilityFlags {
    pub tool_call: bool,  // 预留，所有 adapter 当前设为 false
}

// openjax-protocol — ToolCallCompleted 等事件已完整定义
Event::ToolCallCompleted { turn_id, tool_call_id, tool_name, ok, output, .. }
Event::ToolCallArgsDelta { .. }  // 流式工具参数 delta 已有
Event::ToolCallsProposed { .. }
```

### 需要重写的核心路径

```
anthropic_messages.rs::build_request()
  → messages: vec![AnthropicMessage { content: String }]   // 只有一条 user 消息

agent/planner.rs::execute_natural_language_turn()
  → build_planner_input(...)  // 全部历史+工具规则拼成一条 prompt

dispatcher/mod.rs::route_model_output()
  → parse_model_decision(model_output)  // 解析自定义 JSON
```

---

## 三、目标架构

### 请求结构（迁移后）

**Anthropic Messages API：**
```json
{
  "model": "claude-sonnet-4-6",
  "system": "You are OpenJax, an all-purpose personal AI assistant...\n\n<tool_policy>...</tool_policy>",
  "tools": [
    {"name": "read_file", "description": "...", "input_schema": {...}},
    {"name": "shell",     "description": "...", "input_schema": {...}}
  ],
  "messages": [
    {"role": "user",      "content": "帮我看一下 src/main.rs"},
    {"role": "assistant", "content": [
      {"type": "tool_use", "id": "tu_01", "name": "read_file", "input": {"file_path": "src/main.rs"}}
    ]},
    {"role": "user", "content": [
      {"type": "tool_result", "tool_use_id": "tu_01", "content": "L1: fn main() {...}"}
    ]},
    {"role": "assistant", "content": [
      {"type": "text", "text": "main.rs 的入口函数是..."}
    ]}
  ]
}
```

**OpenAI Chat Completions API（同理）：**
```json
{
  "model": "gpt-4o",
  "tools": [{"type": "function", "function": {"name": "read_file", "description": "...", "parameters": {...}}}],
  "messages": [
    {"role": "system",    "content": "You are OpenJax..."},
    {"role": "user",      "content": "帮我看一下 src/main.rs"},
    {"role": "assistant", "tool_calls": [{"id": "call_01", "type": "function", "function": {"name": "read_file", "arguments": "{\"file_path\":\"src/main.rs\"}"}}]},
    {"role": "tool",      "tool_call_id": "call_01", "content": "L1: fn main() {...}"},
    {"role": "assistant", "content": "main.rs 的入口函数是..."}
  ]
}
```

### Agent 循环（迁移后）

```
turn_start:
  messages = [history_summary_msgs..., user_msg]

loop:
  response = model.complete(system, tools, messages)
  if response.stop_reason == EndTurn:
    emit ResponseCompleted
    break
  for tool_call in response.tool_calls:
    emit ToolCallStarted
    result = execute_tool(tool_call)
    emit ToolCallCompleted (with metadata in event, clean content in tool_result)
    messages.push(assistant_msg(tool_call))
    messages.push(tool_result_msg(tool_call.id, result.model_content))
```

---

## 四、分阶段实施计划

执行顺序：**Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5**

每个 Phase 完成后独立可构建、可测试，不影响其他模块。

---

### Phase 1：Model 层数据类型重构

**目标**：重新定义 `ModelRequest` / `ModelResponse` 以承载多轮对话和工具调用，同时保持 adapter 接口不变。

#### 1.1 `openjax-core/src/model/types.rs`

**新增类型：**

```rust
/// 对话消息（多轮）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversationMessage {
    User(Vec<UserContentBlock>),
    Assistant(Vec<AssistantContentBlock>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserContentBlock {
    Text { text: String },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// 模型停止原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Other(String),
}
```

**修改 `ModelRequest`：**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub stage: ModelStage,
    /// 系统提示词（覆盖 adapter 默认值）
    pub system_prompt: Option<String>,
    /// 多轮对话消息列表（取代 user_input）
    pub messages: Vec<ConversationMessage>,
    /// 要传递给模型的工具规范
    pub tools: Vec<ToolSpec>,
    pub options: ModelRequestOptions,
}

impl ModelRequest {
    /// 向后兼容：单条 user 消息构建（Phase 1 测试用，Phase 3 后废弃）
    pub fn for_stage(stage: ModelStage, user_input: impl Into<String>) -> Self {
        Self {
            stage,
            system_prompt: None,
            messages: vec![ConversationMessage::User(vec![
                UserContentBlock::Text { text: user_input.into() }
            ])],
            tools: Vec::new(),
            options: ModelRequestOptions::default(),
        }
    }
}
```

**修改 `ModelResponse`：**

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelResponse {
    /// 助手输出的所有内容块（文本 + 工具调用）
    pub content: Vec<AssistantContentBlock>,
    pub usage: Option<ModelUsage>,
    pub stop_reason: Option<StopReason>,
    /// 原始响应体（调试用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

impl ModelResponse {
    /// 提取纯文本内容
    pub fn text(&self) -> String {
        self.content.iter()
            .filter_map(|b| if let AssistantContentBlock::Text { text } = b { Some(text.as_str()) } else { None })
            .collect::<Vec<_>>()
            .join("")
    }

    /// 提取所有 tool_use 块
    pub fn tool_uses(&self) -> Vec<&AssistantContentBlock> {
        self.content.iter()
            .filter(|b| matches!(b, AssistantContentBlock::ToolUse { .. }))
            .collect()
    }

    pub fn has_tool_use(&self) -> bool {
        self.content.iter().any(|b| matches!(b, AssistantContentBlock::ToolUse { .. }))
    }

    pub fn stop_is_tool_use(&self) -> bool {
        matches!(self.stop_reason, Some(StopReason::ToolUse))
    }
}
```

**删除旧字段：**
- 删除 `ModelRequest::user_input: String`
- 删除 `ModelRequest::tool_results: Vec<ToolResult>`（被 `messages` 替代）
- 删除 `ModelResponse::text: String`（被 `content` 替代）
- 删除 `ModelResponse::reasoning: Option<String>`（通过 `AssistantContentBlock` 或单独 Thinking block 扩展，Phase 1 暂保留为 Option）
- 删除 `StreamDelta::Reasoning`（暂保留，Phase 2 视流式实现情况处理）

> **注意**：`ToolSpec` 目前在 `openjax-core/src/tools/spec.rs`，需要在 `types.rs` 中重新导出或直接引用。Phase 1 可暂用 `serde_json::Value` 占位，Phase 2 完成后换成真实类型。

#### 1.2 涉及文件

| 文件 | 改动类型 |
|------|---------|
| `openjax-core/src/model/types.rs` | 主要重构 |

#### 1.3 测试

```rust
// openjax-core/src/model/types.rs #[cfg(test)]
#[test]
fn model_response_text_extracts_only_text_blocks() { ... }

#[test]
fn model_response_tool_uses_extracts_tool_use_blocks() { ... }

#[test]
fn conversation_message_serde_roundtrip() { ... }

#[test]
fn model_request_for_stage_compat_wraps_user_text() { ... }
```

---

### Phase 2：Adapter 层 — 原生工具调用支持

**目标**：让 `AnthropicMessagesClient` 和 `ChatCompletionsClient` 真正发送 `tools` 字段、解析 `tool_use` 响应。

#### 2.1 `openjax-core/src/model/anthropic_messages.rs`

**修改 `AnthropicMessagesRequest`：**

```rust
#[derive(Debug, Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    system: String,
    messages: Vec<AnthropicApiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicToolDef>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<AnthropicThinking>,
}

#[derive(Debug, Serialize)]
struct AnthropicToolDef {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AnthropicApiMessage {
    role: String,  // "user" | "assistant"
    content: AnthropicContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String, #[serde(skip_serializing_if = "std::ops::Not::not")] is_error: bool },
}
```

**修改 `build_request()`：**

```rust
fn build_request(&self, request: &ModelRequest, stream: bool, thinking: Option<AnthropicThinking>) -> AnthropicMessagesRequest {
    let messages = request.messages.iter().map(|msg| match msg {
        ConversationMessage::User(blocks) => AnthropicApiMessage {
            role: "user".to_string(),
            content: if blocks.len() == 1 {
                if let UserContentBlock::Text { text } = &blocks[0] {
                    AnthropicContent::Text(text.clone())
                } else {
                    AnthropicContent::Blocks(blocks.iter().map(user_block_to_api).collect())
                }
            } else {
                AnthropicContent::Blocks(blocks.iter().map(user_block_to_api).collect())
            },
        },
        ConversationMessage::Assistant(blocks) => AnthropicApiMessage {
            role: "assistant".to_string(),
            content: AnthropicContent::Blocks(blocks.iter().map(assistant_block_to_api).collect()),
        },
    }).collect();

    let tools = request.tools.iter().map(|spec| AnthropicToolDef {
        name: spec.name.clone(),
        description: spec.description.clone(),
        input_schema: spec.input_schema.clone(),
    }).collect();

    AnthropicMessagesRequest {
        model: self.model.clone(),
        system: request.system_prompt.clone().unwrap_or_else(default_system_prompt),
        messages,
        tools,
        max_tokens: request.options.max_output_tokens.unwrap_or(32000),
        temperature: None,
        stream: stream.then_some(true),
        thinking,
    }
}
```

**修改响应解析**：将 Anthropic API 的 `content` 数组解析为 `Vec<AssistantContentBlock>`，`stop_reason` 映射为 `StopReason`。

**修改流式解析**：当前 `AnthropicSseParser` 只收集 raw JSON 帧，需要在 `complete_stream()` 中处理以下事件序列：

```
content_block_start  { type: "text" }            → 开始文本块
content_block_delta  { type: "text_delta" }       → 发送 StreamDelta::Text，同时通过 delta_sender 通知 agent 层流式展示给用户
content_block_start  { type: "tool_use", id, name } → 开始工具调用块，发送 StreamDelta::ToolUseStart
content_block_delta  { type: "input_json_delta" } → 累积 tool args，发送 StreamDelta::ToolArgsDelta（不展示给用户）
content_block_stop                                → 发送 StreamDelta::ToolUseEnd
message_delta        { stop_reason }              → 记录 StopReason
```

**关键约束**：`StreamDelta::ToolUseStart/ToolArgsDelta/ToolUseEnd` 只发给 `delta_sender` 用于 agent 层收集工具调用参数，**不得**通过 `ResponseTextDelta` 事件流给用户展示。Agent 层（Phase 3）在收到 `ToolUseEnd` 后才开始执行工具，执行期间通过 `ToolCallStarted`/`ToolCallArgsDelta`/`ToolCallCompleted` 事件通知 TUI。

**新增 StreamDelta 变体**（在 `types.rs`）：

```rust
pub enum StreamDelta {
    Text(String),
    Reasoning(String),  // 暂保留，对应 thinking block
    ToolUseStart { id: String, name: String },
    ToolArgsDelta { id: String, delta: String },
    ToolUseEnd { id: String },
}
```

**`complete_stream()` 内部 tool_use 累积逻辑**：

```rust
// complete_stream() 中维护一个临时 map 来累积流式 tool_use
let mut pending_tool_uses: HashMap<String, PendingToolUse> = HashMap::new();

// 处理 input_json_delta 时
pending_tool_uses
    .entry(id.clone())
    .or_default()
    .args_raw.push_str(&partial_json);
delta_sender.send(StreamDelta::ToolArgsDelta { id, delta: partial_json });

// content_block_stop 时，将累积的 JSON 解析为 serde_json::Value
// 最终 complete_stream() 返回的 ModelResponse.content 包含完整的 ToolUse block
```

**更新 `CapabilityFlags`：**

```rust
// from_provider_config() 和 from_anthropic_config() 中
capabilities: CapabilityFlags {
    stream: true,
    reasoning: true,
    tool_call: true,   // ← 改为 true
    json_mode: false,
},
```

#### 2.2 `openjax-core/src/model/chat_completions.rs`

类似改动，适配 OpenAI format：

```rust
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatApiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ChatToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    // ...existing fields
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
struct ChatToolDef {
    #[serde(rename = "type")]
    tool_type: String,  // "function"
    function: ChatFunctionDef,
}

#[derive(Debug, Serialize)]
struct ChatFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}
```

OpenAI 响应解析：`choices[0].message.tool_calls` → `Vec<AssistantContentBlock::ToolUse>`。

流式：`choices[0].delta.tool_calls` → 累积 `function.arguments` 字符串，按相同约束分发 `StreamDelta::ToolUseStart/ToolArgsDelta/ToolUseEnd`，`choices[0].delta.content` → `StreamDelta::Text`。文本 delta 展示给用户，工具 args delta 不展示。

#### 2.3 涉及文件

| 文件 | 改动类型 |
|------|---------|
| `openjax-core/src/model/anthropic_messages.rs` | 主要重构 |
| `openjax-core/src/model/chat_completions.rs` | 主要重构 |
| `openjax-core/src/model/types.rs` | 新增 StreamDelta 变体 |
| `openjax-core/src/streaming/parser/anthropic.rs` | 扩展 tool_use 事件处理 |
| `openjax-core/src/streaming/parser/openai.rs` | 扩展 tool_calls 事件处理 |

#### 2.4 测试

```
// openjax-core/tests/tools_sandbox/m_native_tool_call_anthropic.rs
- 单工具调用：模型返回 tool_use block → 解析正确
- 多工具并行：模型返回多个 tool_use block → 均解析
- 纯文本响应：stop_reason=end_turn，无 tool_use
- 流式 tool_use：args delta 累积后等于完整 input JSON
- 流式双通道验证：text delta 走 ResponseTextDelta 事件，tool args delta 不走该通道
- tool_use + text 混合响应：同一 response 同时含文本和工具调用时均正确分发

// openjax-core/tests/tools_sandbox/m_native_tool_call_openai.rs
- 同上，针对 chat_completions 格式
- 流式 tool_calls.function.arguments 碎片化累积后等于完整 JSON
```

**注**：这两组集成测试需要 echo/mock model，可复用现有 `echo.rs` 模式，无需真实 API。

---

### Phase 3：Agent 循环层 — 从 Planner Prompt 到多轮对话

**目标**：用 Native Tool Calling 对话循环替换 Planner Prompt 循环，移除 dispatcher 自定义 JSON 解析逻辑。

#### 3.1 `openjax-core/src/agent/prompt.rs` → `system_prompt.rs`

**重命名文件**（或就地重构）。原来的 `build_planner_input` 承担了"把所有东西拼成一条消息"的责任，现在拆分为：

```rust
/// 构建系统提示词（persona + 工具选择策略 + 技能上下文）
pub(crate) fn build_system_prompt(
    skills_context: &str,
) -> String {
    format!(
        "{PERSONA}\n\n{BEHAVIOR}\n\n{SAFETY}\n\n\
         <tool_policy>\n\
         - Prefer read_file before edit_file_range or apply_patch (Update File) unless creating a brand-new file.\n\
         - Prefer edit_file_range for single-file edits when exact line range is known.\n\
         - For multi-file edits or file operations (add/delete/move/rename), use apply_patch.\n\
         - Prefer process_snapshot/system_load/disk_usage for process/host metrics over shell ps/top/df.\n\
         - Do NOT repeat the same tool call with the same arguments.\n\
         </tool_policy>\n\n\
         <available_skills>\n{skills_context}\n</available_skills>"
    )
}

/// 构建历史压缩摘要消息（跨 turn 的上下文注入）
pub(crate) fn build_history_summary_message(summary: &str) -> ConversationMessage {
    ConversationMessage::User(vec![UserContentBlock::Text {
        text: format!("<conversation_history>\n{summary}\n</conversation_history>"),
    }])
}
```

**删除**：`build_planner_input`、`build_json_repair_prompt`、`truncate_for_prompt`（移至 common）。

#### 3.2 `openjax-core/src/agent/planner.rs` 重构

核心循环从：

```
while executed_count < max:
  prompt = build_planner_input(...)
  response = model.complete(prompt)
  json = parse_model_decision(response.text)  // dispatcher
  if json.action == "tool": execute_tool(...)
  if json.action == "final": return response
```

改为：

```rust
pub(crate) async fn execute_natural_language_turn(
    &mut self,
    turn_id: u64,
    user_input: &str,
    events: &mut Vec<Event>,
) {
    // 1. 构建初始 messages（历史摘要 + 当前用户输入）
    let mut messages: Vec<ConversationMessage> = self.build_turn_messages(user_input);

    // 2. 获取当前所有工具规范
    let tool_specs = self.tool_router.tool_specs();

    // 3. 构建系统提示词
    let skills_context = self.build_skills_context(user_input);
    let system_prompt = build_system_prompt(&skills_context);

    let mut executed_count = 0usize;
    let mut tool_traces: Vec<String> = Vec::new();

    loop {
        if executed_count >= self.max_tool_calls_per_turn {
            // emit turn_limit_reached
            return;
        }

        // 4. 向模型发请求
        let request = ModelRequest {
            stage: ModelStage::Planner,
            system_prompt: Some(system_prompt.clone()),
            messages: messages.clone(),
            tools: tool_specs.clone(),
            options: ModelRequestOptions::default(),
        };

        let response = self.request_model(turn_id, &request, events).await?;

        // 5. 将助手响应追加到 messages
        messages.push(ConversationMessage::Assistant(response.content.clone()));

        // 6. 判断分支
        if !response.has_tool_use() || response.stop_is_end_turn() {
            // 最终回答
            let text = response.text();
            self.emit_final_response(turn_id, &text, events);
            self.commit_turn(user_input.to_string(), tool_traces, text);
            return;
        }

        // 7. 执行所有 tool_use blocks
        let mut tool_result_blocks: Vec<UserContentBlock> = Vec::new();
        for tool_use in response.tool_uses() {
            let AssistantContentBlock::ToolUse { id, name, input } = tool_use else { continue };

            self.push_event(events, Event::ToolCallStarted {
                turn_id,
                tool_call_id: id.clone(),
                tool_name: name.clone(),
                target: extract_target(name, input),
                display_name: self.tool_router.display_name(name),
            });

            let result = self.tool_router.execute_tool(ToolExecutionRequest {
                turn_id,
                tool_call_id: id.clone(),
                tool_name: name.clone(),
                arguments: serde_json::to_string(input).unwrap_or_default(),
                // ...
            }).await;

            let (model_content, ok, metadata) = result.split_model_and_metadata();

            self.push_event(events, Event::ToolCallCompleted {
                turn_id,
                tool_call_id: id.clone(),
                tool_name: name.clone(),
                ok,
                output: model_content.clone(),
                // metadata fields added in Phase 5
                display_name: self.tool_router.display_name(name),
            });

            tool_traces.push(format!("{name}({}) -> {}", summarize_args(input), summarize_output(&model_content)));
            executed_count += 1;

            tool_result_blocks.push(UserContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: model_content,
                is_error: !ok,
            });
        }

        // 8. 将工具结果追加到 messages
        messages.push(ConversationMessage::User(tool_result_blocks));
    }
}
```

**关键设计决策：**

- `tool_router.execute_tool()` 返回一个 `ToolExecResult { model_content: String, metadata: ShellMetadata?, ok: bool }`，`model_content` 只包含模型需要的内容（exit_code + stdout + stderr），`metadata` 包含 result_class/backend 等 TUI 用字段（Phase 5 使用）。
- 历史跨 turn 压缩：`build_turn_messages()` 从 `self.history` 构建，历史中旧 turn 的工具调用可摘要化为文本，避免 context 过长。具体策略：最近 N 个 turn 保留完整 tool_use/tool_result 对，更早的 turn 压缩为文字摘要。

#### 3.3 Dispatcher 简化

`dispatcher/mod.rs` 的 `route_model_output()` 和相关 `parse_model_decision` 逻辑在 Phase 3 完成后不再需要。

**处理方式**：
- 保留 `dispatcher/` 模块，但将 `route_model_output()` 标记为 `#[deprecated]`
- 将 `DispatchOutcome` 简化为内部实现（Phase 3 完成后可彻底删除）
- `probe.rs`、`decision.rs` 中的自定义 JSON 解析逻辑（`parse_model_decision`、`DecisionJsonStreamParser`）在 Phase 3 完成后删除

**删除目标文件**（Phase 3 后）：
- `openjax-core/src/dispatcher/probe.rs`
- `openjax-core/src/agent/decision.rs`
- `openjax-core/src/agent/prompt.rs` 中 `build_planner_input` / `build_json_repair_prompt`
- `openjax-core/src/agent/planner_tool_action.rs`（合并进新 planner.rs）

#### 3.4 `openjax-core/src/tools/router_impl.rs` 新增接口

```rust
impl ToolRouter {
    /// 返回所有工具的 ToolSpec（传给 ModelRequest.tools）
    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.specs.clone()
    }

    /// 返回工具显示名称（用于事件）
    pub fn display_name(&self, tool_name: &str) -> Option<String> {
        self.specs.iter()
            .find(|s| s.name == tool_name)
            .map(|s| s.display_name.clone())
    }
}
```

#### 3.5 涉及文件

| 文件 | 改动类型 |
|------|---------|
| `openjax-core/src/agent/prompt.rs` | 重构为 system_prompt.rs |
| `openjax-core/src/agent/planner.rs` | 核心重写 |
| `openjax-core/src/agent/planner_tool_action.rs` | 合并进 planner.rs，删除 |
| `openjax-core/src/agent/decision.rs` | Phase 3 后删除 |
| `openjax-core/src/dispatcher/mod.rs` | 简化，移除 JSON 解析路径 |
| `openjax-core/src/dispatcher/probe.rs` | Phase 3 后删除 |
| `openjax-core/src/tools/router_impl.rs` | 新增 `tool_specs()`, `display_name()` |

#### 3.6 测试

```
// 现有套件全量回归（Phase 3 完成后必须全部通过）
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core --test approval_suite"
zsh -lc "cargo test -p openjax-core --test streaming_suite"
zsh -lc "cargo test -p openjax-core --test skills_suite"
zsh -lc "cargo test -p openjax-core --test core_history_suite"

// 新增集成测试
// openjax-core/tests/native_tool_calling_suite.rs
- m1_single_tool_call_and_respond.rs       // 单工具调用 → 收到结果 → 模型回答
- m2_multi_tool_parallel.rs                // 一次返回多个 tool_use → 并行执行
- m3_tool_call_chain.rs                    // 工具A返回结果 → 模型调用工具B → 最终回答
- m4_max_tool_calls_limit.rs               // 超过 max_tool_calls_per_turn 后终止
- m5_tool_error_recovery.rs                // tool is_error=true → 模型收到错误继续
- m6_history_across_turns.rs               // 第二个 turn 能访问第一个 turn 的结果
```

---

### Phase 4：工具能力补充

**目标**：新增 `write_file`、`glob_files` 工具，归位 `apply_patch` 描述。这些改动与 Phase 1-3 无依赖，可与 Phase 2-3 并行准备。

#### 4.1 新增 `write_file` 工具

**文件**：`openjax-core/src/tools/handlers/write_file.rs`（新建）

```rust
#[derive(Deserialize)]
struct WriteFileArgs {
    file_path: String,
    content: String,
}

pub struct WriteFileHandler;

impl ToolHandler for WriteFileHandler {
    // 路径验证：不允许逃逸工作区（复用 apply_patch/planner.rs 的验证逻辑）
    // 父目录不存在时自动 create_dir_all
    // 直接 write（覆盖）
    // 返回："written <path> (<n> bytes)"
}
```

**工具 Spec**（在 `spec.rs` 新增 `create_write_file_spec()`）：

```json
{
  "name": "write_file",
  "description": "Create or overwrite a file with the given content. Parent directories are created automatically. Use this for creating new files or completely replacing a file's content.",
  "input_schema": {
    "type": "object",
    "properties": {
      "file_path": {"type": "string", "description": "File path relative to workspace root"},
      "content":   {"type": "string", "description": "Full file content to write"}
    },
    "required": ["file_path", "content"]
  }
}
```

**注册**：在 `tool_builder.rs` 的 `build_all_specs()` 和 handler 注册中加入。

**测试**（`openjax-core/tests/tools_sandbox/m_write_file.rs`）：
- 新建文件
- 覆盖已有文件
- 路径逃逸被拒绝（`../../../etc/passwd`）
- 父目录自动创建

#### 4.2 新增 `glob_files` 工具

**文件**：`openjax-core/src/tools/handlers/glob_files.rs`（新建）

```rust
#[derive(Deserialize)]
struct GlobFilesArgs {
    pattern: String,
    #[serde(default)]
    base_path: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

pub struct GlobFilesHandler;
// 使用 glob crate（检查 openjax-core/Cargo.toml，若无则添加）
// 返回匹配的文件路径列表，每行一条，按修改时间排序（最新在前）
```

**工具 Spec**（`spec.rs` 新增 `create_glob_files_spec()`）：

```json
{
  "name": "glob_files",
  "description": "Find files by path pattern (glob syntax). Searches file paths, not content. Returns matching paths sorted by modification time (newest first).",
  "input_schema": {
    "type": "object",
    "properties": {
      "pattern":   {"type": "string", "description": "Glob pattern, e.g. src/**/*.rs or **/*.toml"},
      "base_path": {"type": "string", "description": "Base directory (default: workspace root)"},
      "limit":     {"type": "number", "default": 200, "minimum": 1, "maximum": 2000}
    },
    "required": ["pattern"]
  }
}
```

**Cargo.toml 依赖**（`openjax-core/Cargo.toml`）：
```toml
glob = "0.3"
```

**测试**（`openjax-core/tests/tools_sandbox/m_glob_files.rs`）：
- `**/*.rs` 匹配所有 Rust 文件
- 路径逃逸被拒绝
- limit 生效
- 不存在路径返回空

#### 4.3 `apply_patch` 描述归位

将 `agent/prompt.rs` 中的 apply_patch 格式细节（16 行）移至 `spec.rs` 的 `create_apply_patch_spec()` description 末尾。

`system_prompt.rs` 中仅保留 3 行调度策略（已在 3.1 中展示）。

#### 4.4 涉及文件

| 文件 | 改动类型 |
|------|---------|
| `openjax-core/src/tools/handlers/write_file.rs` | 新建 |
| `openjax-core/src/tools/handlers/glob_files.rs` | 新建 |
| `openjax-core/src/tools/handlers/mod.rs` | pub mod 新增 |
| `openjax-core/src/tools/spec.rs` | 新增两个 spec 函数，apply_patch 描述扩充 |
| `openjax-core/src/tools/tool_builder.rs` | 注册两个新 handler |
| `openjax-core/Cargo.toml` | 添加 glob 依赖 |
| `openjax-core/tests/tools_sandbox/m_write_file.rs` | 新建测试 |
| `openjax-core/tests/tools_sandbox/m_glob_files.rs` | 新建测试 |

---

### Phase 5：Shell 输出分离（P3 完整方案）

**目标**：彻底解决 model_output 与 TUI 数据共用的问题，对齐 Claude Code 的双通道架构。

#### 5.1 扩展 `ToolCallCompleted` 事件（`openjax-protocol/src/lib.rs`）

```rust
Event::ToolCallCompleted {
    turn_id: u64,
    tool_call_id: String,
    tool_name: String,
    ok: bool,
    output: String,  // 保留，但语义变为"TUI 展示用的完整 raw output"
    display_name: Option<String>,
    // 新增 shell metadata（仅 shell 工具填充，其他工具 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shell_metadata: Option<ShellExecutionMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShellExecutionMetadata {
    pub result_class: String,        // "success" | "partial_success" | "failure"
    pub backend: String,             // "macos_seatbelt" | "linux_native" | "none_escalated"
    pub exit_code: i32,
    pub policy_decision: String,     // "Allow" | "AskApproval"
    pub runtime_allowed: bool,
    pub degrade_reason: Option<String>,
    pub runtime_deny_reason: Option<String>,
}
```

#### 5.2 修改 `ToolExecResult`（`openjax-core/src/tools/router_impl.rs`）

```rust
pub struct ToolExecOutcome {
    /// 发送给模型的内容（仅 exit_code + stdout + stderr）
    pub model_content: String,
    /// TUI 展示用的完整 raw output（保留原始详细格式，向后兼容）
    pub display_output: String,
    /// Shell 执行 metadata（仅 shell 工具填充）
    pub shell_metadata: Option<ShellExecutionMetadata>,
    pub success: bool,
}
```

#### 5.3 修改 `sandbox/mod.rs` 的 `execute_shell()`

```rust
// model_content：只给模型看
let model_content = if output.exit_code == 0 {
    format!("exit_code={}\nstdout:\n{}", output.exit_code, output.stdout)
} else {
    format!(
        "exit_code={}\nstdout:\n{}\nstderr:\n{}",
        output.exit_code, output.stdout, output.stderr
    )
};

// display_output：TUI 展示，保留完整 metadata（向后兼容）
let display_output = format!(
    "result_class={}\ncommand={}\nexit_code={}\nbackend={}\n\
     degrade_reason={}\npolicy_decision={:?}\nruntime_allowed={}\n\
     runtime_deny_reason={}\nstdout:\n{}\nstderr:\n{}",
    // ...原有字段
);

let shell_metadata = ShellExecutionMetadata {
    result_class: result_class.as_str().to_string(),
    backend: output.backend_used.as_str().to_string(),
    exit_code: output.exit_code,
    policy_decision: format!("{:?}", output.policy_trace.decision),
    runtime_allowed,
    degrade_reason: output.degrade_reason.clone(),
    runtime_deny_reason: runtime_deny_reason.clone(),
};

Ok(ToolExecOutcome {
    model_content,
    display_output,
    shell_metadata: Some(shell_metadata),
    success: is_shell_success,
})
```

同样更新 `shell.rs` handler 中 skill-trigger guard 路径的输出。

#### 5.4 修改 planner.rs 中 ToolCallCompleted 的发送

Phase 3 实现的 `execute_natural_language_turn` 在发送 `ToolCallCompleted` 时：
- `output` 字段填 `result.display_output`（TUI 向后兼容）
- `shell_metadata` 字段填 `result.shell_metadata`

#### 5.5 TUI 侧改造（`ui/tui/`）

**目标**：TUI 优先从 `shell_metadata` 事件字段读取结构化数据，不再解析 `output` 字符串。

| 文件 | 改动内容 |
|------|---------|
| `ui/tui/src/app/cells.rs:86` | `is_partial = event.shell_metadata.as_ref().map(|m| m.result_class == "partial_success").unwrap_or_else(\|\| output.contains("result_class=partial_success"))` |
| `ui/tui/src/app/tool_output.rs` | `extract_backend_summary` 和 `degraded_risk_summary` 优先用 `shell_metadata`，fallback 保留字符串解析 |

**向后兼容策略**：Phase 5 初期，TUI 同时支持从 `shell_metadata` 字段读和从 `output` 字符串解析（fallback），保证旧事件格式不破坏。

#### 5.6 涉及文件

| 文件 | 改动类型 |
|------|---------|
| `openjax-protocol/src/lib.rs` | 新增 `ShellExecutionMetadata`，扩展 `ToolCallCompleted` |
| `openjax-core/src/tools/router_impl.rs` | `ToolExecOutcome` 新增 `model_content` / `display_output` / `shell_metadata` |
| `openjax-core/src/sandbox/mod.rs` | 拆分 model_content 和 display_output |
| `openjax-core/src/tools/handlers/shell.rs` | skill-trigger 路径同步拆分 |
| `openjax-core/src/agent/planner.rs` | 传递 shell_metadata 到事件 |
| `ui/tui/src/app/cells.rs` | 优先读 shell_metadata |
| `ui/tui/src/app/tool_output.rs` | 优先读 shell_metadata |
| `ui/tui/tests/m12_tool_partial_status.rs` | 更新测试数据 |
| `ui/tui/tests/m17_degraded_mutating_warning.rs` | 更新测试数据 |

---

## 五、执行顺序与依赖关系

```
Phase 1 (types.rs 数据类型)
  └─→ Phase 2 (adapter 原生工具调用支持)
        └─→ Phase 3 (agent 循环重写)
              └─→ Phase 5 (shell 输出分离)

Phase 4 (write_file / glob_files / 描述归位)  ← 可与 Phase 2/3 并行
```

**各 Phase 完成后的可测试状态：**

| Phase | 可验证的行为 |
|-------|------------|
| Phase 1 | `cargo build` 编译通过，类型单元测试通过 |
| Phase 2 | Mock model 的 native tool calling 请求/响应序列化正确 |
| Phase 3 | 全量集成测试通过，工具调用走原生路径，`build_planner_input` 不再被调用 |
| Phase 4 | `write_file`/`glob_files` 集成测试通过 |
| Phase 5 | `ToolCallCompleted` 携带 `shell_metadata`，TUI 显示沙箱信息不再依赖字符串解析 |

---

## 六、全量验证方案

### Phase 3 完成后必须全部通过

```bash
# 核心功能
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core --test approval_suite"
zsh -lc "cargo test -p openjax-core --test approval_events_suite"
zsh -lc "cargo test -p openjax-core --test streaming_suite"
zsh -lc "cargo test -p openjax-core --test skills_suite"
zsh -lc "cargo test -p openjax-core --test core_history_suite"
zsh -lc "cargo test -p openjax-core --test native_tool_calling_suite"

# TUI
zsh -lc "cargo test -p tui_next --test m1_no_duplicate_history"
zsh -lc "cargo test -p tui_next --test m10_approval_panel_navigation"

# Phase 5 完成后
zsh -lc "cargo test -p tui_next"

# 全量
zsh -lc "cargo test --workspace"
zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"
```

### 冒烟验证（端对端）

```bash
# make targets（如有 real API key）
zsh -lc "make core-smoke"
zsh -lc "make core-full"
```

---

## 七、风险与注意事项

### 1. 历史上下文长度
Native tool calling 在一个 turn 内会累积完整的 messages（每次工具调用都追加 assistant+user 两条消息）。当工具调用次数多时，context 可能超出模型限制。

**应对**：现有 `context_compressor.rs` 的 `check_and_auto_compact` 机制在 Phase 3 中需要适配新的 `messages` 格式。Phase 3 初期可先设置保守的 `max_tool_calls_per_turn`，待压缩机制适配后再放开。

### 2. 审批（Approval）流程
当前审批系统在工具执行中途向用户弹出审批请求，会中断工具执行循环。Native tool calling 的循环结构与原来不同，需要确认审批逻辑在新的 `execute_natural_language_turn` 中正确触发。

**应对**：Phase 3 实现时，`approval_suite` 测试必须保持全绿。审批流程保持在 `tool_router.execute_tool()` 内部，不暴露到 agent 循环层。

### 4. 文件大小警告
Phase 3 的 `planner.rs` 重写后代码量可能增大，需注意：
- 超过 500 行时拆出 `planner_conv.rs`（会话构建）和 `planner_exec.rs`（工具执行循环）
- 工具结果格式化逻辑提取为 `tool_result_formatter.rs`

---

## 八、与原计划（tool-optimization-plan.md）的对应关系

| 原计划 | 本计划处理方式 |
|--------|--------------|
| P1 参数类型约束 | 废弃：native tool calling 下模型按 JSON Schema 输出，不会传字符串数字 |
| P2 工具列表动态化 | Phase 3 天然解决：`tool_specs()` 传给 ModelRequest.tools |
| P3 Shell 输出精简 | Phase 5 完整方案（独立计划变为本文 Phase 5） |
| P4 write_file | Phase 4 保留 |
| P5 apply_patch 描述归位 | Phase 4 保留 |
| P6 glob_files | Phase 4 保留 |
| P7 先读后写约束 | Phase 3 system_prompt.rs 中直接写入 tool_policy |
