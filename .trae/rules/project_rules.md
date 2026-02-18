# CLAUDE.md

本文档为在此代码仓库中工作的指导。

## Shell 使用

**优先使用 zsh 执行命令**，而非 bash。
示例：zsh -c "cargo build 2>&1" | head -30

## 项目概述

OpenJax 是一个基于 Rust 实现的内核的 CLI 代理框架，使 AI 模型能够与代码库交互。它提供模块化架构，支持工具执行、沙箱环境和多模型支持, 参考codex 的实现。

本项目是想基于codex 的实现原理打造一个定制化的个人助理，而codex 的tool 工具调用，shell 执行，agent loop, 沙箱机制，等等基本能力是我们的助理agent 也需要的，所以现在一边参考codex 的源码一边实现我们的agent.

[codex 仓库的说明文档详细版](docs/codex-architecture-reference.md)
[codex 仓库的参考文档简略版](docs/codex-quick-reference.md)
[codex 仓库本地路径](/Users/ericw/work/code/ai/codex)



## 项目结构索引

### 总览
- **工作区**: `openJax`
- **核心包**: `openjax-protocol/`, `openjax-core/`, `openjax-cli/`
- **辅助**: `smoke_test/`（冒烟测试用例）
- **文档**: `docs/`

### 根目录文件
- **`Cargo.toml`**: 工作区级别依赖与成员配置
- **`Cargo.lock`**: 依赖锁定文件
- **`README.md`**: 项目简介与使用说明
- **`CLAUDE.md`**: 本仓库工作指南与约定
- **`test.txt`**: 临时测试文件

### 子项目与源码
- **`openjax-protocol/`**: 协议类型与共享数据结构
  - **`src/lib.rs`**: 协议类型定义入口
- **`openjax-core/`**: 代理编排、工具与模型客户端
  - **`src/lib.rs`**: 核心库入口与代理流程
  - **`src/model.rs`**: 模型客户端实现
  - **`src/tools/`**: 工具模块目录
    - **`mod.rs`**: 工具模块声明和导出
    - **`common.rs`**: 通用工具函数（参数解析、路径验证）
    - **`router.rs`**: 工具调用解析和配置类型
    - **`router_impl.rs`**: 工具路由器实现
    - **`grep_files.rs`**: grep_files 工具（使用 ripgrep）
    - **`read_file.rs`**: read_file 工具（支持分页和缩进感知）
    - **`list_dir.rs`**: list_dir 工具（支持递归和分页）
    - **`exec_command.rs`**: exec_command 工具（shell 命令执行）
    - **`apply_patch.rs`**: apply_patch 工具（补丁解析和应用）
  - **`src/config.rs`**: 配置结构与解析
  - **`tests/`**: 核心模块测试（`m3_sandbox.rs`, `m4_apply_patch.rs`）
- **`openjax-cli/`**: CLI 入口与交互显示
  - **`src/main.rs`**: CLI 入口
  - **`tests/e2e_cli.rs`**: CLI 端到端测试
  - **`config.toml.example`**: 配置示例


### 测试与构建产物
- **`smoke_test/`**: 冒烟测试项目
  - **`src/main.rs`**: 测试入口
- **`target/`**, **`smoke_test/target/`**: 构建产物目录（可忽略）


## 构建和测试命令

```bash
# 构建所有包
zsh -c "cargo build"

# 构建特定包
zsh -c "cargo build -p openjax-cli"

# 运行所有测试
zsh -c "cargo test"

# 运行特定测试
zsh -c "cargo test -p openjax-core m3_sandbox"
zsh -c "cargo test -p openjax-core m4_apply_patch"

# 运行并显示输出
zsh -c "cargo test -p openjax-core -- --nocapture"
```

## 环境变量

### 模型后端（按顺序检查）
- `OPENAI_API_KEY`, `OPENJAX_MODEL` (默认: `gpt-4.1-mini`), `OPENAI_BASE_URL`
- `OPENJAX_MINIMAX_API_KEY`, `OPENJAX_MINIMAX_MODEL` (默认: `codex-MiniMax-M2.1`), `OPENJAX_MINIMAX_BASE_URL`

### 运行时策略
- `OPENJAX_APPROVAL_POLICY`: `always_ask` | `on_request` | `never`
- `OPENJAX_SANDBOX_MODE`: `workspace_write` | `danger_full_access`

## 架构

### 代理流程 ([lib.rs](openjax-core/src/lib.rs))
1. 用户输入通过 `Op::UserTurn` 到达
2. 如果输入匹配 `tool:<name> <args>`，则直接执行
3. 否则，使用规划提示调用模型客户端
4. 模型返回 JSON 响应: `{"action":"tool"|"final", ...}`
5. 执行工具，收集输出，返回给模型（每轮最多 5 次调用）
6. 返回 `Event` 流: `TurnStarted` → `ToolCallStarted` → `ToolCallCompleted` → `TurnCompleted`

### 模型客户端 ([model.rs](openjax-core/src/model.rs))
- `ModelClient` 特征，包含 `complete(&self, user_input) -> Result<String>`
- `ChatCompletionsClient`: OpenAI/MiniMax API 包装器（如果没有环境变量则回退到 `EchoModelClient`）

### 工具路由器 ([tools/](openjax-core/src/tools/))
- **模块结构**:
  - `mod.rs`: 工具模块声明和导出
  - `common.rs`: 通用工具函数（参数解析、路径验证、字符边界截断）
  - `router.rs`: 工具调用解析和配置类型（`ToolCall`, `ToolRuntimeConfig`, `ApprovalPolicy`, `SandboxMode`）
  - `router_impl.rs`: 工具路由器实现（`ToolRouter::execute()` 分发到各个工具）
- **工具实现**:
  - `grep_files`: 使用 ripgrep 进行高性能搜索（支持正则表达式、glob 过滤、分页）
  - `read_file`: 文件读取（支持分页、行号显示、缩进感知模式）
  - `list_dir`: 目录列出（支持递归、分页、文件类型标记）
  - `exec_command`: Shell 命令执行（支持批准策略和沙箱模式）
  - `apply_patch`: 补丁解析和应用（支持添加、删除、移动、重命名、更新文件）
- 所有路径都经过验证，防止逃逸工作区根目录

### 沙箱模式
- **WorkspaceWrite**: 限制性 shell（允许的程序: `pwd`, `ls`, `cat`, `rg`, `find`, `head`, `tail`, `wc`, `sed`, `awk`, `echo`, `stat`, `uname`, `which`, `env`, `printf`）
- **DangerFullAccess**: 无命令限制

## 工具语法

```bash
tool:read_file path=src/lib.rs
tool:list_dir path=.
tool:grep_files pattern=fn main path=.
tool:exec_command cmd='zsh -c "cargo test"' require_escalated=true timeout_ms=60000
tool:apply_patch patch='*** Begin Patch\n*** Add File: new.rs\n+// new file\n*** End Patch'
```

