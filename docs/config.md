# OpenJax 配置说明

本文档描述 OpenJax 模型配置的两种格式：

1. legacy 单模型配置（兼容保留）。
2. 新版多模型注册表 + 阶段路由配置（推荐）。

## 配置文件位置

用户级：`~/.openjax/config.toml`

## 启动自动生成

当以上两个路径都不存在时，`tui_next/openjaxd` 启动会自动生成默认模板（优先写入项目级路径）。
默认模板已预置以下 provider：

1. `kimi`（默认路由，模型 `kimi-for-coding`）
2. `glm`
3. `openai`
4. `claude`（anthropic）

你只需要设置对应的 API Key 环境变量即可：

1. `OPENJAX_KIMI_API_KEY`
2. `OPENJAX_GLM_API_KEY`
3. `OPENAI_API_KEY`
4. `OPENJAX_ANTHROPIC_API_KEY`

## 1) Legacy 配置（兼容）

```toml
[model]
backend = "glm" # anthropic | glm | minimax | openai | echo
model = "GLM-4.7"
base_url = "https://open.bigmodel.cn/api/anthropic"
api_key = "your_api_key"

[agent]
max_tool_calls_per_turn = 10
max_planner_rounds_per_turn = 20
```

行为：

1. 运行时自动桥接成 `model.models.default`。
2. `planner/final_writer/tool_reasoning` 默认都使用 `default`。



## 路由与回退

1. `planner`：用于规划决策轮次。
2. `final_writer`：用于最终回复流式生成。
3. `tool_reasoning`：预留阶段，默认建议与 planner 一致。
4. `fallbacks`：按模型 ID 声明回退链，最多执行 2 级 fallback。

## 环境变量

常用：

1. `OPENJAX_APPROVAL_POLICY=always_ask|on_request|never`
2. `OPENJAX_SANDBOX_MODE=workspace_write|danger_full_access`
3. `OPENJAX_LOG_LEVEL=trace|debug|info|warn|error`

Provider 相关：

1. `OPENAI_API_KEY`
2. `OPENJAX_GLM_API_KEY`
3. `OPENJAX_MINIMAX_API_KEY`
4. `OPENJAX_ANTHROPIC_API_KEY`
5. `OPENJAX_THINKING_BUDGET_TOKENS`（Anthropic 协议请求中可覆盖 thinking budget）
6. `OPENJAX_MAX_TOOL_CALLS_PER_TURN`
7. `OPENJAX_MAX_PLANNER_ROUNDS_PER_TURN`

## 请求格式策略

1. `request_profile` 为运行时内部策略，不在 Web/Gateway 暴露给用户配置。
2. 系统会按 provider 自动选择请求格式：如 Kimi Coding 使用 `kimi_coding_v1`，Anthropic 协议使用 `anthropic_default`。
3. 未命中专用策略时，默认走 OpenAI 兼容 `chat_completions` 请求格式。

## 兼容策略

1. 若只配置 legacy `[model]`，系统自动桥接并继续运行。
2. 若同时配置 legacy 与新版 `model.models`，新版优先并记录 warning。
3. 推荐尽快迁移到新版配置结构以获得多模型路由能力。
