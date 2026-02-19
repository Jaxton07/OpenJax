# 扩展新工具指南

本文档介绍了如何为 OpenJax 工具系统扩展新工具。

## 步骤 1：创建工具处理器

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

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        // 根据工具特性决定是否为变异操作
        false
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

impl MyToolHandler {
    async fn execute_my_tool(&self, args: &MyToolArgs) -> Result<String, FunctionCallError> {
        // 实现工具逻辑
        Ok(format!("Tool executed with param1: {}", args.param1))
    }
}
```

## 步骤 2：定义参数结构

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

## 步骤 3：创建工具规范

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

## 步骤 4：注册工具

在 `openjax-core/src/tools/tool_builder.rs` 中注册工具。

```rust
use crate::tools::handlers::my_tool::MyToolHandler;

pub fn build_default_tool_registry() -> (ToolRegistry, Vec<ToolSpec>) {
    let mut builder = ToolRegistryBuilder::new();

    // 注册 my_tool
    let my_handler = Arc::new(MyToolHandler);
    builder.push_spec(create_my_tool_spec(), true);
    builder.register_handler("my_tool", my_handler);

    builder.build()
}
```

## 步骤 5：导出工具

在 `openjax-core/src/tools/handlers/mod.rs` 中导出工具。

```rust
pub mod my_tool;
pub use my_tool::MyToolHandler;
```

在 `openjax-core/src/tools/mod.rs` 中导出工具。

```rust
pub use handlers::MyToolHandler;
```

## 步骤 6：更新文档

更新 [tools-list.md](tools-list.md)，添加新工具的说明。

## 完整示例

以下是一个完整的示例，实现一个简单的 `echo` 工具：

### 1. 创建处理器

```rust
// openjax-core/src/tools/handlers/echo.rs
use async_trait::async_trait;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload, FunctionCallOutputBody};
use crate::tools::registry::{ToolHandler, ToolKind};
use crate::tools::error::FunctionCallError;
use serde::Deserialize;

#[derive(Deserialize)]
struct EchoArgs {
    message: String,
    #[serde(default)]
    repeat: u32,
}

pub struct EchoHandler;

#[async_trait]
impl ToolHandler for EchoHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        false
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "echo handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: EchoArgs = serde_json::from_str(&arguments)
            .map_err(|e| FunctionCallError::Internal(format!("failed to parse arguments: {}", e)))?;

        let repeat = args.repeat.max(1);
        let result = (0..repeat)
            .map(|_| args.message.clone())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(result),
            success: Some(true),
        })
    }
}
```

### 2. 创建工具规范

```rust
// 在 openjax-core/src/tools/spec.rs 中添加
pub fn create_echo_spec() -> ToolSpec {
    ToolSpec {
        name: "echo".to_string(),
        description: "Echo a message multiple times".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo"
                },
                "repeat": {
                    "type": "number",
                    "description": "Number of times to repeat the message",
                    "default": 1
                }
            },
            "required": ["message"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "The echoed message(s)"
        })),
    }
}
```

### 3. 注册工具

```rust
// 在 openjax-core/src/tools/tool_builder.rs 中添加
use crate::tools::handlers::echo::EchoHandler;

pub fn build_default_tool_registry() -> (ToolRegistry, Vec<ToolSpec>) {
    let mut builder = ToolRegistryBuilder::new();

    // 注册 echo
    let echo_handler = Arc::new(EchoHandler);
    builder.push_spec(create_echo_spec(), true);
    builder.register_handler("echo", echo_handler);

    // ... 其他工具

    builder.build()
}
```

### 4. 导出工具

```rust
// openjax-core/src/tools/handlers/mod.rs
pub mod echo;
pub use echo::EchoHandler;

// openjax-core/src/tools/mod.rs
pub use handlers::EchoHandler;
```

### 5. 使用工具

```bash
tool:echo message="Hello, World!" repeat=3
```

## 测试工具

为工具编写测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::context::ToolInvocation;

    #[tokio::test]
    async fn test_echo_handler() {
        let handler = EchoHandler;
        let invocation = ToolInvocation {
            name: "echo".to_string(),
            payload: ToolPayload::Function {
                arguments: serde_json::json!({
                    "message": "test",
                    "repeat": 2
                }).to_string(),
            },
            cwd: std::env::current_dir().unwrap(),
            config: Default::default(),
        };

        let result = handler.handle(invocation).await.unwrap();
        assert!(result.is_success());
    }
}
```

## 相关文档

- [核心组件](core-components.md) - 了解核心组件
- [最佳实践](best-practices.md) - 学习最佳实践
