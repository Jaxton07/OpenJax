# 最佳实践

本文档介绍了使用 OpenJax 工具系统的最佳实践。

## 1. 错误处理

### 使用 FunctionCallError

始终使用 `FunctionCallError` 返回错误：

```rust
// ✅ 好的做法
Err(FunctionCallError::Internal("failed to read file".to_string()))

// ❌ 不好的做法
Err(anyhow!("failed to read file"))
```

### 区分错误类型

根据错误类型选择合适的错误变体：

```rust
// 返回给模型的错误
Err(FunctionCallError::RespondToModel("pattern must not be empty".to_string()))

// 内部错误
Err(FunctionCallError::Internal("failed to parse arguments".to_string()))

// 工具未找到
Err(FunctionCallError::ToolNotFound("unknown_tool".to_string()))

// 批准被拒绝
Err(FunctionCallError::ApprovalRejected("user rejected".to_string()))
```

## 2. 参数验证

### 在工具开始时验证参数

```rust
async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
    let args: MyToolArgs = parse_args(&invocation)?;

    // 验证参数
    if args.pattern.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "pattern must not be empty".to_string(),
        ));
    }

    if let Some(limit) = args.limit {
        if limit > 2000 {
            return Err(FunctionCallError::RespondToModel(
                "limit cannot exceed 2000".to_string(),
            ));
        }
    }

    // 执行工具逻辑
    // ...
}
```

### 使用 serde 验证

```rust
use serde::{Deserialize, Validate};

#[derive(Deserialize, Validate)]
struct MyToolArgs {
    #[validate(length(min = 1, max = 100))]
    name: String,
    
    #[validate(range(min = 1, max = 100))]
    count: i32,
}
```

## 3. 路径验证

### 使用提供的路径验证函数

```rust
use openjax_core::tools::resolve_workspace_path;

let path = resolve_workspace_path(&cwd, &rel_path)?;
```

### 避免路径遍历攻击

```rust
// ✅ 好的做法：使用 resolve_workspace_path
let path = resolve_workspace_path(&cwd, user_input)?;

// ❌ 不好的做法：直接使用用户输入
let path = PathBuf::from(user_input);
```

## 4. 日志记录

### 使用 tracing 库进行结构化日志

```rust
use tracing::{debug, info, warn, error};

debug!(tool_name = %name, "tool started");
info!(tool_name = %name, "tool completed");
warn!(tool_name = %name, "tool warning");
error!(tool_name = %name, error = %err, "tool failed");
```

### 记录关键操作

```rust
async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
    let tool_name = invocation.name.clone();
    
    info!(tool_name = %tool_name, "Tool execution started");
    
    let start = std::time::Instant::now();
    
    match self.execute_tool(&invocation).await {
        Ok(result) => {
            let duration = start.elapsed();
            info!(
                tool_name = %tool_name,
                duration_ms = duration.as_millis(),
                "Tool execution completed successfully"
            );
            Ok(result)
        }
        Err(e) => {
            let duration = start.elapsed();
            error!(
                tool_name = %tool_name,
                duration_ms = duration.as_millis(),
                error = %e,
                "Tool execution failed"
            );
            Err(e)
        }
    }
}
```

## 5. 异步操作

### 对于 I/O 密集型操作，使用异步

```rust
use tokio::fs;

// ✅ 好的做法：使用异步 I/O
let content = tokio::fs::read_to_string(path).await?;

// ❌ 不好的做法：使用同步 I/O
let content = std::fs::read_to_string(path)?;
```

### 使用并发处理

```rust
use futures::future::join_all;

// 并发处理多个文件
let futures = files.into_iter().map(|file| {
    tokio::fs::read_to_string(file)
});

let results = join_all(futures).await;
```

## 6. 超时控制

### 为长时间运行的操作设置超时

```rust
use tokio::time::{timeout, Duration};

let result = timeout(Duration::from_secs(30), operation).await
    .map_err(|_| FunctionCallError::Internal("operation timed out".to_string()))?;
```

### 使用合理的默认超时

```rust
// 根据操作类型设置不同的超时
let timeout = match tool_name.as_str() {
    "grep_files" => Duration::from_secs(30),
    "shell" => Duration::from_secs(60),
    "read_file" => Duration::from_secs(5),
    _ => Duration::from_secs(30),
};
```

## 7. 资源管理

### 及时释放资源

```rust
use tokio::fs::File;

{
    let file = File::open(path).await?;
    // 使用文件
    // 文件在这里自动关闭
}
```

### 使用 Drop trait 清理资源

```rust
struct ToolResource {
    handle: SomeHandle,
}

impl Drop for ToolResource {
    fn drop(&mut self) {
        // 清理资源
    }
}
```

## 8. 测试

### 为工具编写单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_with_valid_args() {
        let handler = MyToolHandler;
        let invocation = create_test_invocation("test_arg");
        
        let result = handler.handle(invocation).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tool_with_invalid_args() {
        let handler = MyToolHandler;
        let invocation = create_test_invocation("");
        
        let result = handler.handle(invocation).await;
        assert!(result.is_err());
    }
}
```

### 使用测试工具

```rust
#[cfg(test)]
mod test_utils {
    use super::*;

    pub fn create_test_invocation(args: &str) -> ToolInvocation {
        ToolInvocation {
            name: "test_tool".to_string(),
            payload: ToolPayload::Function {
                arguments: args.to_string(),
            },
            cwd: std::env::current_dir().unwrap(),
            config: Default::default(),
        }
    }
}
```

## 9. 性能优化

### 复用路由器实例

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

### 使用缓存

```rust
use std::collections::HashMap;

struct ToolCache {
    cache: HashMap<String, String>,
}

impl ToolCache {
    fn get_or_insert<F>(&mut self, key: &str, f: F) -> &String
    where
        F: FnOnce() -> String,
    {
        if !self.cache.contains_key(key) {
            self.cache.insert(key.to_string(), f());
        }
        self.cache.get(key).unwrap()
    }
}
```

## 10. 安全性

### 验证所有输入

```rust
async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
    // 验证工具名称
    if !is_valid_tool_name(&invocation.name) {
        return Err(FunctionCallError::Internal("invalid tool name".to_string()));
    }

    // 验证参数
    let args = validate_args(&invocation.payload)?;

    // 验证路径
    let path = resolve_workspace_path(&invocation.cwd, &args.path)?;

    // 执行工具
    // ...
}
```

### 使用沙箱

```rust
// 始终使用沙箱模式；审批决策由 PolicyRuntime 统一管理
let config = ToolRuntimeConfig {
    sandbox_mode: SandboxMode::WorkspaceWrite,
    ..Default::default()
};
// 通过 agent 注入策略运行时以启用审批：
// agent.set_policy_runtime(Some(runtime));
```

## 11. 文档

### 为工具编写清晰的文档

```rust
/// MyTool does something useful.
/// 
/// # Arguments
/// 
/// * `param1` - Description of param1
/// * `param2` - Description of param2 (optional)
/// 
/// # Examples
/// 
/// ```
/// tool:my_tool param1=value param2=42
/// ```
pub struct MyToolHandler;
```

### 更新工具规范

```rust
pub fn create_my_tool_spec() -> ToolSpec {
    ToolSpec {
        name: "my_tool".to_string(),
        description: "A clear description of what this tool does".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "param1": {
                    "type": "string",
                    "description": "Description of param1"
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

## 相关文档

- [扩展指南](extension-guide.md) - 学习如何扩展工具
- [故障排除](troubleshooting.md) - 解决常见问题
