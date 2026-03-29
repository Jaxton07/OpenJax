# 参考资源和扩展方向

本文档提供了 OpenJax 工具系统的参考资源和未来扩展方向。

## 内部文档

### 核心模块

- [context.rs](../context.rs) - 核心类型定义
- [registry.rs](../registry.rs) - ToolHandler trait 和工具注册表
- [spec.rs](../spec.rs) - 工具规范定义
- [tool_builder.rs](../tool_builder.rs) - 工具注册构建器
- [events.rs](../events.rs) - Hooks 事件类型
- [hooks.rs](../hooks.rs) - Hooks 执行器
- [sandboxing.rs](../sandboxing.rs) - 沙箱策略管理器
- [orchestrator.rs](../orchestrator.rs) - 工具编排器
- [dynamic.rs](../dynamic.rs) - 动态工具管理器
- [router.rs](../router.rs) - 工具调用解析和配置类型
- [router_impl.rs](../router_impl.rs) - 工具路由器实现
- [common.rs](../common.rs) - 通用工具函数

### 工具处理器

- [grep_files.rs](../handlers/grep_files.rs) - grep_files 工具处理器
- [read.rs](../handlers/read.rs) - Read 工具处理器
- [list_dir.rs](../handlers/list_dir.rs) - list_dir 工具处理器
- [shell.rs](../handlers/shell.rs) - shell 命令处理器

## 系统类工具

- [system/mod.rs](../system/mod.rs) - 系统工具模块导出
- [system/process_snapshot.rs](../system/process_snapshot.rs) - 进程快照工具
- [system/system_load.rs](../system/system_load.rs) - 系统负载工具
- [system/disk_usage.rs](../system/disk_usage.rs) - 磁盘使用工具
- [system/provider.rs](../system/provider.rs) - 指标采集 provider 抽象

## 外部参考

### Codex

- [Codex 工具系统](https://github.com/codex-ai/codex) - 参考 Codex 的实现
- [Codex 架构文档](../../../../docs/codex-architecture-reference.md) - Codex 架构详细说明
- [Codex 快速参考](../../../../docs/codex-quick-reference.md) - Codex 快速参考指南

### Rust 生态

- [Rust async_trait](https://docs.rs/async-trait/) - 异步 trait 文档
- [Rust serde](https://serde.rs/) - 序列化框架文档
- [Rust tokio](https://tokio.rs/) - 异步运行时文档
- [Rust tracing](https://docs.rs/tracing/) - 结构化日志文档

### 相关协议

- [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) - 模型上下文协议
- [OpenAI Function Calling](https://platform.openai.com/docs/guides/function-calling) - OpenAI 函数调用

## 后续扩展方向

### 1. MCP 工具支持

集成 Model Context Protocol，支持外部 MCP 服务器工具。

**目标**：
- 实现 MCP 客户端
- 支持 MCP 工具注册
- 处理 MCP 工具调用

**实现计划**：
```rust
// MCP 客户端
pub struct McpClient {
    // ...
}

// MCP 工具处理器
pub struct McpToolHandler {
    client: Arc<McpClient>,
}

impl ToolHandler for McpToolHandler {
    // ...
}
```

### 2. 自定义工具

允许用户定义自己的工具，支持插件系统。

**目标**：
- 支持用户定义工具
- 插件加载机制
- 工具市场

**实现计划**：
```rust
// 工具插件接口
pub trait ToolPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn handler(&self) -> Arc<dyn ToolHandler>;
}

// 插件管理器
pub struct PluginManager {
    plugins: Vec<Box<dyn ToolPlugin>>,
}
```

### 3. 并行执行

支持工具并行调用，提高性能。

**目标**：
- 并行执行多个工具
- 结果聚合
- 错误处理

**实现计划**：
```rust
// 并行执行
pub async fn execute_parallel(
    &self,
    invocations: Vec<ToolInvocation>,
) -> Vec<Result<ToolOutput, FunctionCallError>> {
    let futures = invocations.into_iter().map(|inv| {
        self.orchestrator.run(inv)
    });

    join_all(futures).await
}
```

### 4. 工具链

支持工具之间的依赖关系和链式调用。

**目标**：
- 定义工具依赖
- 链式调用
- 数据传递

**实现计划**：
```rust
// 工具链定义
pub struct ToolChain {
    steps: Vec<ToolStep>,
}

pub struct ToolStep {
    tool: String,
    args: serde_json::Value,
    depends_on: Vec<String>,
}

// 执行工具链
pub async fn execute_chain(&self, chain: &ToolChain) -> Result<Vec<ToolOutput>, FunctionCallError> {
    // ...
}
```

### 5. 工具缓存

缓存工具执行结果，减少重复计算。

**目标**：
- 结果缓存
- 缓存失效
- 缓存策略

**实现计划**：
```rust
// 缓存管理器
pub struct ToolCache {
    cache: HashMap<String, CachedResult>,
    ttl: Duration,
}

pub struct CachedResult {
    output: ToolOutput,
    timestamp: Instant,
}
```

### 6. 工具监控

集成监控和告警系统，追踪工具使用情况。

**目标**：
- 使用统计
- 性能监控
- 告警机制

**实现计划**：
```rust
// 监控指标
pub struct ToolMetrics {
    call_count: HashMap<String, u64>,
    success_count: HashMap<String, u64>,
    error_count: HashMap<String, u64>,
    avg_duration: HashMap<String, Duration>,
}

// 监控 Hook
pub struct MonitoringHook {
    metrics: Arc<RwLock<ToolMetrics>>,
}
```

### 7. 性能优化

优化工具执行性能，减少延迟。

**目标**：
- 减少开销
- 优化 I/O
- 并发处理

**优化方向**：
- 使用更高效的数据结构
- 减少不必要的拷贝
- 优化异步操作
- 使用连接池

### 8. 类型安全增强

使用更强的类型系统，减少运行时错误。

**目标**：
- 编译时类型检查
- 减少运行时错误
- 更好的 IDE 支持

**实现计划**：
```rust
// 使用宏生成类型安全的工具
macro_rules! define_tool {
    ($name:ident, $args:ty) => {
        pub struct $name {
            args: $args,
        }
    };
}

// 类型安全的工具调用
define_tool!(GrepFiles, GrepFilesArgs);
```

### 9. 工具测试框架

提供专门的工具测试框架。

**目标**：
- 简化测试编写
- 提供测试工具
- 集成测试

**实现计划**：
```rust
// 测试框架
pub struct ToolTestHarness {
    registry: ToolRegistry,
}

impl ToolTestHarness {
    pub async fn test_tool(&self, name: &str, args: &str) -> Result<ToolOutput, FunctionCallError> {
        // ...
    }
}
```

### 10. 工具文档生成

自动生成工具文档。

**目标**：
- 从工具规范生成文档
- 支持多种格式
- 自动更新

**实现计划**：
```rust
// 文档生成器
pub struct DocGenerator;

impl DocGenerator {
    pub fn generate_markdown(&self, specs: &[ToolSpec]) -> String {
        // ...
    }

    pub fn generate_html(&self, specs: &[ToolSpec]) -> String {
        // ...
    }
}
```

## 贡献指南

欢迎贡献新的工具和改进！

### 贡献流程

1. Fork 项目
2. 创建功能分支
3. 实现功能
4. 编写测试
5. 更新文档
6. 提交 Pull Request

### 代码规范

- 遵循 Rust 代码规范
- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码
- 编写充分的测试
- 更新相关文档

## 相关文档

- [概述](overview.md) - 了解工具系统概述
- [架构设计](architecture.md) - 了解架构设计
- [扩展指南](extension-guide.md) - 学习如何扩展工具
