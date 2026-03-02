# 03 配置 Schema 与兼容策略

## 新配置结构

```toml
[model.routing]
planner = "glm_fast"
final_writer = "glm_quality"
tool_reasoning = "glm_fast"

[model.routing.fallbacks]
glm_fast = ["glm_quality", "openai_backup"]

[model.models.glm_fast]
provider = "glm"
protocol = "anthropic_messages"
model = "GLM-4.7"
base_url = "https://open.bigmodel.cn/api/anthropic"
api_key_env = "OPENJAX_GLM_API_KEY"
thinking_budget_tokens = 2000
supports_stream = true
supports_reasoning = true
```

## legacy 配置仍可用

```toml
[model]
backend = "glm"
model = "GLM-4.7"
base_url = "https://open.bigmodel.cn/api/anthropic"
api_key = "<key>"
```

## 桥接规则

1. 仅有 legacy 时，自动桥接为 `model.models.default`。
2. 既有 new 又有 legacy 时：优先 new，打印 warning。
3. 默认路由：`planner/final_writer/tool_reasoning` 都指向首个模型。

## 弃用节奏

1. `vNext`：双栈兼容 + warning。
2. `vNext+1`：继续兼容 + 文档强调迁移截止。
3. `vNext+2`：计划移除 legacy 解析（需单独发布公告）。
