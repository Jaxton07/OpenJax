# OpenJax 工具系统

本文档提供了 OpenJax 工具系统的完整概述，包括当前状态、架构设计、使用指南和扩展指南。

## 目录

- [概述](#概述)
- [架构设计](#架构设计)
- [当前状态](#当前状态)
- [核心组件](#核心组件)
- [工具列表](#工具列表)
- [使用指南](#使用指南)
- [扩展新工具](#扩展新工具)
- [Hooks 系统](#hooks-系统)
- [沙箱和批准](#沙箱和批准)
- [动态工具](#动态工具)
- [最佳实践](#最佳实践)
- [故障排除](#故障排除)

## 概述

OpenJax 工具系统是一个模块化、可扩展的工具框架，支持动态注册、统一接口、丰富的输出格式和完整的执行流程。该系统参考了 Codex 的架构设计，提供了与 Codex 一致的体验。

### 核心特性

- **统一的接口抽象**：所有工具实现相同的 `ToolHandler` trait
- **动态注册和分发**：支持运行时注册和分发工具
- **丰富的输出格式**：支持 Text 和 Json 输出，包含成功标志
- **工具规范定义**：JSON Schema 定义工具的输入和输出
- **Hooks 系统**：支持工具执行前后的钩子
- **集中的沙箱和批准管理**：统一的沙箱策略和批准流程
- **支持多种工具类型**：Function、Mcp、Custom、LocalShell
- **动态工具支持**：支持运行时注册自定义工具

## 架构设计

### 分层架构

```
┌─────────────────────────────────────────────────────────┐
│                   用户调用工具                      │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│                  ToolRouter                        │
│  - 解析工具调用                                     │
│  - 创建 ToolInvocation                            │
│  - 调用 ToolOrchestrator                         │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│              ToolOrchestrator                      │
│  - 执行前钩子 (BeforeToolUse)                   │
│  - 检查是否需要批准                                 │
│  - 选择合适的沙箱                                     │
│  - 调用 ToolRegistry::dispatch()                    │
│  - 执行后钩子 (AfterToolUse)                     │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│              ToolRegistry                           │
│  - 存储所有工具处理器                                 │
│  - 分发工具调用到对应的处理器                          │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│            ToolHandler (具体实现)                   │
│  - GrepFilesHandler                                  │
│  - ReadFileHandler                                   │
│  - ListDirHandler                                    │
│  - ExecCommandHandler                                 │
│  - ApplyPatchHandler                                  │
│  - DynamicToolHandler (自定义)                        │
└─────────────────────────────────────────────────────────┘
```

### 模块结构

```
openjax-core/src/tools/
├── mod.rs                    # 模块导出
├── context.rs                # 核心类型定义
├── error.rs                 # 错误类型定义
├── registry.rs              # ToolHandler trait 和工具注册表
├── spec.rs                  # 工具规范定义
├── tool_builder.rs           # 工具注册构建器
├── events.rs                # Hooks 事件类型
├── hooks.rs                 # Hooks 执行器
├── sandboxing.rs            # 沙箱策略管理器
├── orchestrator.rs          # 工具编排器
├── dynamic.rs               # 动态工具管理器
├── router.rs                # 工具调用解析和配置类型
├── router_impl.rs           # 工具路由器实现
├── common.rs                # 通用工具函数
├── handlers/                # 工具处理器目录
│   ├── mod.rs             # 处理器模块导出
│   ├── grep_files.rs      # grep_files 工具处理器
│   ├── read_file.rs       # read_file 工具处理器
│   ├── list_dir.rs        # list_dir 工具处理器
│   ├── exec_command.rs    # exec_command 工具处理器
│   └── apply_patch.rs     # apply_patch 工具处理器
├── exec_command.rs           # exec_command 工具（原有）
├── apply_patch.rs            # apply_patch 工具（原有）
├── grep_files.rs             # grep_files 工具（原有）
├── read_file.rs              # read_file 工具（原有）
└── list_dir.rs               # list_dir 工具（原有）
```

## 当前状态

### 已完成的优化阶段

#### 第一阶段：引入核心抽象层 ✅
- ✅ 定义核心类型（context.rs）
- ✅ 定义 ToolHandler trait（registry.rs）
- ✅ 实现工具注册表（registry.rs）
- ✅ 迁移现有工具到 ToolHandler

#### 第二阶段：实现工具规范和输出格式 ✅
- ✅ 定义工具规范（spec.rs）
- ✅ 实现工具规范注册（tool_builder.rs）

#### 第三阶段：实现 Hooks 系统 ✅
- ✅ 定义 Hooks 类型（events.rs）
- ✅ 实现 Hooks 执行器（hooks.rs）

#### 第四阶段：集中管理沙箱和批准逻辑 ✅
- ✅ 定义沙箱策略（sandboxing.rs）
- ✅ 实现工具编排器（orchestrator.rs）

#### 第五阶段：支持动态注册和扩展性 ✅
- ✅ 实现动态工具支持（dynamic.rs）
- ✅ 更新工具路由器（router_impl.rs）

### 编译状态

✅ **编译成功**，无警告无错误：
```bash
cargo build -p openjax-core
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.17s
```

## 核心组件

### ToolHandler Trait

统一的工具处理器接口，所有工具都必须实现这个 trait。

```rust
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    /// 返回工具类型
    fn kind(&self) -> ToolKind;

    /// 检查是否匹配载荷类型
    fn matches_kind(&self, payload: &ToolPayload) -> bool;

    /// 返回 true 如果工具调用可能修改用户环境
    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool;

    /// 执行工具调用并返回输出
    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>;
}
```

### ToolRegistry

工具注册表，负责存储和分发工具处理器。

```rust
pub struct ToolRegistry {
    handlers: HashMap<String, Arc<dyn ToolHandler>>,
}

impl ToolRegistry {
    /// 注册工具处理器
    pub fn register(&mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>);

    /// 获取工具处理器
    pub fn handler(&self, name: &str) -> Option<Arc<dyn ToolHandler>>;

    /// 分发工具调用
    pub async fn dispatch(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>;
}
```

### ToolOrchestrator

工具编排器，负责管理工具执行的完整流程。

```rust
pub struct ToolOrchestrator {
    registry: Arc<ToolRegistry>,
    hook_executor: HookExecutor,
    sandbox_manager: SandboxManager,
}

impl ToolOrchestrator {
    /// 执行工具调用
    pub async fn run(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. 执行前钩子
        // 2. 检查是否需要批准
        // 3. 选择合适的沙箱
        // 4. 执行工具
        // 5. 执行后钩子
    }
}
```

### ToolRouter

工具路由器，负责解析工具调用并调用编排器。

```rust
pub struct ToolRouter {
    orchestrator: Arc<ToolOrchestrator>,
}

impl ToolRouter {
    pub async fn execute(
        &self,
        call: &ToolCall,
        cwd: &Path,
        config: ToolRuntimeConfig,
    ) -> Result<String>;
}
```

## 工具列表

### grep_files

使用 ripgrep 进行高性能搜索。

**功能**：
- 正则表达式搜索
- Glob 过滤（如 `*.rs`）
- 分页支持（limit 参数）
- 30 秒超时控制

**参数**：
- `pattern` (必需): 正则表达式模式
- `include` (可选): Glob 过滤模式
- `path` (可选): 搜索目录（默认：当前目录）
- `limit` (可选): 最大结果数（默认：100，最大：2000）

**输出**：
- 匹配的文件路径列表，每行一个
- 如果没有匹配，返回 "No matches found."

**示例**：
```bash
tool:grep_files pattern=fn main path=src include=*.rs limit=10
```

### read_file

读取文件内容，支持分页和缩进感知。

**功能**：
- 分页读取（offset 和 limit）
- 显示行号（L1: content 格式）
- 超长行截断（500 字符）
- 缩进感知模式

**参数**：
- `file_path` (必需): 文件路径
- `offset` (可选): 起始行号（1-indexed，默认：1）
- `limit` (可选): 最大行数（默认：2000）
- `mode` (可选): 读取模式（"slice" 或 "indentation"，默认："slice"）
- `indentation` (可选): 缩进感知选项（仅当 mode="indentation" 时使用）

**输出**：
- 文件内容，每行格式为 "L<line_number>: <content>"
- 支持缩进感知模式，返回上下文相关的行

**示例**：
```bash
tool:read_file file_path=src/lib.rs offset=1 limit=50
tool:read_file file_path=src/lib.rs mode=indentation indentation={"anchor_line": 100, "max_levels": 2}
```

### list_dir

列出目录内容，支持递归和分页。

**功能**：
- 递归列出（depth 参数）
- 分页支持（offset 和 limit）
- 文件类型标记（/ 目录、@ 符号链接、? 其他）
- 缩进显示层级结构

**参数**：
- `dir_path` (必需): 目录路径
- `offset` (可选): 起始条目号（1-indexed，默认：1）
- `limit` (可选): 最大条目数（默认：25）
- `depth` (可选): 最大递归深度（默认：2）

**输出**：
- 目录条目，带缩进和类型标记
- 格式："<indent><name><type_marker>"

**示例**：
```bash
tool:list_dir dir_path=src offset=1 limit=50 depth=3
```

### exec_command

执行 shell 命令，支持批准和沙箱模式。

**功能**：
- 执行 zsh 命令
- 支持批准策略
- 沙箱模式限制
- 返回退出码、stdout、stderr

**参数**：
- `cmd` (必需): 要执行的命令
- `require_escalated` (可选): 是否需要提升权限（默认：false）
- `timeout_ms` (可选): 超时时间（默认：30000ms）

**输出**：
- 命令执行结果
- 格式："exit_code=<code>\nstdout:\n<output>\nstderr:\n<error>"

**沙箱限制**：
- **WorkspaceWrite**: 允许的程序：pwd, ls, cat, rg, grep, find, head, tail, wc, sed, awk, echo, stat, uname, which, env, printf
- **DangerFullAccess**: 无限制

**示例**：
```bash
tool:exec_command cmd='cargo test' require_escalated=true timeout_ms=60000
```

### apply_patch

应用补丁到工作区，支持添加、删除、移动、重命名、更新文件。

**功能**：
- 解析补丁格式
- 支持多种操作（Add、Delete、Move、Rename、Update）
- 包含回滚机制

**参数**：
- `patch` (必需): 补丁文本

**补丁格式**：
```
*** Begin Patch
*** Add File: new_file.rs
+// new file content
*** End Patch
```

**示例**：
```bash
tool:apply_patch patch='*** Begin Patch\n*** Add File: new.rs\n+// content\n*** End Patch'
```

## 使用指南

### 基本使用

1. **解析工具调用**：
   ```rust
   let call = parse_tool_call("tool:grep_files pattern=fn main")?;
   ```

2. **创建工具路由器**：
   ```rust
   let router = ToolRouter::new();
   ```

3. **执行工具**：
   ```rust
   let result = router.execute(&call, &cwd, config).await?;
   ```

### 在 Agent 中使用

```rust
use openjax_core::tools::{ToolRouter, parse_tool_call};

pub async fn execute_tool_turn(&self, input: &str) -> Result<String> {
    let cwd = std::env::current_dir()?;
    let config = ToolRuntimeConfig {
        approval_policy: ApprovalPolicy::AlwaysAsk,
        sandbox_mode: SandboxMode::WorkspaceWrite,
    };

    if let Some(call) = parse_tool_call(input) {
        let router = ToolRouter::new();
        router.execute(&call, &cwd, config).await
    } else {
        Err(anyhow!("invalid tool call format"))
    }
}
```

## 扩展新工具

### 步骤 1：创建工具处理器

在 `openjax-core/src/tools/handlers/` 目录下创建新文件。

```rust
use async_trait::async_trait;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload, FunctionCallOutputBody};
use crate::tools::registry::{ToolHandler, ToolKind};
use crate::tools::error::FunctionCallError;

pub struct MyToolHandler;

#[async_trait]
impl ToolHandler for MyToolHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "my_tool handler received unsupported payload".to_string(),
                ));
            }
        };

        // 解析参数
        let args: MyToolArgs = serde_json::from_str(&arguments)
            .map_err(|e| FunctionCallError::Internal(format!("failed to parse arguments: {}", e)))?;

        // 执行工具逻辑
        let result = self.execute_my_tool(&args).await?;

        // 返回结果
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(result),
            success: Some(true),
        })
    }
}
```

### 步骤 2：定义参数结构

使用 serde 定义参数结构。

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct MyToolArgs {
    #[serde(default)]
    param1: String,
    param2: Option<i32>,
}
```

### 步骤 3：创建工具规范

在 `openjax-core/src/tools/spec.rs` 中添加工具规范。

```rust
pub fn create_my_tool_spec() -> ToolSpec {
    ToolSpec {
        name: "my_tool".to_string(),
        description: "Description of my tool".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "param1": {
                    "type": "string",
                    "description": "Description of param1"
                },
                "param2": {
                    "type": "number",
                    "description": "Description of param2",
                    "default": 10
                }
            },
            "required": ["param1"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "Description of output"
        })),
    }
}
```

### 步骤 4：注册工具

在 `openjax-core/src/tools/tool_builder.rs` 中注册工具。

```rust
pub fn build_default_tool_registry() -> (ToolRegistry, Vec<ToolSpec>) {
    let mut builder = ToolRegistryBuilder::new();

    // 注册 my_tool
    let my_handler = Arc::new(MyToolHandler);
    builder.push_spec(create_my_tool_spec(), true);
    builder.register_handler("my_tool", my_handler);

    builder.build()
}
```

### 步骤 5：导出工具

在 `openjax-core/src/tools/handlers/mod.rs` 中导出工具。

```rust
pub mod my_tool;
pub use my_tool::MyToolHandler;
```

在 `openjax-core/src/tools/mod.rs` 中导出工具。

```rust
pub use handlers::MyToolHandler;
```

### 步骤 6：更新文档

更新本文档，添加新工具的说明。

## Hooks 系统

### HookEvent 类型

```rust
pub enum HookEvent {
    BeforeToolUse(BeforeToolUse),
    AfterToolUse(AfterToolUse),
}
```

### BeforeToolUse

工具使用前钩子，包含以下信息：
- `tool_name`: 工具名称
- `call_id`: 调用 ID
- `tool_input`: 工具输入

### AfterToolUse

工具使用后钩子，包含以下信息：
- `tool_name`: 工具名称
- `call_id`: 调用 ID
- `tool_input`: 工具输入
- `executed`: 是否执行成功
- `success`: 是否成功
- `duration_ms`: 执行时长（毫秒）
- `mutating`: 是否为变异操作
- `sandbox`: 沙箱类型
- `sandbox_policy`: 沙箱策略
- `output_preview`: 输出预览

### 使用 Hooks

```rust
use openjax_core::tools::{HookExecutor, HookEvent, BeforeToolUse};

let hook_executor = HookExecutor::new();

// 执行前钩子
hook_executor.execute(&HookEvent::BeforeToolUse(BeforeToolUse {
    tool_name: "grep_files".to_string(),
    call_id: "12345".to_string(),
    tool_input: "pattern=fn main".to_string(),
}));

// 执行工具...

// 执行后钩子
hook_executor.execute(&HookEvent::AfterToolUse(AfterToolUse {
    tool_name: "grep_files".to_string(),
    call_id: "12345".to_string(),
    tool_input: "pattern=fn main".to_string(),
    executed: true,
    success: true,
    duration_ms: 150,
    mutating: false,
    sandbox: "workspace_write".to_string(),
    sandbox_policy: "workspace_write".to_string(),
    output_preview: Some("file1.rs\nfile2.rs".to_string()),
}));
```

## 沙箱和批准

### SandboxPolicy

```rust
pub enum SandboxPolicy {
    None,
    ReadOnly,
    Write,
    DangerFullAccess,
}
```

### 环境变量

设置沙箱模式：
```bash
export OPENJAX_SANDBOX_MODE=workspace_write  # 默认
export OPENJAX_SANDBOX_MODE=danger_full_access  # 无限制
export OPENJAX_SANDBOX_MODE=read_only  # 只读
```

### 批准策略

```rust
pub enum ApprovalPolicy {
    AlwaysAsk,    // 总是询问
    OnRequest,    // 仅在请求时询问
    Never,         // 从不询问
}
```

设置批准策略：
```bash
export OPENJAX_APPROVAL_POLICY=always_ask  # 默认
export OPENJAX_APPROVAL_POLICY=on_request
export OPENJAX_APPROVAL_POLICY=never
```

### 变异操作

以下工具被认为是变异操作：
- `exec_command`: 执行命令可能修改文件系统
- `apply_patch`: 应用补丁会修改文件

以下工具被认为是非变异操作：
- `grep_files`: 只读操作
- `read_file`: 只读操作
- `list_dir`: 只读操作

## 动态工具

### DynamicToolManager

动态工具管理器，支持运行时注册自定义工具。

```rust
use openjax_core::tools::DynamicToolManager;

let mut dynamic_manager = DynamicToolManager::new();

// 注册动态工具
dynamic_manager.register(name, handler);

// 列出所有工具
let tools = dynamic_manager.list_tools();

// 移除工具
dynamic_manager.unregister(name);
```

### 使用场景

1. **插件系统**：允许用户编写自己的工具
2. **运行时扩展**：不需要重新编译即可添加新工具
3. **A/B 测试**：可以动态切换不同的工具实现
4. **多租户支持**：不同租户可以使用不同的工具集

## 最佳实践

### 1. 错误处理

始终使用 `FunctionCallError` 返回错误：

```rust
// ✅ 好的做法
Err(FunctionCallError::Internal("failed to read file".to_string()))

// ❌ 不好的做法
Err(anyhow!("failed to read file"))
```

### 2. 参数验证

在工具开始时验证参数：

```rust
if args.pattern.is_empty() {
    return Err(FunctionCallError::RespondToModel(
        "pattern must not be empty".to_string(),
    ));
}
```

### 3. 路径验证

使用提供的路径验证函数：

```rust
use openjax_core::tools::resolve_workspace_path;

let path = resolve_workspace_path(&cwd, &rel_path)?;
```

### 4. 日志记录

使用 `tracing` 库进行结构化日志：

```rust
use tracing::{debug, info, warn, error};

debug!(tool_name = %name, "tool started");
info!(tool_name = %name, "tool completed");
warn!(tool_name = %name, "tool warning");
error!(tool_name = %name, error = %err, "tool failed");
```

### 5. 异步操作

对于 I/O 密集型操作，使用异步：

```rust
use tokio::fs;

let content = tokio::fs::read_to_string(path).await?;
```

### 6. 超时控制

为长时间运行的操作设置超时：

```rust
use tokio::time::{timeout, Duration};

let result = timeout(Duration::from_secs(30), operation).await?;
```

## 故障排除

### 常见问题

#### 1. 工具未找到

**错误**：`ToolNotFound("unknown tool: xxx")`

**原因**：工具未注册或名称拼写错误

**解决**：
- 检查工具名称是否正确
- 确认工具已在 `build_default_tool_registry()` 中注册

#### 2. 参数解析失败

**错误**：`Internal("failed to parse arguments: xxx")`

**原因**：参数格式不正确或类型不匹配

**解决**：
- 检查参数格式是否符合 JSON Schema
- 确认参数类型正确

#### 3. 路径验证失败

**错误**：`Internal("path escapes workspace: xxx")`

**原因**：路径超出工作区或包含父目录遍历

**解决**：
- 使用相对路径
- 避免使用 `..` 或绝对路径

#### 4. 批准被拒绝

**错误**：`ApprovalRejected("command rejected by user")`

**原因**：用户拒绝了工具执行

**解决**：
- 检查批准策略设置
- 确认用户输入 `y` 同意执行

#### 5. 超时

**错误**：`Internal("operation timed out after xxx ms")`

**原因**：操作执行时间超过超时限制

**解决**：
- 增加超时时间
- 优化操作性能

### 调试技巧

#### 1. 启用调试日志

```bash
export RUST_LOG=openjax_core=debug
```

#### 2. 检查工具注册

```rust
let router = ToolRouter::new();
// 检查工具是否已注册
```

#### 3. 测试工具调用

```bash
# 直接测试工具调用
tool:grep_files pattern=fn main
```

#### 4. 查看工具规范

```rust
use openjax_core::tools::build_all_specs;

let specs = build_all_specs();
for spec in specs {
    println!("Tool: {}", spec.name);
    println!("Description: {}", spec.description);
    println!("Input Schema: {}", serde_json::to_string_pretty(&spec.input_schema).unwrap());
}
```

## 参考资源

### 内部文档

- [context.rs](context.rs) - 核心类型定义
- [registry.rs](registry.rs) - ToolHandler trait 和工具注册表
- [spec.rs](spec.rs) - 工具规范定义
- [tool_builder.rs](tool_builder.rs) - 工具注册构建器
- [events.rs](events.rs) - Hooks 事件类型
- [hooks.rs](hooks.rs) - Hooks 执行器
- [sandboxing.rs](sandboxing.rs) - 沙箱策略管理器
- [orchestrator.rs](orchestrator.rs) - 工具编排器
- [dynamic.rs](dynamic.rs) - 动态工具管理器

### 工具处理器

- [grep_files.rs](handlers/grep_files.rs) - grep_files 工具处理器
- [read_file.rs](handlers/read_file.rs) - read_file 工具处理器
- [list_dir.rs](handlers/list_dir.rs) - list_dir 工具处理器
- [exec_command.rs](handlers/exec_command.rs) - exec_command 工具处理器
- [apply_patch.rs](handlers/apply_patch.rs) - apply_patch 工具处理器

### 外部参考

- [Codex 工具系统](https://github.com/codex-ai/codex) - 参考 Codex 的实现
- [Rust async_trait](https://docs.rs/async-trait/) - 异步 trait 文档
- [Rust serde](https://serde.rs/) - 序列化框架文档
- [Rust tokio](https://tokio.rs/) - 异步运行时文档

## 后续扩展方向

### 1. MCP 工具支持

集成 Model Context Protocol，支持外部 MCP 服务器工具。

### 2. 自定义工具

允许用户定义自己的工具，支持插件系统。

### 3. 并行执行

支持工具并行调用，提高性能。

### 4. 工具链

支持工具之间的依赖关系和链式调用。

### 5. 工具缓存

缓存工具执行结果，减少重复计算。

### 6. 工具监控

集成监控和告警系统，追踪工具使用情况。

### 7. 性能优化

优化工具执行性能，减少延迟。

### 8. 类型安全增强

使用更强的类型系统，减少运行时错误。

## 总结

OpenJax 工具系统已经完成了全面的优化，达到了与 Codex 一致的架构水平。系统具备：

- ✅ 统一的接口抽象（ToolHandler trait）
- ✅ 动态注册和分发能力（ToolRegistry）
- ✅ 丰富的输出格式（Text、Json）
- ✅ 工具规范定义（JSON Schema）
- ✅ Hooks 系统（BeforeToolUse、AfterToolUse）
- ✅ 集中的沙箱和批准管理（ToolOrchestrator）
- ✅ 支持多种工具类型（Function、Mcp、Custom、LocalShell）
- ✅ 动态工具支持（DynamicToolManager）
- ✅ 清晰的模块化架构

这个系统为 OpenJax 的后续扩展打下了坚实的基础，支持快速添加新工具、集成外部服务和实现复杂的工具链。
