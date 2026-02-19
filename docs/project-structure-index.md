## 项目结构索引

### 总览

OpenJax 是一个基于 Rust 实现的 CLI 代理框架，使 AI 模型能够与代码库交互。项目采用 Cargo 工作区结构，包含以下核心包：

| 包名 | 描述 |
|------|------|
| `openjax-protocol` | 协议类型与共享数据结构 |
| `openjax-core` | 代理编排、工具系统与模型客户端 |
| `openjax-cli` | CLI 入口与交互界面 |
| `smoke_test` | 冒烟测试用例 |

---

## 模块详解

### 1. openjax-protocol（协议层）

**路径**: `openjax-protocol/`

**功能**: 定义跨模块共享的核心类型和数据结构，是整个系统的类型基础。

**核心类型** ([src/lib.rs](openjax-protocol/src/lib.rs)):

| 类型 | 描述 |
|------|------|
| `ThreadId` | 代理线程唯一标识符 |
| `AgentSource` | 代理来源（Root/SubAgent） |
| `AgentStatus` | 代理状态（PendingInit/Running/Completed/Errored 等） |
| `Op` | 操作类型（UserTurn/SpawnAgent/SendToAgent/InterruptAgent/ResumeAgent/Shutdown） |
| `Event` | 事件类型（TurnStarted/ToolCallStarted/ToolCallCompleted/AssistantMessage 等） |

**常量**:
- `MAX_AGENT_DEPTH`: 最大代理嵌套深度（当前为 1）

---

### 2. openjax-core（核心层）

**路径**: `openjax-core/`

**功能**: 实现代理编排、工具执行、模型通信和配置管理的核心逻辑。

#### 2.1 核心模块

| 文件 | 功能 |
|------|------|
| [lib.rs](openjax-core/src/lib.rs) | 核心库入口，定义 `Agent` 结构体和主循环逻辑 |
| [model.rs](openjax-core/src/model.rs) | 模型客户端实现（OpenAI/MiniMax/GLM/Echo） |
| [config.rs](openjax-core/src/config.rs) | 配置结构定义与加载逻辑 |
| [logger.rs](openjax-core/src/logger.rs) | 日志初始化 |

#### 2.2 Agent 结构 ([lib.rs](openjax-core/src/lib.rs))

**核心职责**:
- 处理用户输入（自然语言或工具调用语法）
- 调用模型客户端进行规划决策
- 执行工具调用并收集结果
- 管理对话历史和状态

**关键常量**:
- `MAX_TOOL_CALLS_PER_TURN`: 单回合最大工具调用次数（5）
- `MAX_PLANNER_ROUNDS_PER_TURN`: 单回合最大规划轮次（10）
- `MAX_CONVERSATION_HISTORY_ITEMS`: 最大对话历史条目（20）

**主要方法**:
- `submit(Op)`: 提交操作，返回事件流
- `spawn_sub_agent()`: 创建子代理（预留扩展）

#### 2.3 模型客户端 ([model.rs](openjax-core/src/model.rs))

**接口**: `ModelClient` trait

**实现**:
| 客户端 | 描述 | 环境变量 |
|--------|------|----------|
| `ChatCompletionsClient` | OpenAI 兼容 API 客户端 | `OPENAI_API_KEY`, `OPENJAX_MODEL` |
| `ChatCompletionsClient` | MiniMax API 客户端 | `OPENJAX_MINIMAX_API_KEY` |
| `ChatCompletionsClient` | GLM API 客户端 | `OPENJAX_GLM_API_KEY` |
| `EchoModelClient` | 回退测试客户端 | 无 |

#### 2.4 配置系统 ([config.rs](openjax-core/src/config.rs))

**配置结构**:
- `Config`: 顶层配置
  - `ModelConfig`: 模型配置（backend/api_key/base_url/model）
  - `SandboxConfig`: 沙箱配置（mode/approval_policy）
  - `AgentConfig`: 代理配置（max_agents/max_depth）

**配置加载顺序**:
1. `./.openjax/config/config.toml`
2. `~/.openjax/config.toml`

---

### 3. tools 模块（工具系统）

**路径**: `openjax-core/src/tools/`

**功能**: 实现工具注册、路由、执行和沙箱管理。

#### 3.1 核心组件

| 文件 | 功能 |
|------|------|
| [mod.rs](openjax-core/src/tools/mod.rs) | 模块入口，导出公共类型 |
| [router.rs](openjax-core/src/tools/router.rs) | 工具调用解析、运行时配置 |
| [router_impl.rs](openjax-core/src/tools/router_impl.rs) | 工具路由器实现 |
| [context.rs](openjax-core/src/tools/context.rs) | 工具上下文、载荷、输出类型 |
| [registry.rs](openjax-core/src/tools/registry.rs) | 工具注册表 |
| [spec.rs](openjax-core/src/tools/spec.rs) | 工具规范定义 |
| [error.rs](openjax-core/src/tools/error.rs) | 错误类型定义 |
| [common.rs](openjax-core/src/tools/common.rs) | 通用工具函数 |

#### 3.2 工具编排

| 文件 | 功能 |
|------|------|
| [orchestrator.rs](openjax-core/src/tools/orchestrator.rs) | 工具编排器（钩子、批准、沙箱） |
| [sandboxing.rs](openjax-core/src/tools/sandboxing.rs) | 沙箱策略管理 |
| [hooks.rs](openjax-core/src/tools/hooks.rs) | 钩子执行器 |
| [events.rs](openjax-core/src/tools/events.rs) | 钩子事件定义 |
| [dynamic.rs](openjax-core/src/tools/dynamic.rs) | 动态工具管理 |
| [tool_builder.rs](openjax-core/src/tools/tool_builder.rs) | 工具构建器 |

#### 3.3 工具处理器 (handlers/)

| 文件 | 工具名 | 功能 |
|------|--------|------|
| [read_file.rs](openjax-core/src/tools/handlers/read_file.rs) | `read_file` | 文件读取（分页、缩进感知） |
| [list_dir.rs](openjax-core/src/tools/handlers/list_dir.rs) | `list_dir` | 目录列出（递归、分页） |
| [grep_files.rs](openjax-core/src/tools/handlers/grep_files.rs) | `grep_files` | 文件搜索（ripgrep） |
| [shell.rs](openjax-core/src/tools/handlers/shell.rs) | `shell` | Shell 命令执行 |
| [apply_patch.rs](openjax-core/src/tools/handlers/apply_patch.rs) | `apply_patch` | 补丁应用 |
| [edit_file_range.rs](openjax-core/src/tools/handlers/edit_file_range.rs) | `edit_file_range` | 行范围编辑 |

#### 3.4 apply_patch 子模块 (apply_patch/)

| 文件 | 功能 |
|------|------|
| [types.rs](openjax-core/src/tools/apply_patch/types.rs) | 补丁类型定义（PatchOperation/PatchHunk/PlannedAction） |
| [parser.rs](openjax-core/src/tools/apply_patch/parser.rs) | 补丁解析器 |
| [planner.rs](openjax-core/src/tools/apply_patch/planner.rs) | 补丁行动计划 |
| [matcher.rs](openjax-core/src/tools/apply_patch/matcher.rs) | 行匹配算法 |
| [applier.rs](openjax-core/src/tools/apply_patch/applier.rs) | 补丁应用逻辑 |
| [heredoc.rs](openjax-core/src/tools/apply_patch/heredoc.rs) | Heredoc 格式处理 |
| [tool.rs](openjax-core/src/tools/apply_patch/tool.rs) | 工具入口 |
| [grammar.lark](openjax-core/src/tools/apply_patch/grammar.lark) | Lark 语法定义 |

#### 3.5 核心类型

**工具调用**:
```rust
struct ToolCall {
    name: String,
    args: HashMap<String, String>,
}
```

**运行时配置**:
```rust
struct ToolRuntimeConfig {
    approval_policy: ApprovalPolicy,  // AlwaysAsk/OnRequest/Never
    sandbox_mode: SandboxMode,        // WorkspaceWrite/DangerFullAccess
    shell_type: ShellType,
    tools_config: ToolsConfig,
}
```

**沙箱策略**:
```rust
enum SandboxPolicy {
    None,
    ReadOnly,
    Write,
    DangerFullAccess,
}
```

---

### 4. openjax-cli（CLI 层）

**路径**: `openjax-cli/`

**功能**: 提供命令行交互界面。

#### 4.1 主要文件

| 文件 | 功能 |
|------|------|
| [src/main.rs](openjax-cli/src/main.rs) | CLI 入口，REPL 循环 |
| [tests/e2e_cli.rs](openjax-cli/tests/e2e_cli.rs) | 端到端测试 |
| [config.toml.example](openjax-cli/config.toml.example) | 配置示例 |

#### 4.2 CLI 参数

| 参数 | 描述 |
|------|------|
| `--model` | 模型后端（minimax/openai/echo） |
| `--approval` | 审批策略（always_ask/on_request/never） |
| `--sandbox` | 沙箱模式（workspace_write/danger_full_access） |
| `--config` | 配置文件路径 |

#### 4.3 交互命令

- 直接输入文本：自然语言对话
- `tool:<name> <args>`：直接工具调用
- `/exit`：退出程序

---

### 5. docs（文档目录）

**路径**: `docs/`

**结构**:
```
docs/
├── plan/                    # 计划文档
│   └── tool/               # 工具相关计划
├── tools/                   # 工具系统文档
│   ├── architecture.md     # 架构说明
│   ├── core-components.md  # 核心组件
│   ├── usage-guide.md      # 使用指南
│   └── ...
├── codex-architecture-reference.md  # Codex 架构参考
├── codex-quick-reference.md         # Codex 快速参考
├── config.md                        # 配置说明
├── security.md                      # 安全说明
└── project-structure-index.md       # 本文档
```

---

## 构建与测试

### 构建命令

```bash
# 构建所有包
cargo build

# 构建特定包
cargo build -p openjax-cli

# 发布构建
cargo build --release
```

### 测试命令

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test -p openjax-core m3_sandbox
cargo test -p openjax-core m4_apply_patch

# 显示输出
cargo test -p openjax-core -- --nocapture
```

---

## 环境变量

### 模型配置

| 变量 | 描述 | 默认值 |
|------|------|--------|
| `OPENAI_API_KEY` | OpenAI API 密钥 | - |
| `OPENJAX_MODEL` | OpenAI 模型名 | `gpt-4.1-mini` |
| `OPENAI_BASE_URL` | OpenAI API 地址 | `https://api.openai.com/v1` |
| `OPENJAX_MINIMAX_API_KEY` | MiniMax API 密钥 | - |
| `OPENJAX_MINIMAX_MODEL` | MiniMax 模型名 | `codex-MiniMax-M2.1` |
| `OPENJAX_GLM_API_KEY` | GLM API 密钥 | - |

### 运行时配置

| 变量 | 描述 | 可选值 |
|------|------|--------|
| `OPENJAX_APPROVAL_POLICY` | 审批策略 | `always_ask`/`on_request`/`never` |
| `OPENJAX_SANDBOX_MODE` | 沙箱模式 | `workspace_write`/`danger_full_access` |

---

## 工具语法

```bash
# 文件操作
tool:read_file file_path=src/lib.rs
tool:list_dir dir_path=.
tool:grep_files pattern=fn\ main path=.

# 命令执行
tool:shell cmd='cargo test'
tool:shell cmd='rm file.txt' require_escalated=true timeout_ms=60000

# 文件编辑
tool:apply_patch patch='*** Begin Patch\n*** Add File: new.rs\n+// new\n*** End Patch'
tool:edit_file_range file_path=src/lib.rs start_line=1 end_line=5 new_text='// replaced'
```

---

## 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                        openjax-cli                          │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    main.rs                           │   │
│  │  - CLI 参数解析                                       │   │
│  │  - REPL 循环                                          │   │
│  │  - 事件打印                                           │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       openjax-core                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   lib.rs     │  │   model.rs   │  │    config.rs     │  │
│  │   Agent      │  │ ModelClient  │  │    Config        │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│         │                                                   │
│         ▼                                                   │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                    tools/                             │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐  │  │
│  │  │   router    │  │ orchestrator│  │   handlers   │  │  │
│  │  │ ToolRouter  │  │ ToolOrchestrator│ read_file   │  │  │
│  │  │ ToolCall    │  │ HookExecutor    │ list_dir    │  │  │
│  │  └─────────────┘  └─────────────┘  │ grep_files   │  │  │
│  │         │                │         │ shell        │  │  │
│  │         ▼                ▼         │ apply_patch  │  │  │
│  │  ┌─────────────┐  ┌─────────────┐  │ edit_file_   │  │  │
│  │  │  sandboxing │  │   hooks     │  │   range      │  │  │
│  │  │SandboxManager│ │HookExecutor │  └──────────────┘  │  │
│  │  └─────────────┘  └─────────────┘                    │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    openjax-protocol                         │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                     lib.rs                            │  │
│  │  ThreadId | AgentSource | AgentStatus | Op | Event   │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```
