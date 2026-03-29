# 架构设计

本文档描述了 OpenJax 工具系统的架构设计和模块结构。

## 分层架构

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
│  - ShellCommandHandler                                 │
│  - ApplyPatchHandler                                  │
│  - ProcessSnapshotHandler / SystemLoadHandler         │
│  - DiskUsageHandler                                   │
│  - DynamicToolHandler (自定义)                        │
└─────────────────────────────────────────────────────────┘
```

## 模块结构

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
├── system/                  # 系统类只读工具
│   ├── mod.rs
│   ├── errors.rs
│   ├── types.rs
│   ├── provider.rs
│   ├── process_snapshot.rs
│   ├── system_load.rs
│   └── disk_usage.rs
├── handlers/                # 工具处理器目录
│   ├── mod.rs             # 处理器模块导出
│   ├── grep_files.rs      # grep_files 工具处理器
│   ├── read.rs            # Read 工具处理器
│   ├── list_dir.rs        # list_dir 工具处理器
│   ├── shell.rs          # shell 命令处理器
├── shell.rs               # shell 命令（原有）
├── grep_files.rs             # grep_files 工具（原有）
├── （legacy internal read module） # 仅内部保留，非对外契约
└── list_dir.rs               # list_dir 工具（原有）
```

## 架构层次说明

### 1. 用户层

用户通过 CLI 或 Agent 调用工具，输入格式为：
```
tool:<tool_name> <args>
```

### 2. 路由层 (ToolRouter)

**职责**：
- 解析工具调用字符串
- 创建 `ToolInvocation` 对象
- 调用 `ToolOrchestrator` 执行工具

**关键方法**：
```rust
pub async fn execute(
    &self,
    call: &ToolCall,
    cwd: &Path,
    config: ToolRuntimeConfig,
) -> Result<String>
```

### 3. 编排层 (ToolOrchestrator)

**职责**：
- 管理工具执行的完整流程
- 执行前钩子（BeforeToolUse）
- 检查是否需要批准
- 选择合适的沙箱
- 调用 ToolRegistry 分发工具
- 执行后钩子（AfterToolUse）

**关键方法**：
```rust
pub async fn run(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>
```

### 4. 注册层 (ToolRegistry)

**职责**：
- 存储所有工具处理器
- 根据工具名称分发调用到对应的处理器

**关键方法**：
```rust
pub fn register(&self, name: impl Into<String>, handler: Arc<dyn ToolHandler>)
pub async fn dispatch(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>
```

### 5. 处理层 (ToolHandler)

**职责**：
- 实现具体的工具逻辑
- 解析参数
- 执行操作
- 返回结果

**关键方法**：
```rust
async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>
```

## 数据流

```
用户输入
   ↓
ToolRouter 解析
   ↓
ToolOrchestrator 执行前钩子
   ↓
检查批准策略
   ↓
选择沙箱
   ↓
ToolRegistry 分发
   ↓
ToolHandler 执行
   ↓
返回结果
   ↓
ToolOrchestrator 执行后钩子
   ↓
返回给用户
```

## 设计原则

1. **单一职责**：每个组件只负责一个明确的功能
2. **开闭原则**：对扩展开放，对修改关闭
3. **依赖倒置**：高层模块不依赖低层模块，都依赖抽象
4. **接口隔离**：使用 trait 定义清晰的接口
5. **组合优于继承**：通过组合构建复杂功能

## 扩展性

系统支持多种扩展方式：

1. **新增工具**：优先按领域分层（通用工具放 `handlers/`，系统观测工具放 `system/`），实现 `ToolHandler` trait 并注册
2. **自定义 Hooks**：添加 BeforeToolUse 或 AfterToolUse 钩子
3. **自定义沙箱**：扩展 `SandboxPolicy`
4. **动态工具**：使用 `DynamicToolManager` 运行时注册

## 相关文档

- [核心组件](core-components.md) - 深入了解各组件的实现细节
- [扩展指南](extension-guide.md) - 学习如何扩展工具系统
