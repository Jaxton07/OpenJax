# 测试与排障

## 关键测试

1. `skills_suite`（聚合 skills 相关集成测试）

## 常用命令

```bash
zsh -lc "cargo test -p openjax-core --test skills_suite"
```

## 常见问题

1. 未发现 skill  
检查目录名是否为 `skills/<skill_dir>/SKILL.md`，并确认 `SKILL.md` 存在。

2. skill 被忽略  
可能与已有 skill 名称标准化后冲突，按优先级仅保留一个。

3. prompt 未注入  
检查 `skills.enabled` 或 `OPENJAX_SKILLS_ENABLED` 是否被设置为 false。
