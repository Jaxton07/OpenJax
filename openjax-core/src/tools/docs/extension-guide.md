# 扩展新工具指南

本文档介绍了如何为 OpenJax 工具系统扩展新工具。

## 步骤 0：接入权限声明（PolicyDescriptor）

在实现处理器前，先把新工具接入 `ToolInvocation::policy_descriptor()` 的匹配分支，确保策略中心可以识别该工具的动作、能力和风险标签。

当前接入点在：`openjax-core/src/tools/context.rs`。

```rust
// openjax-core/src/tools/context.rs
impl ToolInvocation {
    pub fn policy_descriptor(&self) -> Option<PolicyDescriptor> {
        let descriptor = match self.tool_name.as_str() {
            "my_tool" => PolicyDescriptor {
                action: "read".to_string(),
                capabilities: vec!["fs_read".to_string()],
                risk_tags: vec![],
            },
            _ => return None,
        };
        Some(descriptor)
    }
}
```

没有这一步，即使处理器实现完成，也不算“工具接入完成”。

## 步骤 1：创建工具处理器

按工具类型选择目录：

- 通用工作区工具：`openjax-core/src/tools/handlers/`
- 系统观测类只读工具：`openjax-core/src/tools/system/`

在实现新工具前，先完成权限声明设计。新工具必须实现 `PolicyDescriptor`（或语义等价的权限声明接口/结构），并明确描述该工具需要的授权边界。没有权限声明的工具，不视为完成接入。

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

## 接入门禁

新工具接入在文档和 CI 层面都必须满足以下条件：

- 必须实现 `PolicyDescriptor`，或提供同义的权限声明能力，用于描述工具的授权需求
- 必须覆盖三类测试：`allow`、`ask`/`escalate`、`deny`
- 缺少权限声明时，CI 不能把该工具判定为“接入完成”

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
    use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolPayload, ToolTurnContext};

    #[tokio::test]
    async fn test_echo_handler() {
        let handler = EchoHandler;
        let invocation = ToolInvocation {
            tool_name: "echo".to_string(),
            call_id: "call_test_echo".to_string(),
            payload: ToolPayload::Function {
                arguments: serde_json::json!({
                    "message": "test",
                    "repeat": 2
                }).to_string(),
            },
            turn: ToolTurnContext::default(),
        };

        let result = handler.handle(invocation).await.unwrap();
        match result {
            crate::tools::context::ToolOutput::Function { body, success } => {
                assert_eq!(success, Some(true));
                match body {
                    FunctionCallOutputBody::Text(text) => assert!(!text.is_empty()),
                    FunctionCallOutputBody::Json(_) => panic!("unexpected json body"),
                }
            }
            crate::tools::context::ToolOutput::Mcp { .. } => panic!("unexpected mcp output"),
        }
    }
}
```

## 相关文档

- [核心组件](core-components.md) - 了解核心组件
- [最佳实践](best-practices.md) - 学习最佳实践
