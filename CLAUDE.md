# OpenJax 的 AGENTS 指南
本指南面向本仓库中的自治编码代理。
请使用可复现命令，遵循现有模式，并验证改动。


## 项目概述
OpenJax 是一个基于 Rust 实现的agent系统，使 AI 模型能够与处理用户各种需求。它提供模块化架构，支持工具执行、沙箱环境和多模型支持, 参考codex 的实现。
本项目是想基于codex 的实现原理打造一个定制化的个人全能管家或助理，参考钢铁侠里贾维斯的定位(目前尚处在初级阶段-基础功能开发阶段，后续会逐渐扩展各种高级功能)。

## 1) 项目概览
- OpenJax 是一个以 Rust 为主、保留 Python SDK 的代理框架。
- Rust 工作区成员（`Cargo.toml`）：
  - `openjax-protocol`
  - `openjax-core`
  - `openjaxd`
  - `openjax-gateway`
  - `openjax-store`
  - `openjax-policy`
  - `ui/tui`
- Python 包：
  - `python/openjax_sdk`
- 架构索引：

## 2) 关键路径
- `openjax-core/`：代理循环、工具、沙箱、审批。
- `openjax-protocol/`：协议/事件/数据类型。
- `openjaxd/`：守护进程。
- `openjax-gateway/`：HTTP/SSE 网关（会话、turn、审批、事件流）。
- `openjax-store/`：SQLite 持久化存储层（会话、消息、事件、LLM provider 配置）。
- `openjax-policy/`：统一策略中心（规则匹配、版本化发布、会话 overlay、审计记录）。
- `ui/tui/`：Rust TUI（最新版）。
- `ui/web/`：React Web 前端（通过 gateway 访问会话与流式事件）。
- `python/openjax_sdk/`：面向守护进程的异步 SDK。

### 子模块 README 导航

优先阅读以下文档以快速进入对应模块上下文：
- [openjax-protocol/README.md](openjax-protocol/README.md)
- [openjax-core/README.md](openjax-core/README.md)
- [openjax-core/src/tools/README.md](openjax-core/src/tools/README.md)
- [openjax-gateway/README.md](openjax-gateway/README.md)
- [openjax-store/README.md](openjax-store/README.md)
- [openjax-policy/README.md](openjax-policy/README.md)
- [ui/tui/README.md](ui/tui/README.md)
- [ui/web/README.md](ui/web/README.md)
- [openjaxd/README.md](openjaxd/README.md)
- [python/openjax_sdk/README.md](python/openjax_sdk/README.md)

### 工具参考

- [.claude/docs/reference/cmux.md](.claude/docs/reference/cmux.md) — cmux 命令行工具快速参考


## 4) 构建命令
- `zsh -lc "cargo build"`
- `zsh -lc "cargo build -p openjax-core"`
- `zsh -lc "cargo build -p openjax-gateway"`
- `zsh -lc "cargo build -p openjax-policy"`
- `zsh -lc "cargo build -p tui_next"`
- `zsh -lc "cargo build -p openjaxd"`
- `zsh -lc "cd ui/web && pnpm build"`

## 5) Lint 与格式化
- `zsh -lc "cargo fmt -- --check"`
- `zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"`

## 6) 测试命令
### 推荐入口（core / gateway 分层测试）
- `zsh -lc "make core-smoke"`
- `zsh -lc "make core-feature-skills"`
- `zsh -lc "make core-feature-tools"`
- `zsh -lc "make core-feature-streaming"`
- `zsh -lc "make core-feature-approval"`
- `zsh -lc "make core-feature-history"`
- `zsh -lc "make core-full"`
- `zsh -lc "make core-baseline"`
- `zsh -lc "make gateway-smoke"`
- `zsh -lc "make gateway-fast"`
- `zsh -lc "make gateway-doc"`
- `zsh -lc "make gateway-full"`
- `zsh -lc "make gateway-baseline"`

### 其他测试运行
- `zsh -lc "cargo test"`
- `zsh -lc "cargo test --workspace"`
- `zsh -lc "cargo test -p openjax-policy --tests"`
- `zsh -lc "cargo test -p tui_next"`
- `zsh -lc "cd ui/web && pnpm test"`

### 单个 Rust 集成测试 / 定位用例（重要）
对 `openjax-core`、`openjax-gateway` 的日常回归，优先走上面的 `make` / `scripts/test/*.sh` 分层入口。
需要精确定位某个 suite 或单个 case 时，再直接使用 `cargo test --test <file_stem>`。
- `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"`
- `zsh -lc "cargo test -p openjax-core --test approval_suite"`
- `zsh -lc "cargo test -p openjax-core --test approval_events_suite"`
- `zsh -lc "cargo test -p openjax-core --test streaming_suite"`
- `zsh -lc "cargo test -p openjax-core --test skills_suite"`
- `zsh -lc "cargo test -p openjax-core --test core_history_suite"`
- `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite system_tools_are_registered_in_specs --locked --quiet"`
- `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite"`
- `zsh -lc "cargo test -p openjax-gateway --test policy_api_suite"`
- `zsh -lc "cargo test -p openjax-gateway --test m1_assistant_message_compat_only"`
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
  - 在 `tests/` 中以 `*_suite.rs` 作为集成测试入口（`cargo test --test <suite_stem>`）
  - 具体用例可按 `m*_*.rs` 组织在 suite 子目录中，通过 `#[path = \"...\"] mod ...;` 收编
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
  - `OPENJAX_GATEWAY_BIND`
  - `OPENJAX_GATEWAY_API_KEYS`（或兼容变量 `OPENJAX_API_KEYS`）


## 17) Python SDK 调试
- 推荐在仓库根目录执行并设置 `PYTHONPATH=python/openjax_sdk/src`。
- SDK 测试命令：
  - `zsh -lc "PYTHONPATH=python/openjax_sdk/src python3 -m unittest discover -s python/openjax_sdk/tests -v"`




## 项目级工作规则

### 第一性原理
请使用第一性原理思考。你不能总是假设我非常清楚自己想要什么和该怎么得到。请保持审慎，从原始需求和问题出发，如果动机和目标不清晰，停下来和我讨论。

### 方案规范
当需要你给出修改或重构方案时必须符合以下规范：

- 不允许给出兼容性或补丁性的方案
- 不允许过度设计，保持最短路径实现且不能违反第一条要求
- 不允许自行给出我提供的需求以外的方案，例如一些兜底和降级方案，这可能导致业务逻辑偏移问题
- 必须确保方案的逻辑正确，必须经过全链路的逻辑验证。

### 其他
- (重要)在处理 Rust 项目文件时，优先使用 JetBrains / RustRover 的 `rustrover-index` MCP server 进行符号、引用、实现、类型层级和文本索引查询；不要先使用 `rg`、`grep`、`find` 等本地搜索。只有在确认当前会话无法使用该 MCP，或其能力不足以完成当前任务时，才允许退回本地搜索；退回前必须明确说明失败点属于“未配置 / 未连接 / 当前 agent 无工具暴露 / 其他”中的哪一类。
- (重要)在分派subagent 任务时记得告知subagent 也可以使用`rustrover-index` MCP
- 本地开发环境通过make run-web-dev 启动前端和后台时需要预览时不要输入localhost 访问，统一输入127.0.0.1 加端口号访问
- 在修改过程中如果发现某个文件内容过多，或者代码量很大，记得提醒用户规划拆分计划
- 在针对某部分做修改时优先根据README 索引了解对应模块的上下文，避免自己全量搜索查看以看到太多无关内容。
- 写代码过程中尽量遵循模块化可扩展原则，避免在同一个代码文件添加过多代码。推荐500行以下，尽量不要超过800行以上，避免给后续修改造成额外工作量。
- 不要随意拉新分支，需要拉新分支时提前说明


## 18) 代码提交与 PR 工作流
### 提交前准备（同步 main）
1. 确认当前分支不是 `main`
2. `git fetch origin main` 拉取最新 main
3. 若有未提交改动先 `git stash`，完成 rebase 后 `git stash pop`
4. `git rebase origin/main`，有冲突手动解决后继续

### 提交规范
- 用 `git add <具体文件>` 逐一添加，**禁止** `git add .` 或 `git add -A`
- commit message 格式：`类型(范围): 中文摘要`
  - 类型：`feat` / `fix` / `docs` / `refactor` / `chore` / `test`
  - 可在类型前加 emoji（参考现有历史记录风格）
- message 末尾附：`Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>`
- 保持改动原子化，一个 commit 只做一件事

### 推送与 PR
- push 使用 `-u` 绑定远程：`git push -u origin <branch>`
- 用 `gh pr create` 创建 PR，base branch 统一指向 `main`
- PR title 与 commit message 风格一致，控制在 70 字以内
- PR body 必须包含两部分：
  - **Summary**：改了什么、为什么改（1-3 条要点）
  - **Test plan**：验证步骤 checklist
- 可直接调用 `/commit-pr` skill 自动完成上述全流程
