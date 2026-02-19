# 动态工具

本文档介绍了 OpenJax 工具系统的动态工具支持。

## 概述

动态工具支持允许在运行时注册自定义工具，无需重新编译代码。这对于插件系统、A/B 测试、多租户支持等场景非常有用。

## DynamicToolManager

动态工具管理器，支持运行时注册自定义工具。

### API

```rust
use openjax_core::tools::DynamicToolManager;

let mut dynamic_manager = DynamicToolManager::new();

// 注册动态工具
dynamic_manager.register(name, handler);

// 列出所有工具
let tools = dynamic_manager.list_tools();

// 移除工具
dynamic_manager.unregister(name);

// 获取工具处理器
let handler = dynamic_manager.get_handler(name);
```

### 方法说明

- `register()`: 注册动态工具
- `list_tools()`: 列出所有已注册的工具
- `unregister()`: 移除已注册的工具
- `get_handler()`: 获取工具处理器

## 使用场景

### 1. 插件系统

允许用户编写自己的工具插件，在运行时加载：

```rust
use openjax_core::tools::{DynamicToolManager, ToolHandler};
use std::sync::Arc;

// 加载插件
fn load_plugin(plugin_path: &str) -> Result<Arc<dyn ToolHandler>, Box<dyn std::error::Error>> {
    // 从动态库加载工具处理器
    // ...
}

// 注册插件工具
let mut dynamic_manager = DynamicToolManager::new();
let plugin_handler = load_plugin("my_plugin.so")?;
dynamic_manager.register("my_plugin_tool", plugin_handler);
```

### 2. 运行时扩展

不需要重新编译即可添加新工具：

```rust
// 从配置文件加载工具定义
let tool_configs = load_tool_configs("tools.yaml")?;

for config in tool_configs {
    let handler = create_handler_from_config(&config)?;
    dynamic_manager.register(config.name, handler);
}
```

### 3. A/B 测试

可以动态切换不同的工具实现：

```rust
// 注册两个版本的处理器
dynamic_manager.register("tool_v1", Arc::new(ToolV1Handler));
dynamic_manager.register("tool_v2", Arc::new(ToolV2Handler));

// 根据用户分组选择版本
let handler_name = if user_group == "A" {
    "tool_v1"
} else {
    "tool_v2"
};

let handler = dynamic_manager.get_handler(handler_name)?;
```

### 4. 多租户支持

不同租户可以使用不同的工具集：

```rust
// 为每个租户创建独立的工具管理器
let mut tenant_managers: HashMap<String, DynamicToolManager> = HashMap::new();

// 为租户注册专用工具
tenant_managers.entry("tenant1".to_string())
    .or_insert_with(DynamicToolManager::new)
    .register("tenant1_tool", Arc::new(Tenant1ToolHandler));

tenant_managers.entry("tenant2".to_string())
    .or_insert_with(DynamicToolManager::new)
    .register("tenant2_tool", Arc::new(Tenant2ToolHandler));
```

## 完整示例

### 创建动态工具

```rust
use async_trait::async_trait;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload, FunctionCallOutputBody};
use crate::tools::registry::{ToolHandler, ToolKind};
use crate::tools::error::FunctionCallError;
use serde::Deserialize;

#[derive(Deserialize)]
struct DynamicToolArgs {
    message: String,
}

pub struct DynamicToolHandler {
    tool_name: String,
}

impl DynamicToolHandler {
    pub fn new(tool_name: String) -> Self {
        Self { tool_name }
    }
}

#[async_trait]
impl ToolHandler for DynamicToolHandler {
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
                    "dynamic tool handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: DynamicToolArgs = serde_json::from_str(&arguments)
            .map_err(|e| FunctionCallError::Internal(format!("failed to parse arguments: {}", e)))?;

        let result = format!("Dynamic tool '{}' executed with message: {}", self.tool_name, args.message);

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(result),
            success: Some(true),
        })
    }
}
```

### 注册和使用动态工具

```rust
use openjax_core::tools::DynamicToolManager;
use std::sync::Arc;

// 创建动态工具管理器
let mut dynamic_manager = DynamicToolManager::new();

// 创建并注册动态工具
let tool_handler = Arc::new(DynamicToolHandler::new("my_dynamic_tool".to_string()));
dynamic_manager.register("my_dynamic_tool".to_string(), tool_handler);

// 列出所有工具
let tools = dynamic_manager.list_tools();
println!("Available tools: {:?}", tools);

// 获取工具处理器
if let Some(handler) = dynamic_manager.get_handler("my_dynamic_tool") {
    // 使用工具
    let invocation = ToolInvocation {
        name: "my_dynamic_tool".to_string(),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({
                "message": "Hello from dynamic tool!"
            }).to_string(),
        },
        cwd: std::env::current_dir().unwrap(),
        config: Default::default(),
    };

    let result = handler.handle(invocation).await?;
    println!("Result: {:?}", result);
}

// 移除工具
dynamic_manager.unregister("my_dynamic_tool");
```

## 动态工具规范

动态工具也需要提供工具规范，以便系统能够正确调用：

```rust
use crate::tools::spec::ToolSpec;

fn create_dynamic_tool_spec(name: &str, description: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to process"
                }
            },
            "required": ["message"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "The processed message"
        })),
    }
}
```

## 最佳实践

1. **工具命名**：使用清晰、唯一的工具名称，避免冲突
2. **错误处理**：妥善处理工具执行中的错误
3. **资源管理**：及时清理不再使用的工具
4. **文档**：为动态工具提供清晰的文档和规范
5. **测试**：为动态工具编写充分的测试

## 限制和注意事项

1. **性能**：动态注册的工具可能有轻微的性能开销
2. **类型安全**：动态工具的类型检查在运行时进行
3. **依赖管理**：确保动态工具的依赖可用
4. **安全性**：动态工具需要经过安全审查

## 相关文档

- [扩展指南](extension-guide.md) - 学习如何扩展工具
- [核心组件](core-components.md) - 了解核心组件
