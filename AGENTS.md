# OpenJax 的 AGENTS 指南
本指南面向本仓库中的自治编码代理。
请使用可复现命令，遵循现有模式，并验证改动。


## 项目概述
OpenJax 是一个基于 Rust 实现的代理框架，使 AI 模型能够与代码库交互。它提供模块化架构，支持工具执行、沙箱环境和多模型支持, 参考codex 的实现。
本项目是想基于codex 的实现原理打造一个定制化的个人助理，而codex 的tool 工具调用，shell 执行，agent loop, 沙箱机制，等等基本能力是我们的助理agent 也需要的

## 1) 项目概览
- OpenJax 是一个以 Rust 为主、保留 Python SDK 的代理框架。
- Rust 工作区成员（`Cargo.toml`）：
  - `openjax-protocol`
  - `openjax-core`
  - `openjaxd`
  - `openjax-gateway`
  - `ui/tui`
- Python 包：
  - `python/openjax_sdk`
- 架构索引：

## 2) 关键路径
- `openjax-core/`：代理循环、工具、沙箱、审批。
- `openjax-protocol/`：协议/事件/数据类型。
- `openjaxd/`：守护进程。
- `openjax-gateway/`：HTTP/SSE 网关（会话、turn、审批、事件流）。
- `ui/tui/`：Rust TUI（最新版）。
- `ui/web/`：React Web 前端（通过 gateway 访问会话与流式事件）。
- `python/openjax_sdk/`：面向守护进程的异步 SDK。
- `smoke_test/`：冒烟测试脚本。

### 子模块 README 导航

优先阅读以下文档以快速进入对应模块上下文：
- [openjax-protocol/README.md](openjax-protocol/README.md)
- [openjax-core/README.md](openjax-core/README.md)
- [openjax-core/src/tools/README.md](openjax-core/src/tools/README.md)
- [openjax-gateway/README.md](openjax-gateway/README.md)
- [ui/tui/README.md](ui/tui/README.md)
- [ui/web/README.md](ui/web/README.md)
- [openjaxd/README.md](openjaxd/README.md)
- [python/openjax_sdk/README.md](python/openjax_sdk/README.md)



## 3) 命令执行策略
- 从仓库根目录运行命令。
- 优先使用 `zsh -lc "..."`（与 `CLAUDE.md` 中的仓库指引一致）。

## 4) 构建命令
- `zsh -lc "cargo build"`
- `zsh -lc "cargo build -p openjax-core"`
- `zsh -lc "cargo build -p openjax-gateway"`
- `zsh -lc "cargo build -p tui_next"`
- `zsh -lc "cargo build -p openjaxd"`
- `zsh -lc "cd ui/web && pnpm build"`

## 5) Lint 与格式化
- `zsh -lc "cargo fmt -- --check"`
- `zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"`

## 6) 测试命令
### 全量测试运行
- `zsh -lc "cargo test"`
- `zsh -lc "cargo test --workspace"`
- `zsh -lc "cargo test -p openjax-core"`
- `zsh -lc "cargo test -p openjax-gateway"`
- `zsh -lc "cargo test -p tui_next"`
- `zsh -lc "cd ui/web && pnpm test"`

### 单个 Rust 集成测试（重要）
对于 `tests/` 中的文件，使用 `--test <file_stem>`。
避免对这些测试文件只使用纯过滤器形式。
- `zsh -lc "cargo test -p openjax-core --test m3_sandbox"`
- `zsh -lc "cargo test -p openjax-core --test m4_apply_patch"`
- `zsh -lc "cargo test -p openjax-core --test m5_approval_handler"`
- `zsh -lc "cargo test -p openjax-core --test m6_submit_stream"`
- `zsh -lc "cargo test -p openjax-core --test m7_backward_compat_submit"`
- `zsh -lc "cargo test -p tui_next --test m1_no_duplicate_history"`
- `zsh -lc "cargo test -p tui_next --test m10_approval_panel_navigation"`

### Rust 调试输出
- `zsh -lc "cargo test -p openjax-core -- --nocapture"`



## 7) Rust 代码风格
- 工作区版本（edition）是 `2024`。
- 使用 rustfmt 默认配置格式化；4 空格缩进。
- 命名：
  - 函数/模块/变量：`snake_case`
  - 结构体/枚举/trait：`PascalCase`
  - 常量/静态变量：`SCREAMING_SNAKE_CASE`
- 保持模块聚焦且可组合（`openjax-core/src/tools/` 是参考风格）。
- 优先使用显式类型和枚举，而不是临时拼接的字符串状态。

## 8) Python 代码风格
- Python 版本是 `>=3.10`。
- 使用 4 空格缩进和 PEP 8 命名。
- 为公共与内部函数保留类型注解（测试中也包含 `-> None`）。
- 使用 `str | None` 联合类型语法。
- 将 `python/openjax_sdk` 保持为 SDK 层；不要复制 `openjax-core` 的业务逻辑。

## 9) 导入顺序
### Rust
1. `pub mod`
2. `pub use`
3. 外部 crates
4. `std`
5. 内部 crate 导入
### Python
1. `from __future__ import annotations`
2. 标准库
3. 第三方包
4. 本地包导入

## 10) 类型与 API 表面
- Rust：对可能失败的操作优先返回 `Result<T, E>`，并为负载使用类型化结构体。
- Python：为新增/修改函数标注参数和返回值类型。
- 除非迁移是有意且有文档说明，否则保持现有公共 API 名称不变。

## 11) 错误处理
### Rust
- 在应用/服务边界使用 `anyhow::Result`。
- 对结构化工具/领域错误使用 `thiserror` 枚举。
- 为 IO/进程失败添加上下文（`context`、`with_context`）。
- 在生产路径中避免 `unwrap()`。
### Python
- 优先使用具体异常（`OpenJaxProtocolError`、`OpenJaxResponseError`）。
- 将 `contextlib.suppress(...)` 限制在清理/关闭路径。
- 不要静默吞掉非清理类错误。

## 12) 测试期望
- 任何行为变更都应包含测试新增/更新。
- Rust 模式：
  - 在 `#[cfg(test)]` 块中写单元测试
  - 在 `tests/` 中写集成测试，文件命名使用 `m*_*.rs`
- Python 模式：
  - `unittest`
  - 文件名 `test_*.py`
  - 方法名应描述单一行为
- 覆盖 happy path 和失败/边界场景。


## 14) Commit/PR 说明
- 历史记录通常使用 emoji + Conventional Commit 风格。
- 保持改动范围小且原子化。
- 在 PR 描述中包含测试证据（命令和结果）。

## 15) 安全与运行时配置
- 绝不要硬编码密钥。
- 通过环境变量配置运行时/模型策略：
  - `OPENAI_API_KEY`
  - `OPENJAX_MODEL`
  - `OPENJAX_SANDBOX_MODE`
  - `OPENJAX_APPROVAL_POLICY`
  - `OPENJAX_GATEWAY_BIND`
  - `OPENJAX_GATEWAY_API_KEYS`（或兼容变量 `OPENJAX_API_KEYS`）

## 16) Cursor/Copilot 规则文件
仓库扫描结果：
- `.cursorrules`：未找到
- `.cursor/rules/`：未找到
- `.github/copilot-instructions.md`：未找到
如果这些文件后续出现，请将其视为更高优先级并合并到本指南。

## 17) Python SDK 调试
- 推荐在仓库根目录执行并设置 `PYTHONPATH=python/openjax_sdk/src`。
- SDK 测试命令：
  - `zsh -lc "PYTHONPATH=python/openjax_sdk/src python3 -m unittest discover -s python/openjax_sdk/tests -v"`


## 项目级工作规则
- 在修改过程中如果发现某个文件内容过多，或者代码量很大，记得提醒用户规划拆分计划
- 写代码过程中尽量遵循模块化可扩展原则，避免在同一个代码文件添加过多代码。推荐500行以下，尽量不要超过800行以上，避免给后续修改造成额外工作量。
