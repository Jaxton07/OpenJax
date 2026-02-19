# Hooks 系统

本文档介绍了 OpenJax 工具系统的 Hooks 系统。

## 概述

Hooks 系统允许在工具执行前后插入自定义逻辑，用于日志记录、监控、审计等场景。

## HookEvent 类型

```rust
pub enum HookEvent {
    BeforeToolUse(BeforeToolUse),
    AfterToolUse(AfterToolUse),
}
```

## BeforeToolUse

工具使用前钩子，在工具执行前触发。

### 字段

```rust
pub struct BeforeToolUse {
    pub tool_name: String,
    pub call_id: String,
    pub tool_input: String,
}
```

- `tool_name`: 工具名称
- `call_id`: 调用 ID
- `tool_input`: 工具输入

### 使用场景

- 日志记录工具调用
- 验证工具参数
- 检查权限
- 收集使用统计

## AfterToolUse

工具使用后钩子，在工具执行后触发。

### 字段

```rust
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
```

- `tool_name`: 工具名称
- `call_id`: 调用 ID
- `tool_input`: 工具输入
- `executed`: 是否执行成功
- `success`: 是否成功
- `duration_ms`: 执行时长（毫秒）
- `mutating`: 是否为变异操作
- `sandbox`: 沙箱类型
- `sandbox_policy`: 沙箱策略
- `output_preview`: 输出预览

### 使用场景

- 记录工具执行结果
- 监控性能
- 审计日志
- 错误追踪
- 使用统计

## 实现 Hook

### BeforeToolUse Hook

```rust
use async_trait::async_trait;
use crate::tools::events::BeforeToolUse;

#[async_trait]
pub trait BeforeToolUseHook: Send + Sync {
    async fn execute(&self, event: &BeforeToolUse) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct LoggingBeforeHook;

#[async_trait]
impl BeforeToolUseHook for LoggingBeforeHook {
    async fn execute(&self, event: &BeforeToolUse) -> Result<(), Box<dyn std::error::Error>> {
        println!("Tool '{}' called with input: {}", event.tool_name, event.tool_input);
        Ok(())
    }
}
```

### AfterToolUse Hook

```rust
use async_trait::async_trait;
use crate::tools::events::AfterToolUse;

#[async_trait]
pub trait AfterToolUseHook: Send + Sync {
    async fn execute(&self, event: &AfterToolUse) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct LoggingAfterHook;

#[async_trait]
impl AfterToolUseHook for LoggingAfterHook {
    async fn execute(&self, event: &AfterToolUse) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "Tool '{}' executed in {}ms, success: {}",
            event.tool_name, event.duration_ms, event.success
        );
        Ok(())
    }
}
```

## 注册 Hooks

```rust
use openjax_core::tools::{HookExecutor, BeforeToolUseHook, AfterToolUseHook};

let mut hook_executor = HookExecutor::new();

// 注册 BeforeToolUse Hook
hook_executor.register_before_hook(Arc::new(LoggingBeforeHook));

// 注册 AfterToolUse Hook
hook_executor.register_after_hook(Arc::new(LoggingAfterHook));
```

## 使用 Hooks

```rust
use openjax_core::tools::{HookExecutor, HookEvent, BeforeToolUse, AfterToolUse};

let hook_executor = HookExecutor::new();

// 执行前钩子
hook_executor.execute(&HookEvent::BeforeToolUse(BeforeToolUse {
    tool_name: "grep_files".to_string(),
    call_id: "12345".to_string(),
    tool_input: "pattern=fn main".to_string(),
})).await;

// 执行工具...

// 执行后钩子
hook_executor.execute(&HookEvent::AfterToolUse(AfterToolUse {
    tool_name: "grep_files".to_string(),
    call_id: "12345".to_string(),
    tool_input: "pattern=fn main".to_string(),
    executed: true,
    success: true,
    duration_ms: 150,
    mutating: false,
    sandbox: "workspace_write".to_string(),
    sandbox_policy: "workspace_write".to_string(),
    output_preview: Some("file1.rs\nfile2.rs".to_string()),
})).await;
```

## 完整示例

以下是一个完整的示例，实现日志记录和监控功能：

```rust
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Instant;
use crate::tools::events::{BeforeToolUse, AfterToolUse, BeforeToolUseHook, AfterToolUseHook};

// 日志记录 Hook
pub struct LoggingHook;

#[async_trait]
impl BeforeToolUseHook for LoggingHook {
    async fn execute(&self, event: &BeforeToolUse) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            tool_name = %event.tool_name,
            call_id = %event.call_id,
            tool_input = %event.tool_input,
            "Tool execution started"
        );
        Ok(())
    }
}

#[async_trait]
impl AfterToolUseHook for LoggingHook {
    async fn execute(&self, event: &AfterToolUse) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            tool_name = %event.tool_name,
            call_id = %event.call_id,
            executed = event.executed,
            success = event.success,
            duration_ms = event.duration_ms,
            mutating = event.mutating,
            "Tool execution completed"
        );
        Ok(())
    }
}

// 性能监控 Hook
pub struct PerformanceMonitorHook {
    slow_threshold_ms: u64,
}

impl PerformanceMonitorHook {
    pub fn new(slow_threshold_ms: u64) -> Self {
        Self { slow_threshold_ms }
    }
}

#[async_trait]
impl AfterToolUseHook for PerformanceMonitorHook {
    async fn execute(&self, event: &AfterToolUse) -> Result<(), Box<dyn std::error::Error>> {
        if event.duration_ms > self.slow_threshold_ms {
            tracing::warn!(
                tool_name = %event.tool_name,
                duration_ms = event.duration_ms,
                threshold_ms = self.slow_threshold_ms,
                "Tool execution exceeded threshold"
            );
        }
        Ok(())
    }
}

// 注册 Hooks
let mut hook_executor = HookExecutor::new();
hook_executor.register_before_hook(Arc::new(LoggingHook));
hook_executor.register_after_hook(Arc::new(LoggingHook));
hook_executor.register_after_hook(Arc::new(PerformanceMonitorHook::new(5000)));
```

## Hook 执行顺序

1. 所有 `BeforeToolUse` hooks 按注册顺序执行
2. 工具执行
3. 所有 `AfterToolUse` hooks 按注册顺序执行

## 错误处理

Hook 执行中的错误会被记录，但不会阻止工具执行：

```rust
#[async_trait]
impl BeforeToolUseHook for MyHook {
    async fn execute(&self, event: &BeforeToolUse) -> Result<(), Box<dyn std::error::Error>> {
        if some_condition {
            // 返回错误，但工具仍会执行
            return Err("Hook error".into());
        }
        Ok(())
    }
}
```

## 最佳实践

1. **快速执行**：Hooks 应该快速执行，避免影响工具性能
2. **错误处理**：妥善处理 Hook 中的错误，避免影响工具执行
3. **幂等性**：Hooks 应该是幂等的，多次执行结果一致
4. **日志记录**：使用结构化日志记录 Hook 执行情况
5. **避免副作用**：避免在 Hook 中产生副作用

## 相关文档

- [核心组件](core-components.md) - 了解核心组件
- [最佳实践](best-practices.md) - 学习最佳实践
