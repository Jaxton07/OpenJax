# Skills 概述

OpenJax Skills 是一个软增强层：

1. 扫描技能目录中的 `SKILL.md`
2. 解析公共字段 `name` / `description` / Markdown 正文
3. 按用户输入匹配 top-N skills
4. 将 skills 上下文注入 planner prompt

Skills 不会替代工具系统，也不会改变审批与沙箱策略。

## 目录发现范围

1. `~/.openjax/skills`

## 去重规则

1. key 为 skill 名称标准化后的 `normalized_name`
2. 同层按目录名字典序扫描，先到先得
