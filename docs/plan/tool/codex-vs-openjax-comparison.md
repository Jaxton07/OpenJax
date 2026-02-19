# Codex vs OpenJax 工具系统对比

本文档详细对比了 Codex 和 OpenJax 的工具系统实现差异。

## 架构对比

### Codex 工具系统架构

Codex 采用了更复杂和模块化的架构：

```
codex-rs/core/src/tools/
├── mod.rs                    # 模块导出
├── router.rs                 # 路由 API 响应到工具调用
├── registry.rs               # 工具注册表和分发
├── orchestrator.rs           # 沙箱选择、批准流程、重试逻辑
├── spec.rs                   # 工具规范定义和注册
├── context.rs                # 核心类型定义
├── events.rs                # 工具事件定义
├── sandboxing.rs            # 沙箱策略
├── parallel.rs              # 并行执行支持
├── runtimes/                # 运行时实现
│   ├── shell.rs
│   ├── unified_exec.rs
│   └── apply_patch.rs
└── handlers/                # 工具处理器实现
    ├── grep_files.rs
    ├── read_file.rs
    ├── list_dir.rs
    ├── apply_patch.rs
    ├── shell.rs
    ├── unified_exec.rs
    └── ... (更多工具)
```

**核心组件**：
- `ToolHandler` trait：统一的工具处理器接口
- `ToolRegistry`：工具注册表，负责分发
- `ToolOrchestrator`：沙箱选择、批准流程、重试逻辑
- `ToolRouter`：路由 API 响应到工具调用
- `ToolPayload`：工具调用载荷（支持多种类型）
- `ToolOutput`：工具输出（包含成功标志）
- Hooks 系统：BeforeToolUse、AfterToolUse

### OpenJax 工具系统架构

OpenJax 采用了更简单的架构：

```
openjax-core/src/tools/
├── mod.rs                    # 模块导出
├── common.rs                 # 通用工具函数
├── router.rs                 # 工具调用解析和配置类型
├── router_impl.rs            # 工具路由器实现
├── grep_files.rs            # grep_files 工具
├── read_file.rs             # read_file 工具
├── list_dir.rs              # list_dir 工具
├── shell.rs          # shell 工具
└── apply_patch.rs           # apply_patch 工具
```

**核心组件**：
- 独立的工具函数：`grep_files()`, `read_file()`, `list_dir()`, `shell()`, `apply_patch()`
- `ToolRouter`：简单的路由器，分发到各个工具函数
- `ToolCall`：工具调用结构体（name + args）
- `ToolRuntimeConfig`：运行时配置（approval_policy + sandbox_mode）

## 主要差异

### 1. 工具接口抽象

#### Codex
```rust
#[async_trait]
pub trait ToolHandler: Send + Sync {
    fn kind(&self) -> ToolKind;
    fn matches_kind(&self, payload: &ToolPayload) -> bool;
    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool;
    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>;
}
```

**优点**：
- 统一的接口，所有工具实现相同的 trait
- 易于添加新工具（只需实现 trait）
- 支持多种工具类型（Function、Mcp、Custom、LocalShell）

#### OpenJax
```rust
// 每个工具是独立的函数
pub async fn grep_files(call: &ToolCall, cwd: &Path) -> Result<String>
pub async fn read_file(call: &ToolCall, cwd: &Path) -> Result<String>
pub async fn list_dir(call: &ToolCall, cwd: &Path) -> Result<String>
// ...
```

**优点**：
- 简单直接，易于理解
- 每个工具独立，职责清晰

**缺点**：
- 没有统一的接口抽象
- 难以扩展（添加新工具需要修改多处代码）

### 2. 工具注册和分发

#### Codex
```rust
// 工具注册
let handler = Arc::new(GrepFilesHandler);
builder.push_spec(create_grep_files_tool(...), true);
builder.register_handler("grep_files", handler);

// 工具分发
pub async fn dispatch(&self, invocation: ToolInvocation) -> Result<ResponseInputItem, FunctionCallError> {
    let handler = self.handler(tool_name.as_ref())?;
    // 执行前钩子
    // 调用处理器
    // 执行后钩子
}
```

**优点**：
- 动态注册，支持运行时添加工具
- 集中管理，易于维护
- 支持 hooks 系统（执行前/后钩子）

#### OpenJax
```rust
// 工具分发（硬编码）
pub async fn execute(&self, call: &ToolCall, cwd: &Path, config: ToolRuntimeConfig) -> Result<String> {
    match call.name.as_str() {
        "read_file" => read_file(call, cwd).await,
        "list_dir" => list_dir(call, cwd).await,
        "grep_files" => grep_files(call, cwd).await,
        "shell" => shell(call, cwd, config).await,
        "apply_patch" => apply_patch_tool(call, cwd).await,
        _ => Err(anyhow!("unknown tool: {}", call.name))
    }
}
```

**优点**：
- 简单直接，易于理解
- 编译时检查，类型安全

**缺点**：
- 硬编码，添加新工具需要修改路由器
- 不支持动态注册

### 3. 参数解析

#### Codex
```rust
// 从 JSON 字符串解析参数
let arguments = match payload {
    ToolPayload::Function { arguments } => arguments,
    _ => return Err(...),
};
let args: GrepFilesArgs = parse_arguments(&arguments)?;
```

**优点**：
- 统一的参数解析逻辑
- 支持复杂的参数类型（嵌套、可选等）

#### OpenJax
```rust
// 从 HashMap 解析参数
let args: GrepFilesArgs = parse_tool_args(&call.args)?;
```

**优点**：
- 简单直接，易于使用

**缺点**：
- 参数类型受限（只支持 String）
- 不支持复杂的参数结构

### 4. 输出格式

#### Codex
```rust
pub enum ToolOutput {
    Function { body: FunctionCallOutputBody, success: Option<bool> },
    Mcp { result: Result<CallToolResult, String> },
}

pub enum FunctionCallOutputBody {
    Text(String),
    Json(serde_json::Value),
}
```

**优点**：
- 丰富的输出格式（支持 Text 和 Json）
- 包含成功标志，便于模型理解
- 支持多种输出类型（Function、Mcp）

#### OpenJax
```rust
// 简单的字符串输出
pub async fn grep_files(call: &ToolCall, cwd: &Path) -> Result<String>
```

**优点**：
- 简单直接，易于理解

**缺点**：
- 输出格式单一（只支持字符串）
- 没有成功标志，模型难以判断执行结果

### 5. 沙箱和批准机制

#### Codex
```rust
// ToolOrchestrator 处理沙箱选择和批准流程
pub async fn run<Rq, Out, T>(&self, request: Rq) -> Result<Out, T> {
    // 1. 检查是否需要批准
    // 2. 选择合适的沙箱
    // 3. 执行工具
    // 4. 如果沙箱拒绝，可以尝试提升权限
}
```

**优点**：
- 集中管理沙箱策略
- 支持沙箱升级（从 read-only 到 write）
- 统一的批准流程

#### OpenJax
```rust
// 在工具函数内部处理批准和沙箱
pub async fn shell(call: &ToolCall, cwd: &Path, config: ToolRuntimeConfig) -> Result<String> {
    if should_prompt_approval(config.approval_policy, require_escalated)
        && !ask_for_approval(&command)?
    {
        return Err(anyhow!("command rejected by user"));
    }

    if let SandboxMode::WorkspaceWrite = config.sandbox_mode {
        deny_if_blocked_in_workspace_write(&command, cwd)?;
    }

    // 执行命令
}
```

**优点**：
- 简单直接，易于理解

**缺点**：
- 沙箱逻辑分散在各个工具中
- 不支持沙箱升级
- 批准逻辑重复

### 6. Hooks 系统

#### Codex
```rust
// 执行前钩子
BeforeToolUse {
    tool_name,
    call_id,
    tool_input,
}

// 执行后钩子
AfterToolUse {
    tool_name,
    call_id,
    tool_input,
    executed,
    success,
    duration_ms,
    mutating,
    sandbox,
    sandbox_policy,
    output_preview,
}
```

**优点**：
- 支持执行前/后钩子
- 丰富的上下文信息
- 便于监控和调试

#### OpenJax
- **没有 hooks 系统**

**缺点**：
- 无法在工具执行前后插入自定义逻辑
- 难以监控和调试工具执行

### 7. 工具类型支持

#### Codex
```rust
pub enum ToolPayload {
    Function { arguments: String },     // 标准函数调用
    Custom { input: String },            // 自由形式/自定义工具
    LocalShell { params: ShellToolCallParams },  // 本地 shell 调用
    Mcp { server, tool, raw_arguments }, // MCP 服务器工具
}
```

**优点**：
- 支持多种工具类型
- 易于扩展新的工具类型
- 支持 MCP（Model Context Protocol）工具

#### OpenJax
```rust
pub struct ToolCall {
    pub name: String,
    pub args: HashMap<String, String>,
}
```

**优点**：
- 简单直接，易于理解

**缺点**：
- 只支持一种工具类型（函数调用）
- 不支持 MCP、自定义工具等

### 8. 工具规范和 API 暴露

#### Codex
```rust
// 工具规范定义
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
}

// 注册工具
builder.push_spec(create_grep_files_tool(...), true);
```

**优点**：
- 自动生成 API 文档
- 支持输入/输出 schema 验证
- 便于模型理解工具能力

#### OpenJax
- **没有工具规范定义**

**缺点**：
- 无法自动生成 API 文档
- 模型难以理解工具能力
- 没有输入/输出验证

## 总结

### Codex 优势
1. **模块化架构**：清晰的分层，职责分离
2. **统一的接口**：`ToolHandler` trait 提供一致的抽象
3. **动态注册**：支持运行时添加工具
4. **丰富的输出**：支持多种输出格式和成功标志
5. **Hooks 系统**：支持执行前/后钩子
6. **多种工具类型**：支持 Function、Mcp、Custom、LocalShell
7. **工具规范**：自动生成 API 文档和 schema 验证
8. **集中管理**：沙箱、批准、重试逻辑集中管理

### OpenJax 优势
1. **简单直接**：易于理解和维护
2. **编译时检查**：类型安全，编译时发现错误
3. **独立工具**：每个工具独立，职责清晰

### 建议改进方向

如果要让 OpenJax 的工具系统更接近 Codex 的水平，建议：

1. **引入 ToolHandler trait**：提供统一的工具接口
2. **实现工具注册表**：支持动态注册和分发
3. **添加工具规范**：定义工具的输入/输出 schema
4. **实现 Hooks 系统**：支持执行前/后钩子
5. **丰富输出格式**：支持多种输出类型和成功标志
6. **支持多种工具类型**：支持 MCP、自定义工具等
7. **集中管理沙箱**：将沙箱和批准逻辑集中到 orchestrator

## 当前状态

### 已实现的工具（OpenJax）
- ✅ `grep_files`：使用 ripgrep 进行高性能搜索
- ✅ `read_file`：文件读取（支持分页和缩进感知）
- ✅ `list_dir`：目录列出（支持递归和分页）
- ✅ `shell`：Shell 命令执行（支持批准和沙箱）
- ✅ `apply_patch`：补丁解析和应用

### 与 Codex 的差异
- ❌ 没有 `ToolHandler` trait
- ❌ 没有工具注册表
- ❌ 没有工具规范定义
- ❌ 没有 Hooks 系统
- ❌ 输出格式单一（只支持字符串）
- ❌ 不支持 MCP 工具
- ❌ 沙箱和批准逻辑分散
- ❌ 不支持动态注册

### 核心实现差异
虽然 OpenJax 的工具实现（grep_files、read_file、list_dir）直接复用了 Codex 的核心逻辑，但：
- **架构层面**：OpenJax 缺少 Codex 的抽象层和模块化设计
- **扩展性**：OpenJax 难以扩展新工具类型和功能
- **可维护性**：OpenJax 的沙箱和批准逻辑分散在各个工具中
- **可观测性**：OpenJax 缺少 hooks 系统和丰富的上下文信息
