# Skills 架构

`openjax-core/src/skills/` 模块分层如下：

1. `manifest.rs`：解析 `SKILL.md` 与 frontmatter
2. `loader.rs`：目录扫描、去重、优先级合并
3. `registry.rs`：构建 `SkillRegistry`，提供选择入口
4. `matcher.rs`：按用户输入打分并排序
5. `prompt.rs`：将选中 skills 渲染为 prompt 片段
6. `types.rs`：核心数据结构和运行时配置
7. `errors.rs`：解析错误定义

## Agent 接入链路

1. `bootstrap.rs` 初始化 `SkillRegistry` 和 `SkillRuntimeConfig`
2. `planner.rs` 每轮根据 `user_input` 调用 matcher
3. `agent/prompt.rs` 注入 `Available skills (auto-selected)` 段落

Skills 失败不会阻断主流程，只记录 warning。
