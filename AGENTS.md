# Repository Guidelines

## 项目概述
OpenJax 的长期愿景是打造一个类似贾维斯（Jarvis）的全能 AI 助理，而不只是编程助手。当前阶段聚焦于夯实底层基础能力：参考 codex 的实现思路，逐步建设 tool 调用、shell 执行、agent loop、沙箱机制和多模型支持等核心能力。

项目当前以 Rust 实现内核，并提供 CLI 代理框架，让 AI 模型能够先稳定地与代码库交互；在此基础上，再向更通用的个人助理能力持续演进。

[本仓库架构详细介绍](docs/project-structure-index.md)
[codex 仓库的说明文档详细版](docs/codex-architecture-reference.md)
[codex 仓库的参考文档简略版](docs/codex-quick-reference.md)
[codex 仓库本地路径](/Users/ericw/work/code/ai/codex)


## 项目结构与模块划分
OpenJax 是 Rust workspace，核心由 4 个 crate 组成：
- `openjax-core/`：Agent 主循环、模型客户端、工具路由、沙箱与工具处理器。
- `openjax-protocol/`：跨模块共享的协议与数据结构。
- `openjax-cli/`：命令行入口与交互层。
- `openjax-tui/`：Rust TUI（ratatui）实现。

配套目录：
- `openjax-core/tests/`：核心集成测试（如 sandbox、`apply_patch`）。
- `openjax-cli/tests/`：CLI 端到端测试。
- `openjax-tui/tests/`：Rust TUI 交互与渲染测试。
- `python/openjax_tui/`：Python TUI MVP（当前 Python TUI 主实现）。
- `python/openjax_sdk/`：Python SDK（供 Python TUI 调用 daemon）。
- `docs/`：架构、工具设计与实施文档。
- `smoke_test/`：轻量级冒烟验证。

## Python TUI 当前实现约定
Python TUI 目前为 MVP 形态，核心逻辑集中在单文件，后续再按功能逐步拆分。

当前目录结构（以现状为准）：
- `python/openjax_tui/src/openjax_tui/app.py`：事件循环、输入处理、审批交互、logo 渲染、流式输出与工具摘要。
- `python/openjax_tui/src/openjax_tui/__main__.py`：`python -m openjax_tui` 启动入口。
- `python/openjax_tui/tests/`：输入后端、输入归一化、logo 选择、流式渲染、工具汇总、smoke 测试。

当前行为约束（基于现有实现）：
- 输入后端默认优先 `prompt_toolkit`（TTY 且依赖可用），否则回退 `basic`。
- 支持审批相关命令：`/approve <id> y|n`、快捷 `y|n`、`/pending`。
- Logo 采用 long/short/tiny 三档并按终端宽度选择，终端支持时启用 ANSI 渐变。
- 工具调用按 turn 聚合，在 `turn_completed` 时输出单行摘要（calls/ok/fail/duration/tools）。
- `assistant_delta` 与最终 `assistant_message` 需要去重，避免重复渲染。

演进要求：
- Python TUI 改动需优先补充 `python/openjax_tui/tests/` 对应测试。
- 在完成模块化拆分前，避免继续把高耦合新逻辑堆入 `app.py`；优先提取可测试函数。
- Python TUI 仅负责交互层，不复制 `openjax-core` 的核心业务逻辑。

## 构建、测试与开发命令
本仓库统一使用 `zsh` 执行命令。

- `zsh -lc "cargo build"`：构建整个 workspace。
- `zsh -lc "cargo build -p openjax-cli"`：仅构建 CLI。
- `zsh -lc "cargo build -p openjax-tui"`：仅构建 Rust TUI。
- `zsh -lc "cargo test"`：运行全部测试。
- `zsh -lc "cargo test -p openjax-core m3_sandbox"`：运行沙箱相关测试。
- `zsh -lc "cargo test -p openjax-core m4_apply_patch"`：运行补丁工具相关测试。
- `zsh -lc "cargo test -p openjax-core -- --nocapture"`：显示测试过程输出。
- `zsh -lc "PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest discover -s python/openjax_tui/tests -v"`：运行 Python TUI 测试。

## 代码风格与命名规范
遵循 Rust 默认规范，并在提交前保持 `rustfmt` 格式一致。
- 缩进：4 个空格。
- 命名：函数/文件/模块使用 `snake_case`，类型/trait 使用 `PascalCase`，常量使用 `SCREAMING_SNAKE_CASE`。
- 模块设计：优先小而清晰的职责划分（参考 `openjax-core/src/tools/`）。
- 注释：简洁说明“意图与约束”，避免解释显而易见的语法。

## 测试规范
Rust 模块使用 `cargo test`，Python TUI 使用 `python3 -m unittest`。功能改动必须补充对应测试，重点覆盖：
- `openjax-core/tests/`：工具行为、路径校验、沙箱策略、补丁应用边界。
- `openjax-cli/tests/`：用户可见交互与命令行为。
- `openjax-tui/tests/`：Rust TUI 事件映射、布局渲染、审批与终端恢复。
- `python/openjax_tui/tests/`：Python TUI 输入、渲染、审批命令与 smoke 行为。

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
