# Skills 配置

## config.toml

```toml
[skills]
enabled = true
max_selected = 3
max_prompt_chars = 6000
```

## 环境变量覆盖

1. `OPENJAX_SKILLS_ENABLED`
2. `OPENJAX_SKILLS_MAX_SELECTED`
3. `OPENJAX_SKILLS_MAX_PROMPT_CHARS`

优先级：环境变量 > 配置文件 > 默认值。

## 默认值

1. `enabled = true`
2. `max_selected = 3`
3. `max_prompt_chars = 6000`
