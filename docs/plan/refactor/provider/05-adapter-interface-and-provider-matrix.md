# 05 适配器接口与 Provider/Protocol 矩阵

## 统一接口

### `ModelClient`

1. `complete(request: &ModelRequest) -> ModelResponse`
2. `complete_stream(request: &ModelRequest, delta_sender) -> ModelResponse`

### `ProviderAdapter`

1. `complete(...)`
2. `complete_stream(...)`
3. 元数据：`model_id/provider/protocol/backend_name/capabilities`

## 统一数据结构

1. `ModelRequest { stage, user_input, system_prompt, options }`
2. `ModelResponse { text, reasoning, usage, finish_reason, raw }`
3. `CapabilityFlags { stream, reasoning, tool_call, json_mode }`

## 当前矩阵

| provider | protocol | stream | reasoning | 说明 |
|---|---|---:|---:|---|
| `openai` | `chat_completions` | yes | no | OpenAI 兼容协议 |
| `minimax` | `chat_completions` | yes | no | MiniMax OpenAI 兼容 |
| `glm` | `anthropic_messages` | yes | yes | GLM Anthropic 兼容 |
| `anthropic` | `anthropic_messages` | yes | yes | Anthropic 原生消息协议 |

## 新 provider 接入清单

1. 在 `config` 中定义 provider entry。
2. 新增 adapter 实现 `ProviderAdapter`。
3. 在 `factory` 注册 `protocol -> adapter` 构造。
4. 为 adapter 补单测与协议样例。
