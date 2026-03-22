# model 模块

`openjax-core/src/model/` 负责统一模型抽象、配置解析、多模型路由与具体协议适配器实现。

## 目录与职责

- `mod.rs`: 模块导出，向上层暴露 `ModelClient`、`build_model_client*`。
- `client.rs`: 抽象接口：
  - `ModelClient`（供 Agent 使用）
  - `ProviderAdapter`（供路由器统一调度）
- `types.rs`: 统一请求/响应结构：
  - `ModelStage`：`planner` / `final_writer` / `tool_reasoning`
  - `ModelRequest` / `ModelResponse` / `CapabilityFlags`
- `factory.rs`: 入口装配。优先使用新注册表配置，必要时桥接 legacy 配置。
- `registry.rs`: 将 `[model]` / `[model.models]` / `[model.routing]` 规范化为 `ModelRegistry`。
- `router.rs`: `ModelRouter`，按 stage 选主模型并执行单次调用（主模型失败直接报错，不自动 fallback）。
- `chat_completions.rs`: OpenAI 兼容协议适配器（OpenAI / MiniMax / GLM chat-completions）。
- `anthropic_messages.rs`: Anthropic Messages 协议适配器（Anthropic / GLM anthropic 兼容）。
- `../streaming/parser/`: provider 流式读取统一入口（`SseParser`）。
- `echo.rs`: 回显模型，主要用于调试。
- `missing_config.rs`: 当无可用模型配置时返回可读的缺省错误客户端。

## 组装流程（factory）

1. 检查 legacy `backend=echo`，命中则直接返回 `EchoModelClient`。
2. 从配置构建 `ModelRegistry`（支持 legacy 字段桥接到 `model.models.default`）。
3. 为 registry 中每个模型构建 `ProviderAdapter`。
4. 若适配器列表为空，按 legacy/env fallback 顺序尝试：
   - MiniMax chat-completions
   - Anthropic messages
   - GLM anthropic-messages
   - OpenAI chat-completions
   - GLM chat-completions
5. 仍不可用则返回 `MissingConfigModelClient`。

## 路由（router）

- 主路由由 `ModelStage` 决定：
  - `planner` -> `routing.planner`
  - `final_writer` -> `routing.final_writer`
  - `tool_reasoning` -> `routing.tool_reasoning`
- 当前运行时只调用路由选中的主模型一次；若主模型失败会直接返回错误，不自动 fallback 到其它 provider。
- 路由会基于能力位过滤不匹配模型，例如：
  - `require_reasoning=true` 时跳过不支持 reasoning 的模型。
  - 流式调用时跳过不支持 stream 的模型。

## 配置要点

- 推荐新配置：
  - `[model.models.<id>]` 定义 provider/protocol/model/base_url/api_key 等。
  - `[model.routing]` 定义分阶段主模型和 fallback。
- 兼容 legacy 配置：
  - `[model]` 下 `backend/model/api_key/base_url` 仍可用。
  - 当新旧配置并存时，新配置优先。

## 常见环境变量

- OpenAI: `OPENAI_API_KEY`, `OPENJAX_MODEL`, `OPENAI_BASE_URL`
- Anthropic: `OPENJAX_ANTHROPIC_API_KEY`, `OPENJAX_ANTHROPIC_MODEL`, `OPENJAX_ANTHROPIC_BASE_URL`, `OPENJAX_ANTHROPIC_VERSION`
- GLM: `OPENJAX_GLM_API_KEY`, `OPENJAX_GLM_MODEL`, `OPENJAX_GLM_BASE_URL`, `OPENJAX_GLM_ANTHROPIC_BASE_URL`
- MiniMax: `OPENJAX_MINIMAX_API_KEY`, `OPENJAX_MINIMAX_MODEL`, `OPENJAX_MINIMAX_BASE_URL`
- Thinking 与日志：`OPENJAX_THINKING_BUDGET_TOKENS`, `OPENJAX_LOG_THINKING`

## 扩展建议

- 新增协议时优先实现 `ProviderAdapter`，再在 `factory.rs` 的 `build_adapter_for_registered_model` 挂载。
- 新增能力位判断时，同步更新 `CapabilityFlags` 与 `router.rs` 过滤逻辑。
- 若修改默认模型/URL/env 变量，需同步更新 `openjax-core/README.md` 的环境变量表。
- provider 流式代码应复用 `streaming/parser`，避免在 provider 内重复 `pending + line split` 逻辑。
