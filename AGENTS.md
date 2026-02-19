# Repository Guidelines

## 项目概述
OpenJax 是一个基于 Rust 实现的内核的 CLI 代理框架，使 AI 模型能够与代码库交互。它提供模块化架构，支持工具执行、沙箱环境和多模型支持, 参考codex 的实现。

本项目是想基于codex 的实现原理打造一个定制化的个人助理，而codex 的tool 工具调用，shell 执行，agent loop, 沙箱机制，等等基本能力是我们的助理agent 也需要的，所以现在一边参考codex 的源码一边实现我们的agent.

[本仓库架构详细介绍](docs/project-structure-index.md)
[codex 仓库的说明文档详细版](docs/codex-architecture-reference.md)
[codex 仓库的参考文档简略版](docs/codex-quick-reference.md)
[codex 仓库本地路径](/Users/ericw/work/code/ai/codex)


## 项目结构与模块划分
OpenJax 是 Rust workspace，核心由 3 个 crate 组成：
- `openjax-core/`：Agent 主循环、模型客户端、工具路由、沙箱与工具处理器。
- `openjax-protocol/`：跨模块共享的协议与数据结构。
- `openjax-cli/`：命令行入口与交互层。

配套目录：
- `openjax-core/tests/`：核心集成测试（如 sandbox、`apply_patch`）。
- `openjax-cli/tests/`：CLI 端到端测试。
- `docs/`：架构、工具设计与实施文档。
- `smoke_test/`：轻量级冒烟验证。

## 构建、测试与开发命令
本仓库统一使用 `zsh` 执行命令。

- `zsh -lc "cargo build"`：构建整个 workspace。
- `zsh -lc "cargo build -p openjax-cli"`：仅构建 CLI。
- `zsh -lc "cargo test"`：运行全部测试。
- `zsh -lc "cargo test -p openjax-core m3_sandbox"`：运行沙箱相关测试。
- `zsh -lc "cargo test -p openjax-core m4_apply_patch"`：运行补丁工具相关测试。
- `zsh -lc "cargo test -p openjax-core -- --nocapture"`：显示测试过程输出。

## 代码风格与命名规范
遵循 Rust 默认规范，并在提交前保持 `rustfmt` 格式一致。
- 缩进：4 个空格。
- 命名：函数/文件/模块使用 `snake_case`，类型/trait 使用 `PascalCase`，常量使用 `SCREAMING_SNAKE_CASE`。
- 模块设计：优先小而清晰的职责划分（参考 `openjax-core/src/tools/`）。
- 注释：简洁说明“意图与约束”，避免解释显而易见的语法。

## 测试规范
统一使用 `cargo test`。功能改动必须补充对应测试，重点覆盖：
- `openjax-core/tests/`：工具行为、路径校验、沙箱策略、补丁应用边界。
- `openjax-cli/tests/`：用户可见交互与命令行为。

测试命名尽量沿用现有模式（如 `m*_*.rs`），并覆盖正常路径与异常路径。

## 提交与 Pull Request 规范
当前提交历史以“emoji + Conventional Commit”风格为主，例如：
- `✨ feat(tools): ...`
- `♻️ refactor(tools): ...`
- `🔧 chore(config): ...`
- `📝 docs: ...`

提交信息应聚焦单一变更、使用祈使语。PR 至少包含：
- 变更目的与行为影响；
- 涉及模块/路径；
- 测试证据（命令与结果）；
- 若行为或接口变化，附带文档/配置更新。

## 安全与配置建议
禁止硬编码密钥。模型与运行策略通过环境变量配置（如 `OPENAI_API_KEY`、`OPENJAX_MODEL`、`OPENJAX_SANDBOX_MODE`、`OPENJAX_APPROVAL_POLICY`），并根据风险等级选择合适的沙箱与审批策略。