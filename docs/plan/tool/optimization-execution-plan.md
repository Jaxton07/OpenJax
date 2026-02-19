# OpenJax 工具系统优化执行计划

本文档详细规划了将 OpenJax 工具系统优化到与 Codex 一致水平的执行步骤。

## 目标

将 OpenJax 的工具系统从当前的简单架构升级到与 Codex 一致的模块化、可扩展架构，为后续扩展应用打下坚实基础。

## 总体策略

采用**渐进式重构**策略，保持现有功能不变的同时，逐步引入 Codex 的架构特性：

1. **第一阶段**：引入核心抽象层（ToolHandler trait、工具注册表）
2. **第二阶段**：实现工具规范和输出格式
3. **第三阶段**：实现 Hooks 系统
4. **第四阶段**：集中管理沙箱和批准逻辑
5. **第五阶段**：支持动态注册和扩展性

## 详细执行步骤

### 第一阶段：引入核心抽象层

#### 步骤 1.1：定义核心类型

**文件**：`openjax-core/src/tools/context.rs`（新建）

**目标**：定义工具系统的核心类型

**内容**：
```rust
use std::collections::HashMap;
use std::path::PathBuf;

/// 工具调用载荷
#[derive(Debug, Clone)]
pub enum ToolPayload {
    Function { arguments: String },     // 标准函数调用
    Custom { input: String },            // 自由形式/自定义工具
    LocalShell { params: ShellToolCallParams },  // 本地 shell 调用
    Mcp { server: String, tool: String, raw_arguments: String }, // MCP 服务器工具
}

/// 工具输出
#[derive(Debug, Clone)]
pub enum ToolOutput {
    Function { body: FunctionCallOutputBody, success: Option<bool> },
    Mcp { result: Result<McpToolResult, String> },
}

/// 函数调用输出体
#[derive(Debug, Clone)]
pub enum FunctionCallOutputBody {
    Text(String),
    Json(serde_json::Value),
}

/// 工具调用上下文
#[derive(Debug, Clone)]
pub struct ToolInvocation {
    pub tool_name: String,
    pub call_id: String,
    pub payload: ToolPayload,
    pub turn: ToolTurnContext,
}

/// 工具轮次上下文
#[derive(Debug, Clone)]
pub struct ToolTurnContext {
    pub cwd: PathBuf,
    pub sandbox_policy: SandboxPolicy,
    pub windows_sandbox_level: Option<String>,
}

/// MCP 工具结果
#[derive(Debug, Clone, serde::Deserialize)]
pub struct McpToolResult {
    pub content: Option<serde_json::Value>,
    pub is_error: Option<bool>,
}
```

**验证**：编译通过，类型定义正确

---

#### 步骤 1.2：定义 ToolHandler trait

**文件**：`openjax-core/src/tools/registry.rs`（新建）

**目标**：定义统一的工具处理器接口

**内容**：
```rust
use async_trait::async_trait;
use crate::tools::context::{ToolInvocation, ToolOutput};

/// 工具类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Function,
    Mcp,
}

/// 工具处理器 trait
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// 返回工具类型
    fn kind(&self) -> ToolKind;

    /// 检查是否匹配载荷类型
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            (self.kind(), payload),
            (ToolKind::Function, ToolPayload::Function { .. })
                | (ToolKind::Mcp, ToolPayload::Mcp { .. })
        )
    }

    /// 返回 true 如果工具调用可能修改用户环境
    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        false
    }

    /// 执行工具调用并返回输出
    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>;
}
```

**验证**：编译通过，trait 定义正确

---

#### 步骤 1.3：实现工具注册表

**文件**：`openjax-core/src/tools/registry.rs`（继续）

**目标**：实现动态工具注册和分发

**内容**：
```rust
use std::collections::HashMap;
use std::sync::Arc;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload};

/// 工具注册表
pub struct ToolRegistry {
    handlers: HashMap<String, Arc<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// 注册工具处理器
    pub fn register(&mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) {
        let name = name.into();
        if self.handlers.contains_key(&name) {
            tracing::warn!("overwriting handler for tool {}", name);
        }
        self.handlers.insert(name, handler);
    }

    /// 获取工具处理器
    pub fn handler(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.handlers.get(name).cloned()
    }

    /// 分发工具调用
    pub async fn dispatch(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let tool_name = invocation.tool_name.clone();
        let handler = self.handler(&tool_name)
            .ok_or_else(|| FunctionCallError::ToolNotFound(tool_name))?;

        if !handler.matches_kind(&invocation.payload) {
            return Err(FunctionCallError::InvalidPayload(
                format!("tool {} does not support payload type", tool_name)
            ));
        }

        handler.handle(invocation).await
    }
}
```

**验证**：编译通过，注册表功能正常

---

#### 步骤 1.4：迁移现有工具到 ToolHandler

**文件**：
- `openjax-core/src/tools/handlers/`（新建目录）
- `openjax-core/src/tools/handlers/grep_files.rs`（新建）
- `openjax-core/src/tools/handlers/read_file.rs`（新建）
- `openjax-core/src/tools/handlers/list_dir.rs`（新建）
- `openjax-core/src/tools/handlers/shell.rs`（新建）
- `openjax-core/src/tools/handlers/apply_patch.rs`（新建）

**目标**：将现有工具函数迁移到 ToolHandler 实现

**内容**（以 grep_files 为例）：
```rust
use async_trait::async_trait;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload, FunctionCallOutputBody};
use crate::tools::registry::{ToolHandler, ToolKind};
use crate::function_tool::FunctionCallError;

pub struct GrepFilesHandler;

#[async_trait]
impl ToolHandler for GrepFilesHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "grep_files handler received unsupported payload".to_string(),
                ));
            }
        };

        // 复用现有的 grep_files 逻辑
        let args: GrepFilesArgs = parse_tool_args_from_json(&arguments)?;
        let search_results = run_rg_search(...).await?;

        if search_results.is_empty() {
            Ok(ToolOutput::Function {
                body: FunctionCallOutputBody::Text("No matches found.".to_string()),
                success: Some(false),
            })
        } else {
            Ok(ToolOutput::Function {
                body: FunctionCallOutputBody::Text(search_results.join("\n")),
                success: Some(true),
            })
        }
    }
}
```

**验证**：所有工具编译通过，功能正常

---

### 第二阶段：实现工具规范和输出格式

#### 步骤 2.1：定义工具规范

**文件**：`openjax-core/src/tools/spec.rs`（新建）

**目标**：定义工具规范和 schema

**内容**：
```rust
use serde_json::Value;

/// 工具规范
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
}

/// 创建 grep_files 工具规范
pub fn create_grep_files_spec() -> ToolSpec {
    ToolSpec {
        name: "grep_files".to_string(),
        description: "Search files using ripgrep with regex pattern support".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., *.rs)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results (default: 100, max: 2000)"
                }
            },
            "required": ["pattern"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "List of matching file paths"
        })),
    }
}
```

**验证**：编译通过，规范定义正确

---

#### 步骤 2.2：实现工具规范注册

**文件**：`openjax-core/src/tools/spec.rs`（继续）

**目标**：注册所有工具规范

**内容**：
```rust
use crate::tools::registry::ToolRegistry;

/// 工具配置
pub struct ToolsConfig {
    pub shell_type: ShellToolType,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
}

#[derive(Debug, Clone, Copy)]
pub enum ShellToolType {
    Default,
    Local,
    UnifiedExec,
    Disabled,
}

/// 构建工具规范
pub fn build_specs(config: &ToolsConfig) -> ToolRegistryBuilder {
    let mut builder = ToolRegistryBuilder::new();

    // 注册 grep_files
    let grep_handler = Arc::new(GrepFilesHandler);
    builder.push_spec(create_grep_files_spec(), true);
    builder.register_handler("grep_files", grep_handler);

    // 注册 read_file
    let read_handler = Arc::new(ReadFileHandler);
    builder.push_spec(create_read_file_spec(), true);
    builder.register_handler("read_file", read_handler);

    // 注册 list_dir
    let list_handler = Arc::new(ListDirHandler);
    builder.push_spec(create_list_dir_spec(), true);
    builder.register_handler("list_dir", list_handler);

    // 注册 shell
    let exec_handler = Arc::new(ExecCommandHandler);
    builder.push_spec(create_shell_spec(), true);
    builder.register_handler("shell", exec_handler);

    // 注册 apply_patch
    let patch_handler = Arc::new(ApplyPatchHandler);
    builder.push_spec(create_apply_patch_spec(), true);
    builder.register_handler("apply_patch", patch_handler);

    builder
}

pub struct ToolRegistryBuilder {
    specs: Vec<ToolSpec>,
    registry: ToolRegistry,
}

impl ToolRegistryBuilder {
    pub fn new() -> Self {
        Self {
            specs: Vec::new(),
            registry: ToolRegistry::new(),
        }
    }

    pub fn push_spec(&mut self, spec: ToolSpec, parallel: bool) {
        self.specs.push(spec);
    }

    pub fn register_handler(&mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) {
        self.registry.register(name, handler);
    }

    pub fn build(self) -> (ToolRegistry, Vec<ToolSpec>) {
        (self.registry, self.specs)
    }
}
```

**验证**：编译通过，规范注册正常

---

### 第三阶段：实现 Hooks 系统

#### 步骤 3.1：定义 Hooks 类型

**文件**：`openjax-core/src/tools/events.rs`（新建）

**目标**：定义工具执行前后的钩子事件

**内容**：
```rust
use serde::{Deserialize, Serialize};

/// 工具使用前钩子
#[derive(Debug, Clone, Serialize)]
pub struct BeforeToolUse {
    pub tool_name: String,
    pub call_id: String,
    pub tool_input: String,
}

/// 工具使用后钩子
#[derive(Debug, Clone, Serialize)]
pub struct AfterToolUse {
    pub tool_name: String,
    pub call_id: String,
    pub tool_input: String,
    pub executed: bool,
    pub success: bool,
    pub duration_ms: u64,
    pub mutating: bool,
    pub sandbox: String,
    pub sandbox_policy: String,
    pub output_preview: Option<String>,
}

/// 钩子事件
#[derive(Debug, Clone)]
pub enum HookEvent {
    BeforeToolUse(BeforeToolUse),
    AfterToolUse(AfterToolUse),
}
```

**验证**：编译通过，钩子类型定义正确

---

#### 步骤 3.2：实现 Hooks 执行器

**文件**：`openjax-core/src/tools/hooks.rs`（新建）

**目标**：实现钩子执行逻辑

**内容**：
```rust
use crate::tools::events::HookEvent;
use tracing::{debug, info};

/// 钩子执行器
pub struct HookExecutor;

impl HookExecutor {
    pub fn new() -> Self {
        Self
    }

    /// 执行钩子事件
    pub fn execute(&self, event: &HookEvent) {
        match event {
            HookEvent::BeforeToolUse(data) => {
                debug!(
                    tool_name = %data.tool_name,
                    call_id = %data.call_id,
                    "BeforeToolUse: {}", data.tool_input
                );
            }
            HookEvent::AfterToolUse(data) => {
                info!(
                    tool_name = %data.tool_name,
                    call_id = %data.call_id,
                    success = data.success,
                    duration_ms = data.duration_ms,
                    "AfterToolUse: executed={}, mutating={}",
                    data.executed, data.mutating
                );
            }
        }
    }
}
```

**验证**：编译通过，钩子执行器正常

---

### 第四阶段：集中管理沙箱和批准逻辑

#### 步骤 4.1：定义沙箱策略

**文件**：`openjax-core/src/tools/sandboxing.rs`（新建）

**目标**：定义沙箱策略和选择逻辑

**内容**：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPolicy {
    None,
    ReadOnly,
    Write,
    DangerFullAccess,
}

impl SandboxPolicy {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_SANDBOX_MODE").as_deref() {
            Ok("danger_full_access") => Self::DangerFullAccess,
            Ok("workspace_write") => Self::Write,
            Ok("read_only") => Self::ReadOnly,
            _ => Self::Write,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ReadOnly => "read_only",
            Self::Write => "workspace_write",
            Self::DangerFullAccess => "danger_full_access",
        }
    }
}
```

**验证**：编译通过，沙箱策略定义正确

---

#### 步骤 4.2：实现工具编排器

**文件**：`openjax-core/src/tools/orchestrator.rs`（新建）

**目标**：集中管理沙箱选择、批准流程、重试逻辑

**内容**：
```rust
use crate::tools::context::{ToolInvocation, ToolOutput};
use crate::tools::registry::ToolRegistry;
use crate::tools::sandboxing::SandboxPolicy;
use crate::tools::hooks::HookExecutor;

/// 工具编排器
pub struct ToolOrchestrator {
    registry: ToolRegistry,
    hook_executor: HookExecutor,
    sandbox_policy: SandboxPolicy,
}

impl ToolOrchestrator {
    pub fn new(registry: ToolRegistry, sandbox_policy: SandboxPolicy) -> Self {
        Self {
            registry,
            hook_executor: HookExecutor::new(),
            sandbox_policy,
        }
    }

    /// 执行工具调用
    pub async fn run(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. 执行前钩子
        self.hook_executor.execute(&HookEvent::BeforeToolUse(
            BeforeToolUse {
                tool_name: invocation.tool_name.clone(),
                call_id: invocation.call_id.clone(),
                tool_input: format!("{:?}", invocation.payload),
            }
        ));

        // 2. 检查是否需要批准
        if self.requires_approval(&invocation) {
            if !self.ask_for_approval(&invocation)? {
                return Err(FunctionCallError::ApprovalRejected(
                    "command rejected by user".to_string()
                ));
            }
        }

        // 3. 选择合适的沙箱
        let sandbox = self.select_sandbox(&invocation);

        // 4. 执行工具
        let start = std::time::Instant::now();
        let result = self.registry.dispatch(invocation).await;
        let duration = start.elapsed();

        // 5. 执行后钩子
        let is_mutating = self.registry.handler(&invocation.tool_name)
            .map(|h| async { h.is_mutating(&invocation).await })
            .unwrap_or(false);

        self.hook_executor.execute(&HookEvent::AfterToolUse(
            AfterToolUse {
                tool_name: invocation.tool_name.clone(),
                call_id: invocation.call_id.clone(),
                tool_input: format!("{:?}", invocation.payload),
                executed: result.is_ok(),
                success: result.is_ok(),
                duration_ms: duration.as_millis(),
                mutating: is_mutating,
                sandbox: sandbox.as_str().to_string(),
                sandbox_policy: self.sandbox_policy.as_str().to_string(),
                output_preview: result.as_ref().ok().map(|o| format!("{:?}", o)),
            }
        ));

        result
    }

    fn requires_approval(&self, invocation: &ToolInvocation) -> bool {
        match self.sandbox_policy {
            SandboxPolicy::None => false,
            SandboxPolicy::ReadOnly => false,
            SandboxPolicy::Write => self.is_mutating_operation(invocation),
            SandboxPolicy::DangerFullAccess => false,
        }
    }

    fn select_sandbox(&self, invocation: &ToolInvocation) -> SandboxPolicy {
        // 根据工具类型和操作选择合适的沙箱
        self.sandbox_policy
    }

    fn is_mutating_operation(&self, invocation: &ToolInvocation) -> bool {
        // 检查工具是否是变异操作
        match invocation.tool_name.as_str() {
            "shell" | "apply_patch" => true,
            _ => false,
        }
    }

    fn ask_for_approval(&self, invocation: &ToolInvocation) -> Result<bool, FunctionCallError> {
        println!("[approval] 执行工具需要确认: {}", invocation.tool_name);
        println!("[approval] 输入 y 同意，其他任意输入拒绝:");

        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| FunctionCallError::Internal(format!("failed to read approval: {}", e)))?;

        Ok(answer.trim().eq_ignore_ascii_case("y"))
    }
}
```

**验证**：编译通过，编排器功能正常

---

### 第五阶段：支持动态注册和扩展性

#### 步骤 5.1：实现动态工具支持

**文件**：`openjax-core/src/tools/handlers/dynamic.rs`（新建）

**目标**：支持动态加载的工具

**内容**：
```rust
use async_trait::async_trait;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::registry::{ToolHandler, ToolKind};

pub struct DynamicToolHandler {
    spec: DynamicToolSpec,
}

#[derive(Debug, Clone)]
pub struct DynamicToolSpec {
    pub name: String,
    pub description: String,
    pub execute_fn: Box<dyn Fn(ToolInvocation) -> Result<ToolOutput, FunctionCallError> + Send + Sync>,
}

#[async_trait]
impl ToolHandler for DynamicToolHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        (self.spec.execute_fn)(invocation).await
    }
}
```

**验证**：编译通过，动态工具支持正常

---

#### 步骤 5.2：更新工具路由器

**文件**：`openjax-core/src/tools/router.rs`（修改）

**目标**：集成 ToolOrchestrator 到现有路由器

**内容**：
```rust
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::context::{ToolInvocation, ToolPayload};

pub struct ToolRouter {
    orchestrator: ToolOrchestrator,
}

impl ToolRouter {
    pub fn new(orchestrator: ToolOrchestrator) -> Self {
        Self { orchestrator }
    }

    pub async fn execute(&self, call: &ToolCall, cwd: &Path, config: ToolRuntimeConfig) -> Result<String> {
        // 构造 ToolInvocation
        let invocation = ToolInvocation {
            tool_name: call.name.clone(),
            call_id: uuid::Uuid::new_v4().to_string(),
            payload: ToolPayload::Function {
                arguments: serde_json::to_string(&call.args)
                    .map_err(|e| anyhow!("failed to serialize args: {}", e))?
            },
            turn: ToolTurnContext {
                cwd: cwd.to_path_buf(),
                sandbox_policy: config.sandbox_policy,
                windows_sandbox_level: None,
            },
        };

        // 使用编排器执行
        let result = self.orchestrator.run(invocation).await?;

        // 转换输出为字符串
        match result {
            ToolOutput::Function { body, .. } => {
                match body {
                    FunctionCallOutputBody::Text(text) => Ok(text),
                    FunctionCallOutputBody::Json(json) => Ok(json.to_string()),
                }
            }
            ToolOutput::Mcp { result, .. } => {
                match result {
                    Ok(r) => Ok(serde_json::to_string(&r)?),
                    Err(e) => Ok(format!("error: {}", e)),
                }
            }
        }
    }
}
```

**验证**：编译通过，路由器集成正常

---

## 执行顺序

### 优先级排序

1. **高优先级**（核心基础设施）：
   - 步骤 1.1：定义核心类型
   - 步骤 1.2：定义 ToolHandler trait
   - 步骤 1.3：实现工具注册表
   - 步骤 1.4：迁移现有工具到 ToolHandler

2. **中优先级**（增强功能）：
   - 步骤 2.1：定义工具规范
   - 步骤 2.2：实现工具规范注册
   - 步骤 3.1：定义 Hooks 类型
   - 步骤 3.2：实现 Hooks 执行器

3. **低优先级**（扩展性）：
   - 步骤 4.1：定义沙箱策略
   - 步骤 4.2：实现工具编排器
   - 步骤 5.1：实现动态工具支持
   - 步骤 5.2：更新工具路由器

### 时间估算

- 第一阶段：2-3 天
- 第二阶段：1-2 天
- 第三阶段：1 天
- 第四阶段：2-3 天
- 第五阶段：1-2 天

**总计**：7-11 天

## 验证标准

每个步骤完成后，需要满足以下验证标准：

1. **编译通过**：`cargo build -p openjax-core` 无错误无警告
2. **功能测试**：运行现有测试用例，确保功能正常
3. **集成测试**：确保新组件与现有系统正确集成
4. **文档更新**：更新相关文档以反映新架构

## 回滚计划

如果某个步骤出现问题，可以回滚到上一个稳定状态：

1. 使用 git 创建每个步骤的分支
2. 每个步骤完成后提交代码
3. 如果出现问题，可以回滚到上一个提交

## 风险评估

### 高风险
- **步骤 1.4**：迁移现有工具可能引入 bug
  - 缓解措施：充分测试，保持原有实现作为参考

### 中风险
- **步骤 4.2**：工具编排器可能影响现有沙箱逻辑
  - 缓解措施：逐步迁移，保持向后兼容

### 低风险
- 其他步骤：主要是新增功能，风险较低

## 成功标准

完成所有步骤后，OpenJax 工具系统应该具备以下能力：

1. ✅ 统一的 `ToolHandler` trait
2. ✅ 动态工具注册表
3. ✅ 工具规范定义（schema）
4. ✅ Hooks 系统（执行前/后钩子）
5. ✅ 丰富的输出格式（Text、Json）
6. ✅ 集中的沙箱和批准管理
7. ✅ 支持动态注册和扩展
8. ✅ 与 Codex 架构一致

## 后续扩展方向

完成基础架构后，可以轻松扩展以下功能：

1. **MCP 工具支持**：集成 Model Context Protocol
2. **自定义工具**：允许用户定义自己的工具
3. **并行执行**：支持工具并行调用
4. **工具链**：支持工具之间的依赖关系
5. **工具缓存**：缓存工具执行结果
6. **工具监控**：集成监控和告警系统
