# 核心组件

本文档详细介绍了 OpenJax 工具系统的核心组件。

## ToolHandler Trait

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

### 方法说明

- `kind()`: 返回工具类型（Function、Mcp、Custom、LocalShell）
- `matches_kind()`: 检查载荷类型是否匹配
- `is_mutating()`: 判断工具是否会修改用户环境
- `handle()`: 执行工具并返回结果

## ToolRegistry

工具注册表，负责存储和分发工具处理器。

```rust
pub struct ToolRegistry {
    handlers: RwLock<HashMap<String, Arc<dyn ToolHandler>>>,
}

impl ToolRegistry {
    /// 注册工具处理器
    pub fn register(&self, name: impl Into<String>, handler: Arc<dyn ToolHandler>);

    /// 获取工具处理器
    pub fn handler(&self, name: &str) -> Option<Arc<dyn ToolHandler>>;

    /// 分发工具调用
    pub async fn dispatch(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>;
}
```

### 方法说明

- `register()`: 注册新的工具处理器
- `handler()`: 获取已注册的工具处理器
- `dispatch()`: 分发工具调用到对应的处理器

## ToolOrchestrator

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

### 执行流程

1. **执行前钩子**：调用 `BeforeToolUse` 钩子
2. **检查批准**：根据批准策略决定是否需要用户确认
3. **选择沙箱**：根据沙箱策略选择合适的执行环境
4. **执行工具**：通过 `ToolRegistry::dispatch()` 执行工具
5. **执行后钩子**：调用 `AfterToolUse` 钩子

## ToolRouter

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

### 方法说明

- `execute()`: 解析工具调用并执行，返回结果字符串

## ToolInvocation

工具调用对象，包含调用的所有信息。

```rust
pub struct ToolInvocation {
    pub name: String,
    pub payload: ToolPayload,
    pub cwd: PathBuf,
    pub config: ToolRuntimeConfig,
}
```

### 字段说明

- `name`: 工具名称
- `payload`: 工具载荷（参数）
- `cwd`: 当前工作目录
- `config`: 运行时配置

## ToolOutput

工具输出对象，包含执行结果。

```rust
pub enum ToolOutput {
    Function {
        body: FunctionCallOutputBody,
        success: Option<bool>,
    },
    Mcp {
        body: McpOutputBody,
        success: Option<bool>,
    },
    Custom {
        body: CustomOutputBody,
        success: Option<bool>,
    },
    LocalShell {
        body: LocalShellOutputBody,
        success: Option<bool>,
    },
}
```

### 变体说明

- `Function`: 函数类型工具输出
- `Mcp`: MCP 工具输出
- `Custom`: 自定义工具输出
- `LocalShell`: 本地 shell 命令输出

## ToolRuntimeConfig

工具运行时配置。

```rust
pub struct ToolRuntimeConfig {
    pub approval_policy: ApprovalPolicy,
    pub sandbox_mode: SandboxMode,
    pub shell_type: ShellType,
    pub tools_config: ToolsConfig,
    pub prevent_shell_skill_trigger: bool,
}
```

### 字段说明

- `approval_policy`: 批准策略（AlwaysAsk、OnRequest、Never）
- `sandbox_mode`: 沙箱模式（WorkspaceWrite、DangerFullAccess）
- `shell_type`: shell 执行类型（`bash/zsh/sh/pwsh`）
- `tools_config`: 工具注册与规格配置（例如是否禁用 `shell`、`apply_patch` 规格类型）
- `prevent_shell_skill_trigger`: 启用后会阻止将 `/skill-name` 这类字符串当作 shell 命令执行

> 兼容性说明：本次更新不引入公开 API 破坏性改动，主要修复配置项生效一致性与并发安全问题。

## HookExecutor

Hooks 执行器，负责执行工具前后的钩子。

```rust
pub struct HookExecutor {
    before_hooks: Vec<Arc<dyn BeforeToolUseHook>>,
    after_hooks: Vec<Arc<dyn AfterToolUseHook>>,
}

impl HookExecutor {
    /// 执行前钩子
    pub async fn execute_before(&self, event: &BeforeToolUse);

    /// 执行后钩子
    pub async fn execute_after(&self, event: &AfterToolUse);
}
```

### 方法说明

- `execute_before()`: 执行所有 BeforeToolUse 钩子
- `execute_after()`: 执行所有 AfterToolUse 钩子

## SandboxManager

沙箱管理器，负责管理沙箱策略。

```rust
pub struct SandboxManager {
    policy: SandboxPolicy,
}

impl SandboxManager {
    /// 获取沙箱策略
    pub fn policy(&self) -> &SandboxPolicy;

    /// 检查命令是否允许执行
    pub fn is_command_allowed(&self, cmd: &str) -> bool;
}
```

### 方法说明

- `policy()`: 获取当前沙箱策略
- `is_command_allowed()`: 检查命令是否允许在当前沙箱中执行

## DynamicToolManager

动态工具管理器，支持运行时注册自定义工具。

```rust
pub struct DynamicToolManager {
    tools: Mutex<HashMap<String, Arc<dyn ToolHandler>>>,
}

impl DynamicToolManager {
    /// 注册动态工具
    pub fn register(&self, name: String, handler: Arc<dyn ToolHandler>);

    /// 列出所有工具
    pub fn list_tools(&self) -> Vec<String>;

    /// 移除工具
    pub fn unregister(&self, name: &str);
}
```

### 方法说明

- `register()`: 注册动态工具
- `list_tools()`: 列出所有已注册的工具
- `unregister()`: 移除已注册的工具

## 相关文档

- [架构设计](architecture.md) - 了解整体架构
- [Hooks 系统](hooks-system.md) - 深入了解 Hooks
- [沙箱和批准](sandbox-and-approval.md) - 了解沙箱和批准机制
