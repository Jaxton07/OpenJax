# 测试与排障

## 关键测试

1. `m10_skills_discovery`
2. `m11_skills_prompt_injection`
3. `m12_skills_config_toggle`
4. `m13_skills_duplicate_resolution`

## 常用命令

```bash
zsh -lc "cargo test -p openjax-core --test m10_skills_discovery"
zsh -lc "cargo test -p openjax-core --test m11_skills_prompt_injection"
zsh -lc "cargo test -p openjax-core --test m12_skills_config_toggle"
zsh -lc "cargo test -p openjax-core --test m13_skills_duplicate_resolution"
```

## 常见问题

1. 未发现 skill  
检查目录名是否为 `skills/<skill_dir>/SKILL.md`，并确认 `SKILL.md` 存在。

2. skill 被忽略  
可能与已有 skill 名称标准化后冲突，按优先级仅保留一个。

3. prompt 未注入  
检查 `skills.enabled` 或 `OPENJAX_SKILLS_ENABLED` 是否被设置为 false。
