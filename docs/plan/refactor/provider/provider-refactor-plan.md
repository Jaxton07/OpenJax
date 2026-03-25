# Provider 层重构计划

## 背景与目标

当前 OpenJax 的 provider 层存在以下问题：

1. **协议判断依赖 URL 启发式推断**，不可靠，无法支持任意自定义 provider
2. **内置 catalog 协议配置有误**（Kimi、MiniMax 应走 `anthropic_messages` 协议而非 `chat_completions`）
3. **请求格式未对齐业界标准**：temperature 全部硬编码 0.2、max_tokens 多数场景未设置、缺少 Anthropic beta header 等
4. **`ProviderRecord` 不存储 protocol**，用户通过 WebUI 添加的 provider 无法正确路由

重构目标：**完全对齐 OpenCode 的 provider 接入标准**，使 OpenJax 能稳定接入当前主流 LLM provider（Anthropic、Kimi、MiniMax、GLM），并为用户自定义 provider 提供明确的协议选择机制。

参考实现：`/Users/ericw/work/code/ai/opencode/packages/opencode/`

---

## 当前状态分析

### 协议路由（有问题）

- `openjax-core/src/provider_store.rs` 中的 `provider_protocol()` 函数通过 URL 字符串猜测协议，脆弱且不准确
- `openjax-core/src/builtin_catalog.rs` 中 Kimi 使用 `chat_completions`，但官方（及 models.dev）明确为 Anthropic 格式
- `openjax-store/src/types.rs` 中 `ProviderRecord` 无 `protocol` 字段

### 请求格式（未对齐）

- `openjax-core/src/model/chat_completions.rs`：temperature 硬编码 `0.2`，仅 kimi profile 设置 max_tokens
- `openjax-core/src/model/anthropic_messages.rs`：缺少 `anthropic-beta` 请求头，无 extended thinking 支持
- 无 `User-Agent` 统一注入（kimi profile 里有硬编码 `KimiCLI/0.77`，应该清除）
- 无 prompt caching（cache_control / cacheControl 注入）

---

## 目标状态

### 内置 Provider Catalog

只内置以下 5 个 provider，其余由用户自行添加：

| catalog_key | display_name | base_url | protocol | env_key |
|---|---|---|---|---|
| `openai` | OpenAI | `https://api.openai.com/v1` | `chat_completions` | `OPENAI_API_KEY` |
| `anthropic` | Anthropic (Claude) | `https://api.anthropic.com` | `anthropic_messages` | `OPENJAX_ANTHROPIC_API_KEY` |
| `kimi_coding` | Kimi Coding | `https://api.kimi.com/coding/v1` | `anthropic_messages` | `OPENJAX_KIMI_API_KEY` |
| `minimax_coding` | MiniMax Coding | `https://api.minimax.io/anthropic/v1` | `anthropic_messages` | `OPENJAX_MINIMAX_API_KEY` |
| `glm_coding` | GLM Coding | `https://open.bigmodel.cn/api/anthropic` | `anthropic_messages` | `OPENJAX_GLM_API_KEY` |

**依据**：
- OpenCode models.dev：`kimi-for-coding → npm: @ai-sdk/anthropic`，`minimax → npm: @ai-sdk/anthropic`
- GLM BigModel 官方同时提供 Anthropic-compatible endpoint：`/api/anthropic`
- 不内置 Gemini、OpenRouter、Azure（用户可自行添加 custom provider）

### base_url 语义约定

| 协议 | base_url 存储格式 | 客户端追加路径 |
|---|---|---|
| `chat_completions` | 含 `/v1`，如 `https://api.openai.com/v1` | `/chat/completions` |
| `anthropic_messages` | 不含 `/messages`，如 `https://api.anthropic.com` 或 `https://api.kimi.com/coding/v1` | 若末尾有 `/v1` 则追加 `/messages`，否则追加 `/v1/messages` |

当前 `build_messages_endpoint()` 的逻辑与此一致，**不需要改动**。

---

## 任务列表

### Task 1：`ProviderRecord` 增加 `protocol` 字段

**文件**：
- `openjax-store/src/types.rs` — 在 `ProviderRecord` 增加 `pub protocol: String`
- `openjax-store/src/sqlite.rs` — DB schema 迁移：`ALTER TABLE providers ADD COLUMN protocol TEXT NOT NULL DEFAULT 'chat_completions'`；所有 `INSERT`/`SELECT`/`UPDATE` 语句同步更新
- `openjax-store/src/repository.rs` — `ProviderRepository` trait 的 `create_provider` / `update_provider` 签名增加 `protocol` 参数

**注意**：需要写 SQLite migration，确保已有数据兼容（DEFAULT 值为 `chat_completions`）。

---

### Task 2：废弃 `provider_protocol()` 启发式推断，改用显式字段

**文件**：`openjax-core/src/provider_store.rs`

- `build_config_from_providers` 中直接使用 `provider.protocol` 字段，不再调用 `provider_protocol()`
- 删除或标记废弃 `provider_protocol()` 函数
- `provider_vendor()` 保留（用于 provider 名字归一化），不受影响
- `infer_request_profile()` 中对 anthropic 的判断改为直接检查 `provider.protocol == "anthropic_messages"`

---

### Task 3：修复 `builtin_catalog.rs` 内置 catalog

**文件**：`openjax-core/src/builtin_catalog.rs`

按"目标状态"中的表格重写 `BUILTIN_CATALOG`：
- Kimi：`protocol` 改为 `anthropic_messages`，移除 `request_profile: Some("kimi_coding_v1")`（该 profile 是 chat_completions 专用的 User-Agent hack）
- MiniMax：`base_url` 改为 `https://api.minimax.io/anthropic/v1`，`protocol` 改为 `anthropic_messages`
- GLM：`base_url` 改为 `https://open.bigmodel.cn/api/anthropic`，`protocol` 改为 `anthropic_messages`
- Anthropic：`base_url` 保持 `https://api.anthropic.com`，确认 `protocol` 为 `anthropic_messages`
- OpenAI：保持不变

---

### Task 4：对齐请求格式——anthropic_messages 客户端

**文件**：`openjax-core/src/model/anthropic_messages.rs`

**4a. 增加 `anthropic-beta` 请求头**（参考 OpenCode `llm.ts:CUSTOM_LOADERS.anthropic`）：
```
anthropic-beta: interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14
```
固定附加，不受配置影响。

**4b. 统一 User-Agent**：
```
User-Agent: openjax/{VERSION}
```
所有 provider 统一附加，移除 kimi profile 中的 `KimiCLI/0.77`。

**4c. `max_tokens` 始终设置**：
使用 `min(context_window_size, 32000)` 作为默认值（与 OpenCode `ProviderTransform.OUTPUT_TOKEN_MAX = 32000` 对齐）。
目前 AnthropicMessagesRequest 的 `max_tokens` 字段已有，确认 profile 中正确传入。

**4d. temperature 按协议默认**：
Anthropic 协议不设置 temperature（`None`），由 provider 使用自身默认值。当前代码已是如此，确认无误。

---

### Task 5：对齐请求格式——chat_completions 客户端

**文件**：`openjax-core/src/model/chat_completions.rs`

**5a. temperature 改为 `None`（不设置）作为默认**：
当前硬编码 `temperature: 0.2`，改为 `None` 让 provider 使用自身默认。
仅当 `request_profile` 有明确要求时设置具体值。

**5b. 统一 User-Agent**：
与 anthropic 客户端一致，附加 `User-Agent: openjax/{VERSION}`。

**5c. `max_tokens` 始终设置**：
同 Task 4c，使用 `min(context_window_size, 32000)`。

**5d. 移除 `kimi_coding_v1` request profile 中的 `KimiCLI/0.77`**：
Kimi 迁移到 anthropic_messages 协议后，此 profile 不再被 Kimi 使用。
如果保留 profile 用于其他目的，移除 User-Agent 覆盖逻辑。

---

### Task 6：WebUI / Gateway — provider 创建/编辑支持选择 protocol

**文件**：
- `openjax-gateway/src/handlers/provider.rs` — 请求体增加 `protocol` 字段，透传到 store
- `ui/web/` — provider 表单增加 protocol 下拉选项（`chat_completions` / `anthropic_messages`）
- Gateway API 相关集成测试更新

**protocol 下拉选项文案**：
- `chat_completions` → "OpenAI Compatible"
- `anthropic_messages` → "Anthropic Compatible (Claude / Kimi / MiniMax / GLM)"

---

### Task 7：清理 `anthropic_messages.rs` 中的 GLM legacy 路径

**文件**：`openjax-core/src/model/anthropic_messages.rs`

- `from_glm_config()` 中有 `is_legacy_glm_chat_base_url()` 检测逻辑，用于兼容旧的 GLM chat base URL（`/api/coding/paas/v4`）
- GLM 统一切换到 anthropic endpoint 后，此 legacy 兼容逻辑可以删除
- 同时删除 `chat_completions.rs` 中的 `from_glm_config()`（GLM 不再走 chat_completions 路径）

---

## 任务依赖关系

```
Task 1 (DB schema)
    └── Task 2 (废弃启发式推断，改用 protocol 字段)
    └── Task 6 (WebUI 支持 protocol 选择)

Task 3 (修复 catalog) — 独立，可先做

Task 4 (anthropic 请求格式)  — 独立
Task 5 (chat_completions 请求格式) — 独立

Task 7 (清理 GLM legacy) — 依赖 Task 3 完成后确认 GLM 走新路径
```

推荐执行顺序：**Task 3 → Task 4 → Task 5 → Task 1 → Task 2 → Task 6 → Task 7**

Task 3/4/5 无 DB 变更，风险低，先做可以验证请求格式是否正确；Task 1/2/6 涉及 DB migration 和 WebUI，最后做。

---

## OpenCode 参考索引

| 内容 | OpenCode 文件路径 |
|---|---|
| provider 加载与 baseURL 解析 | `packages/opencode/src/provider/provider.ts` → `getSDK()` (L1186-1317) |
| 请求参数构造（temperature/topP/maxTokens）| `packages/opencode/src/provider/transform.ts` → `options()` (L717-835), `temperature()` (L295-311), `topP()` (L313-320), `maxOutputTokens()` (L913-915) |
| streamText 调用（headers/providerOptions）| `packages/opencode/src/session/llm.ts` → `stream()` (L48-294) |
| Anthropic beta header 注入 | `packages/opencode/src/provider/provider.ts` → `CUSTOM_LOADERS.anthropic` (L153-162) |
| 消息规范化（空内容过滤/tool_id 修正）| `packages/opencode/src/provider/transform.ts` → `normalizeMessages()` (L47-172) |
| Prompt caching 注入 | `packages/opencode/src/provider/transform.ts` → `applyCaching()` (L174-215) |
| models.dev provider 数据 | `packages/opencode/src/provider/models.ts` |
| config schema（Provider/options/baseURL）| `packages/opencode/src/config/config.ts` → `Config.Provider` (L978-1036) |
| provider variants（reasoning effort）| `packages/opencode/src/provider/transform.ts` → `variants()` (L335-714) |

### models.dev 关键 provider 数据（2026-03 快照）

| provider | api base_url | npm（对应协议）|
|---|---|---|
| `anthropic` | （由 SDK 内置）`https://api.anthropic.com` | `@ai-sdk/anthropic` → `anthropic_messages` |
| `kimi-for-coding` | `https://api.kimi.com/coding/v1` | `@ai-sdk/anthropic` → `anthropic_messages` |
| `minimax` | `https://api.minimax.io/anthropic/v1` | `@ai-sdk/anthropic` → `anthropic_messages` |
| `minimax-cn` | `https://api.minimaxi.com/anthropic/v1` | `@ai-sdk/anthropic` → `anthropic_messages` |
| `zhipuai` (GLM) | `https://open.bigmodel.cn/api/paas/v4` | `@ai-sdk/openai-compatible` → `chat_completions` |
| `zhipuai-coding-plan` (GLM coding) | `https://open.bigmodel.cn/api/coding/paas/v4` | `@ai-sdk/openai-compatible` → `chat_completions` |

> **注意**：models.dev 中 GLM（zhipuai）使用 `@ai-sdk/openai-compatible`（即 chat_completions 协议），
> 但 GLM 官方同时提供 Anthropic-compatible endpoint（`/api/anthropic`）。
> 本计划选择走 Anthropic-compatible endpoint，与 Kimi/MiniMax 保持一致，
> 理由是接口稳定性更好、与 extended thinking 格式对齐。
> 如果后续发现 GLM Anthropic endpoint 有兼容性问题，可以回退到 chat_completions 路径。

---

## 涉及文件汇总

```
openjax-store/
  src/types.rs                    — ProviderRecord 增加 protocol 字段
  src/sqlite.rs                   — DB schema migration + CRUD 更新
  src/repository.rs               — trait 签名更新

openjax-core/
  src/builtin_catalog.rs          — 修正 Kimi/MiniMax/GLM 的 protocol 和 base_url
  src/provider_store.rs           — 废弃 provider_protocol() 启发式，改用显式字段
  src/model/anthropic_messages.rs — 增加 anthropic-beta header，统一 User-Agent，max_tokens 默认值
  src/model/chat_completions.rs   — temperature 改为 None，统一 User-Agent，max_tokens 默认值
  src/model/request_profiles/     — kimi_coding_v1 profile 移除 User-Agent 覆盖

openjax-gateway/
  src/handlers/provider.rs        — 请求体增加 protocol 字段

ui/web/
  src/                            — provider 表单增加 protocol 下拉

tests/
  相关集成测试同步更新
```
