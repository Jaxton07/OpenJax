# 使用指南

本文档介绍了如何使用 OpenJax 工具系统。

## 基本使用

### 1. 解析工具调用

使用 `parse_tool_call` 函数解析工具调用字符串：

```rust
use openjax_core::tools::parse_tool_call;

let call = parse_tool_call("tool:grep_files pattern=fn main")?;
```

### 2. 创建工具路由器

创建 `ToolRouter` 实例：

```rust
use openjax_core::tools::ToolRouter;

let router = ToolRouter::new();
```

### 3. 执行工具

执行工具并获取结果：

```rust
use openjax_core::tools::ToolRouter;
use std::path::Path;

let router = ToolRouter::new();
let cwd = std::env::current_dir()?;
let config = ToolRuntimeConfig {
    sandbox_mode: SandboxMode::WorkspaceWrite,
    ..Default::default()
};

let result = router.execute(&call, &cwd, config).await?;
```

## 在 Agent 中使用

### 完整示例

审批决策由 Policy Center 统一管理，通过 `PolicyRuntime` 注入 agent：

```rust
use openjax_core::tools::{ToolRouter, parse_tool_call, ToolRuntimeConfig};
use openjax_core::tools::router::SandboxMode;

pub async fn execute_tool_turn(&self, input: &str) -> Result<String> {
    let cwd = std::env::current_dir()?;
    let config = ToolRuntimeConfig {
        sandbox_mode: SandboxMode::WorkspaceWrite,
        ..Default::default()
    };

    if let Some(call) = parse_tool_call(input) {
        let router = ToolRouter::new();
        router.execute(&call, &cwd, config).await
    } else {
        Err(anyhow!("invalid tool call format"))
    }
}
```

注入 `PolicyRuntime` 以启用策略驱动审批：

```rust
use openjax_policy::PolicyRuntime;

agent.set_policy_runtime(Some(runtime));
```

## 配置选项

### 沙箱模式

```rust
pub enum SandboxMode {
    WorkspaceWrite,      // 工作区写入模式（限制性）
    DangerFullAccess,     // 完全访问模式（无限制）
}
```

### 环境变量配置

```bash
# 设置沙箱模式
export OPENJAX_SANDBOX_MODE=workspace_write  # 默认
export OPENJAX_SANDBOX_MODE=danger_full_access  # 无限制
export OPENJAX_SANDBOX_MODE=read_only  # 只读
```

审批策略由 Policy Center 管理，不再通过环境变量配置。

## 常见使用场景

### 1. 搜索代码

```bash
tool:grep_files pattern=fn main path=src include=*.rs
```

### 2. 读取文件

```bash
tool:Read file_path=src/lib.rs offset=1 limit=50
```

### 3. 列出目录

```bash
tool:list_dir dir_path=src depth=2
```

### 4. 执行命令

```bash
tool:shell cmd='cargo test' require_escalated=true
```

## 错误处理

工具执行可能返回以下错误：

```rust
use openjax_core::tools::error::FunctionCallError;

match router.execute(&call, &cwd, config).await {
    Ok(result) => println!("Result: {}", result),
    Err(FunctionCallError::ToolNotFound(name)) => {
        eprintln!("Tool not found: {}", name);
    }
    Err(FunctionCallError::ApprovalRejected(msg)) => {
        eprintln!("Approval rejected: {}", msg);
    }
    Err(FunctionCallError::Internal(msg)) => {
        eprintln!("Internal error: {}", msg);
    }
    Err(FunctionCallError::RespondToModel(msg)) => {
        eprintln!("Model response: {}", msg);
    }
}
```

## 性能优化

### 1. 复用路由器实例

```rust
// ✅ 好的做法：复用路由器
let router = ToolRouter::new();
for call in calls {
    router.execute(&call, &cwd, config).await?;
}

// ❌ 不好的做法：每次创建新路由器
for call in calls {
    let router = ToolRouter::new();
    router.execute(&call, &cwd, config).await?;
}
```

### 2. 使用适当的超时

```rust
// 为长时间运行的操作设置超时
tool:shell cmd='cargo test' timeout_ms=60000
```

### 3. 使用分页

```bash
# 使用分页减少数据传输
tool:Read file_path=large_file.txt offset=1 limit=100
tool:list_dir dir_path=src offset=1 limit=50
```

## 调试

### 启用调试日志

```bash
export RUST_LOG=openjax_core=debug
```

### 查看工具规范

```rust
use openjax_core::tools::build_all_specs;

let specs = build_all_specs();
for spec in specs {
    println!("Tool: {}", spec.name);
    println!("Description: {}", spec.description);
    println!("Input Schema: {}", serde_json::to_string_pretty(&spec.input_schema).unwrap());
}
```

## 相关文档

- [工具列表](tools-list.md) - 查看所有可用工具
- [最佳实践](best-practices.md) - 学习最佳实践
- [故障排除](troubleshooting.md) - 解决常见问题
