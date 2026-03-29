# Codex 技术架构参考文档

本文档基于 OpenAI Codex 项目源码分析，旨在为 OpenJax 项目开发提供技术参考。

## 1. 项目概述

Codex 是 OpenAI 开发的本地 CLI 编程代理框架，支持与代码库交互。项目采用 Rust 实现核心逻辑，提供模块化架构支持工具执行、沙箱环境和多模型交互。

### 1.1 仓库结构

```
codex/
├── codex-cli/          # TypeScript CLI 包装器
├── codex-rs/           # Rust 实现（主要维护版本）
│   ├── core/           # 业务逻辑库
│   ├── exec/           # 头less CLI（codex exec）
│   ├── tui/            # 交互式 TUI（Ratatui）
│   ├── cli/            # CLI 多工具入口
│   └── ...
├── shell-tool-mcp/     # Shell 工具 MCP 服务器
└── docs/               # 项目文档
```

## 2. 核心架构

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                      Codex CLI                              │
├─────────────────────────────────────────────────────────────┤
│  cli/src/main.rs                                           │
│    ↓                                                       │
│  codex_tui (TUI 界面)                                     │
│  codex_exec (非交互模式)                                   │
├─────────────────────────────────────────────────────────────┤
│                  codex-core (业务逻辑)                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │   Codex     │ │   Agent     │ │   Tools Router      │   │
│  │   (主入口)  │ │  (编排)     │ │   (工具分发)        │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │  ModelClient│ │  Protocol   │ │   Sandboxing        │   │
│  │  (模型交互) │ │  (协议)     │ │   (沙箱安全)        │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                    外部服务                                 │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ OpenAI API  │ │  MCP Server │ │   本地工具          │   │
│  │ (模型推理)  │ │  (扩展工具)  │ │   (exec/read_file)  │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心模块

#### Codex 主入口 (`core/src/codex.rs`)
- **职责**: 高层接口，操作 Submission/Event 队列
- **关键结构**:
  ```rust
  pub struct Codex {
      pub(crate) tx_sub: Sender<Submission>,  // 提交队列
      pub(crate) rx_event: Receiver<Event>,    // 事件队列
      pub(crate) agent_status: watch::Receiver<AgentStatus>,
      pub(crate) session: Arc<Session>,
  }
  ```
- **工作流程**: 接收 Submission → 编排 Agent → 发送 Events

#### Agent 编排 (`core/src/agent/`)
- **文件结构**:
  - `mod.rs` - 模块定义
  - `orchestrator.rs` - 编排器核心
  - `control.rs` - Agent 控制
  - `guards.rs` - 深度限制检查
  - `role.rs` - Agent 角色
  - `status.rs` - 状态管理

#### 工具系统 (`core/src/tools/`)
- **工具注册与分发** (`spec.rs`):
  - `ToolRegistryBuilder` - 构建工具注册表
  - `ToolsConfig` - 工具配置
  - 动态工具过滤和规格定义

- **工具处理器** (`handlers/`):
  - `shell.rs` / `unified_exec.rs` - 命令执行
  - `read_file.rs` - 读取文件
  - `list_dir.rs` - 列出目录
  - `grep_files.rs` - 文件搜索
  - `search_tool_bm25.rs` - BM25 搜索
  - `js_repl.rs` - JS REPL
  - `mcp.rs` / `mcp_resource.rs` - MCP 工具

- **工具沙箱** (`sandboxing/`):
  - `sandboxing.rs` - 沙箱接口
  - `ExecApprovalRequirement` - 执行审批需求

### 2.3 协议层 (`protocol/src/`)

#### Submission/Event 模式 (SQ/EQ)
```rust
// 提交队列条目 - 用户请求
pub struct Submission {
    pub id: String,
    pub op: Op,
}

// 操作类型
pub enum Op {
    UserTurn {              // 用户回合
        items: Vec<UserInput>,
        cwd: PathBuf,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
        model: String,
        // ...
    },
    Interrupt,             // 中断
    ExecApproval {         // 执行审批
        id: String,
        approval: ApprovalDecision,
    },
    // ...
}

// 事件类型
pub enum Event {
    TurnStarted,
    TurnCompleted,
    ToolCallStarted,
    ToolCallCompleted,
    // ...
}
```

## 3. 工具系统详解

### 3.1 工具调用流程

```
Model → Tool Call → ToolRouter → ToolHandler → Sandbox → Output → Model
                           ↓
                    Approval Check
                           ↓
                    Policy Check
```

### 3.2 核心工具实现

#### Exec Tool (`core/src/exec.rs`)
```rust
pub async fn process_exec_tool_call(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    sandbox_cwd: &Path,
    // ...
) -> Result<ExecToolCallOutput>
```

**关键参数**:
- `ExecParams.command` - 命令列表
- `ExecParams.cwd` - 工作目录
- `ExecParams.expiration` - 超时控制
- `ExecParams.sandbox_permissions` - 沙箱权限
- `ExecParams.network` - 网络代理

**输出格式**:
- Exit code
- Duration
- stdout/stderr
- 是否超时


### 3.3 工具处理器注册

```rust
// 工具注册示例
ToolRegistryBuilder::new("codex")
    .with_shell_tool(tools_config.shell_type)
    .with_file_tools()
    .with_grep_tool()
    // ...
    .build()
```

## 4. 沙箱与安全机制

### 4.1 沙箱类型

```rust
pub enum SandboxType {
    None,                      // 无沙箱
    MacosSeatbelt,            // macOS Seatbelt
    LinuxSeccomp,             // Linux Seccomp
    WindowsRestrictedToken,   // Windows 限制令牌
}
```

### 4.2 平台特定实现

#### macOS Seatbelt (`core/src/seatbelt.rs`)
- 使用 `sandbox-exec` 机制
- 策略文件: `seatbelt_base_policy.sbpl`, `seatbelt_network_policy.sbpl`
- 支持动态网络策略
- 代理绑定端口白名单

#### Linux Landlock/Seccomp (`core/src/landlock.rs`, `core/src/linux_sandbox/`)
- 使用 bubblewrap + seccomp
- `codex-linux-sandbox` 辅助程序
- 支持 `--sandbox-permission` 参数

#### Windows Sandbox (`core/src/windows_sandbox.rs`)
- 使用受限令牌 (Restricted Token)
- `WindowsSandboxLevel` 控制级别

### 4.3 安全策略评估 (`core/src/safety.rs`)

```rust
pub enum SafetyCheck {
    AutoApprove {
        sandbox_type: SandboxType,
        user_explicitly_approved: bool,
    },
    AskUser,
    Reject {
        reason: String,
    },
}

// 补丁安全评估
pub fn assess_patch_safety(
    action: &ApplyPatchAction,
    policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
    windows_sandbox_level: WindowsSandboxLevel,
) -> SafetyCheck
```

### 4.4 执行策略 (`core/src/exec_policy.rs`)

```rust
pub struct ExecPolicyManager {
    policy: ArcSwap<Policy>,  // 策略缓存
}

pub enum Decision {
    Allow,      // 允许
    Prompt,     // 提示用户
    Forbidden,  // 禁止
}

// 策略规则格式 (.rules 文件)
allow prefix "cargo test"
prompt prefix "rm -rf"
forbidden "sudo"
```

### 4.5 审批流程

```
Command → ExecPolicyManager → Decision
                          ↓
        Allow → Skip Approval → Execute
        Prompt → User Approval → Allow/Reject
        Forbidden → Reject
```

## 5. 模型客户端 (`core/src/client.rs`)

### 5.1 架构

```rust
pub struct ModelClient {
    state: Arc<ModelClientState>,
}

pub struct ModelClientSession {
    client: ModelClient,
    connection: Option<ApiWebSocketConnection>,  // WebSocket 连接
    websocket_last_request: Option<ResponsesApiRequest>,
    turn_state: Arc<OnceLock<String>>,  // 粘性路由状态
}
```

### 5.2 通信模式

- **WebSocket** (首选): `responses_websockets=2026-02-04`
- **HTTP Fallback**: 支持 WebSocket 降级

### 5.3 关键 API

```rust
impl ModelClient {
    pub async fn stream_turn(
        &self,
        turn_context: TurnContext,
        request: ResponsesApiRequest,
    ) -> Result<ResponseStream>;

    pub async fn create_response(
        &self,
        turn_context: TurnContext,
        request: ResponsesApiRequest,
    ) -> Result<()>;
}
```

## 6. TUI 实现 (`codex-rs/tui/`)

### 6.1 主要组件

```
tui/src/
├── app.rs                    # 主应用状态机
├── chatwidget.rs             # 聊天窗口
├── tui.rs                    # TUI 框架
├── history_cell.rs           # 历史记录
├── diff_render.rs            # Diff 渲染
├── markdown_render.rs        # Markdown 渲染
├── shell.rs           # 命令执行
├── approvals.rs              # 审批界面
└── ...
```

### 6.2 使用库

- **ratatui**: TUI 框架
- **crossterm**: 终端处理
- **tokio**: 异步运行时

## 7. CLI 结构

### 7.1 多工具 CLI (`cli/src/main.rs`)

```rust
enum Subcommand {
    Exec(ExecCli),           # 非交互执行
    Review(ReviewArgs),      # 代码审查
    Login(LoginCommand),     # 登录
    Mcp(McpCli),            # MCP 管理
    Sandbox(SandboxArgs),   # 沙箱调试
    Resume(ResumeCommand),  # 恢复会话
    // ...
}
```

### 7.2 命令示例

```bash
# 交互模式
codex

# 非交互执行
codex exec "fix the bug in src/main.rs"

# 代码审查
codex review --model gpt-4o

# 沙箱调试
codex sandbox macos cargo test
codex sandbox linux cargo build

# 会话管理
codex resume <session_id>
codex fork <session_id>
```

## 8. 配置系统

### 8.1 配置文件 (`~/.codex/config.toml`)

```toml
# 模型配置
[model]
default = "gpt-4o"

# API 密钥
[api]
key = "sk-..."

# 沙箱模式
sandbox_mode = "workspace-write"  # read-only | workspace-write | danger-full-access

# 审批策略
approval_policy = "on_request"     # always_ask | on_request | never

# MCP 服务器
[[mcp_servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]

# 通知
[notify]
on_turn_complete = "terminal-notifier -title Codex -message 'Turn complete'"
```

### 8.2 环境变量

```bash
OPENAI_API_KEY              # OpenAI API 密钥
OPENAI_BASE_URL             # OpenAI API 基础 URL
CODEX_APPROVAL_POLICY       # 审批策略
CODEX_SANDBOX_MODE          # 沙箱模式
```

## 9. MCP 集成

### 9.1 MCP 客户端 (`core/src/mcp/`)

```rust
pub struct McpConnectionManager {
    connections: HashMap<String, McpConnection>,
    tool_router: ToolRouter,
}

impl McpConnectionManager {
    pub async fn connect(&self, config: &McpServerConfig) -> Result<()>;
    pub async fn disconnect(&self, name: &str) -> Result<()>;
    pub fn get_tool(&self, name: &str) -> Option<&dyn McpTool>;
}
```

### 9.2 MCP 工具调用

```
Codex → MCP Tool Call → JSON-RPC Request → MCP Server
                                        ↓
                                  Tool Execution
                                        ↓
                                  JSON-RPC Response → Codex
```

## 10. 会话管理

### 10.1 会话持久化 (`core/src/rollout/`)

```
~/.codex/
├── sessions/
│   └── <thread_id>/
│       ├── meta.json           # 元数据
│       ├── config.toml        # 配置快照
│       └── events/            # 事件日志
├── archived/                  # 归档会话
└── config.toml               # 用户配置
```

### 10.2 会话状态

```rust
pub struct Session {
    pub thread_id: ThreadId,
    pub cwd: PathBuf,
    pub sandbox_policy: SandboxPolicy,
    pub model: String,
    pub turns: Vec<Turn>,
}
```

## 11. 构建与测试

### 11.1 构建命令

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release

# 运行测试
cargo test
cargo test -p codex-core
cargo test -p codex-tui -- --nocapture
```

### 11.2 Bazel 构建

```bash
bazel build //...
bazel test //...
```

## 12. OpenJax 参考要点

### 12.1 可借鉴的设计模式

1. **SQ/EQ 异步模式**: Submission → Event 队列解耦
2. **工具路由器**: 统一工具分发接口
3. **沙箱抽象层**: 平台无关的沙箱策略
4. **审批策略系统**: Policy + Decision 模式
5. **会话状态机**: 清晰的状态转换

### 12.2 建议实现顺序

1. **核心框架**: `lib.rs` → `codex.rs` → `protocol.rs`
2. **协议类型**: `Op` → `Event` → 工具调用类型
3. **基础工具**: `read_file` → `list_dir` → `grep_files`
4. **执行系统**: `exec.rs` → 沙箱集成 → 审批流程
5. **模型客户端**: WebSocket/HTTP → 工具响应处理
6. **TUI/CLI**: 交互界面 → 会话管理

### 12.3 关键技术决策

- **异步运行时**: tokio (Codex 使用)
- **TUI 框架**: ratatui (Codex 使用)
- **序列化**: serde + JSON
- **错误处理**: anyhow + thiserror
- **并发安全**: Arc + Mutex/RwLock

## 13. 参考文档

- [Codex 源码](https://github.com/openai/codex)
- [Protocol 协议定义](codex-rs/protocol/src/protocol.rs)
- [工具规格](codex-rs/core/src/tools/spec.rs)
- [沙箱实现](codex-rs/core/src/sandboxing/)
