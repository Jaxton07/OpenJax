# Thinking / Reasoning Variants 计划

## 背景与目标

Extended thinking（模型内部推理）已在 OpenCode 中作为核心能力支持，OpenJax 已有部分实现，但存在明显缺口。本计划目标是：

1. 补齐现有实现中的缺口（chat_completions 非流式不收集推理内容、缺少 reasoning_effort）
2. 为用户提供请求级别的 thinking 开关（而不仅限于环境变量 / 静态模型配置）
3. 对齐 OpenCode 的 thinking variants 设计（budget 预设、per-model capability）

参考实现：`/Users/ericw/work/code/ai/opencode/packages/opencode/`

---

## 当前状态分析

### 已实现

**`openjax-core/src/model/anthropic_messages.rs`**
- `AnthropicThinking` 请求体结构体（`type: "enabled"`, `budget_tokens`）
- 请求时从 `ModelRequestOptions.thinking_budget_tokens` 或 env var `OPENJAX_THINKING_BUDGET_TOKENS` 注入
- 响应解析：`extract_thinking_from_body()`（非流式）、`extract_delta_thinking_from_body()`（SSE）
- 流式推送：`StreamDelta::Reasoning(thinking_delta)`

**`openjax-core/src/model/chat_completions.rs`**
- 流式：`extract_delta_reasoning_from_body()` 提取 `reasoning_content` 字段，推送 `StreamDelta::Reasoning`

**`openjax-core/src/model/types.rs`**
- `ModelRequestOptions.thinking_budget_tokens: Option<u32>`
- `CapabilityFlags.reasoning: bool`
- `ModelResponse.reasoning: Option<String>`

**`openjax-core/src/model/factory.rs` / `registry.rs`**
- `default_capabilities("anthropic_messages")` → `reasoning: true`
- 模型注册时支持 `thinking_budget_tokens` 和 `supports_reasoning`

### 缺口

| 缺口 | 位置 | 说明 |
|---|---|---|
| 非流式不收集推理内容 | `chat_completions.rs:complete()` | `ModelResponse.reasoning` 始终为 `None`，`reasoning_content` 字段未从非流式响应体提取 |
| 缺少 `reasoning_effort` | `chat_completions.rs` | OpenAI o 系列（o1/o3/o4-mini）使用 `reasoning_effort: "low"/"medium"/"high"` 而非 `budget_tokens`，当前无此字段 |
| thinking 只能静态配置 | factory / registry | 用户无法在单次请求时临时开启 / 调整 budget；仅支持 env var 或模型注册时写死 |
| 无 budget 预设 | 全局 | OpenCode 有 low(1000) / medium(8000) / high(16000) 三档预设，OpenJax 无 |
| WebUI / Gateway 无暴露 | gateway / web | thinking 相关参数无法通过 API 或前端配置 |

---

## 目标状态

### thinking 触发优先级（由高到低）

```
请求级 (ModelRequestOptions.thinking_budget_tokens)
    > 模型注册配置 (RegisteredModel.thinking_budget_tokens)
    > env var (OPENJAX_THINKING_BUDGET_TOKENS)
    > 不启用
```

当前 `anthropic_messages.rs` 已实现此优先级逻辑，**不需要改动**。

### 两种 provider 协议的 thinking 参数形式

| 协议 | 参数 | 字段 |
|---|---|---|
| `anthropic_messages` | thinking block | `thinking: {type: "enabled", budget_tokens: N}` |
| `chat_completions`（OpenAI o 系列）| reasoning effort | `reasoning_effort: "low"/"medium"/"high"` |

两者互斥，分别处理。

---

## 任务列表

### Task 1：补齐 `chat_completions.rs` 非流式推理内容收集

**文件**：`openjax-core/src/model/chat_completions.rs`

- 非流式 `complete()` 当前直接返回 `reasoning: None`
- 增加从响应体提取 `reasoning_content` 的逻辑，类似流式的 `extract_delta_reasoning_from_body()`
- OpenAI-compatible 非流式响应格式：

```json
{
  "choices": [{
    "message": {
      "content": "...",
      "reasoning_content": "..."
    }
  }]
}
```

- 提取后写入 `ModelResponse.reasoning`

---

### Task 2：`chat_completions.rs` 增加 `reasoning_effort` 支持

**文件**：
- `openjax-core/src/model/types.rs` — `ModelRequestOptions` 增加 `pub reasoning_effort: Option<String>`
- `openjax-core/src/model/chat_completions.rs` — 请求体 `ChatCompletionsRequest` 增加 `reasoning_effort` 字段；构建请求时从 `ModelRequestOptions` 读取注入

**注意**：
- `reasoning_effort` 仅适用于 OpenAI o 系列，非 o 系列 provider 忽略此字段（`skip_serializing_if = "Option::is_none"`）
- 与 Anthropic 的 `budget_tokens` 语义不同，不需要联动

---

### Task 3：thinking budget 预设枚举

**文件**：`openjax-core/src/model/types.rs`（或新建 `openjax-core/src/model/thinking.rs`）

定义三档预设，对齐 OpenCode `ProviderTransform.variants()`（L335-714）：

```rust
pub const THINKING_BUDGET_LOW: u32 = 1_000;
pub const THINKING_BUDGET_MEDIUM: u32 = 8_000;
pub const THINKING_BUDGET_HIGH: u32 = 16_000;
```

这些常量供模型注册配置和测试使用，不影响运行时逻辑。

---

### Task 4：Gateway 请求体暴露 `thinking_budget_tokens` / `reasoning_effort`

**文件**：`openjax-gateway/src/handlers/`（run / turn 相关 handler）

- 在 turn 请求体中增加可选字段 `thinking_budget_tokens: Option<u32>` 和 `reasoning_effort: Option<String>`
- 透传到 `ModelRequest.options`
- 更新相关集成测试

**不需要**改动 store 层（thinking 配置属于请求级参数，不需要持久化）。

---

### Task 5：WebUI 支持 thinking 控制（可选，后置）

**文件**：`ui/web/src/`

- 在对话输入区增加"thinking"开关或 budget 档位选择
- 可选：模型配置页面展示该模型是否支持 thinking（`capabilities.reasoning`）

此 task 依赖 Task 4 完成，优先级低于 Task 1-4。

---

## 任务依赖关系

```
Task 1 (chat_completions 非流式)  — 独立，风险低，先做
Task 2 (reasoning_effort)        — 独立
Task 3 (budget 预设常量)          — 独立，纯常量定义
Task 4 (Gateway 暴露)            — 依赖 Task 2（reasoning_effort 字段已存在）
Task 5 (WebUI)                   — 依赖 Task 4
```

推荐执行顺序：**Task 3 → Task 1 → Task 2 → Task 4 → Task 5**

---

## OpenCode 参考索引

| 内容 | OpenCode 文件路径 |
|---|---|
| Per-model thinking budget variants (low/medium/high) | `packages/opencode/src/provider/transform.ts` → `variants()` (L335-714) |
| providerOptions 注入 thinking / thinkingConfig | `packages/opencode/src/provider/transform.ts` → `options()` (L717-835) |
| maxOutputTokens 与 budget_tokens 联动约束 | `packages/opencode/src/provider/transform.ts` → `maxOutputTokens()` (L913-915) |
| Anthropic beta header（interleaved-thinking）| `packages/opencode/src/provider/provider.ts` → `CUSTOM_LOADERS.anthropic` (L153-162) |
| stream() 调用中 providerOptions 传递 | `packages/opencode/src/session/llm.ts` → `stream()` (L48-294) |
| reasoning_effort（OpenAI o 系列）| `packages/opencode/src/provider/transform.ts` → `variants()` 中 `openai` 相关段落 |

---

## 涉及文件汇总

```
openjax-core/
  src/model/types.rs              — ModelRequestOptions 增加 reasoning_effort，budget 预设常量
  src/model/chat_completions.rs   — 非流式收集 reasoning_content，reasoning_effort 字段注入
  src/model/anthropic_messages.rs — 当前已实现，确认逻辑正确后无需改动

openjax-gateway/
  src/handlers/                   — turn 请求体暴露 thinking_budget_tokens / reasoning_effort

ui/web/
  src/                            — thinking 控制 UI（后置，Task 5）

tests/
  相关集成测试同步更新
```

---

## 约束与注意事项

- `budget_tokens` 必须小于 `max_tokens`，否则 Anthropic API 返回 400。当前代码未做校验，Task 1/2 完成后需确认 `anthropic_messages.rs` 的 `max_tokens` 设置足够大（与 provider-refactor-plan Task 4c 联动）。
- `chat_completions` 协议的 `reasoning_effort` 仅对支持 o 系列的 provider 生效，其他 provider 忽略此字段（依赖 `skip_serializing_if = "Option::is_none"`，无需额外判断）。
- Anthropic 的 `anthropic-beta: interleaved-thinking-2025-05-14` header 已在 provider-refactor-plan Task 4a 中包含，是 thinking 正常工作的前提条件。
