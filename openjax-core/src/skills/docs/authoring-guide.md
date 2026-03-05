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
4. `/skill-name` 语法应说明为“触发标识”，不要写成 shell 可执行命令
5. 对提交类技能，默认使用轻量检查：`git status --short` + `git diff --stat`
6. 大体量变更先给摘要，再按需展开局部 diff，避免默认全量 `git diff`

## 反例与正例

反例（容易误导模型执行 shell）：

```markdown
```bash
/local-commit
```
```

正例（触发标识 + 执行步骤）：

```markdown
触发标识：`/local-commit`（仅用于匹配技能，不是 shell 命令）

执行步骤：
1. `git status --short`
2. `git diff --stat`
3. 按需查看局部 diff
4. `git add -A`
5. `git commit -m "..."`
```
