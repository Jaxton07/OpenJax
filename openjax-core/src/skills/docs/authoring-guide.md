# Skills 编写指南

## 推荐目录结构

```text
.openjax/skills/
  rust-debug/
    SKILL.md
```

## SKILL.md 最小模板

```markdown
---
name: Rust Debug
description: Diagnose rust compile and test failures
---
1. Run `cargo check`.
2. Read compiler errors.
3. Propose minimal patch.
```

## 建议

1. `name` 简洁唯一，便于 matcher 命中
2. `description` 明确场景关键词
3. instructions 写成可执行步骤，避免空泛描述
