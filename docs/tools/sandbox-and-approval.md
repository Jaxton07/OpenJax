# 沙箱和批准机制

本文档介绍了 OpenJax 工具系统的沙箱和批准机制。

## 概述

沙箱和批准机制提供了对工具执行的安全控制，确保工具在受控的环境中运行，并允许用户控制哪些操作需要批准。

## 沙箱策略

### SandboxPolicy

```rust
pub enum SandboxPolicy {
    None,
    ReadOnly,
    Write,
    DangerFullAccess,
}
```

### 策略说明

- **None**: 无沙箱限制（不推荐）
- **ReadOnly**: 只读模式，不允许任何写入操作
- **Write**: 允许写入操作，但限制可执行的命令
- **DangerFullAccess**: 完全访问模式，无任何限制（危险）

### 环境变量配置

```bash
# 设置沙箱模式
export OPENJAX_SANDBOX_MODE=workspace_write  # 默认
export OPENJAX_SANDBOX_MODE=danger_full_access  # 无限制
export OPENJAX_SANDBOX_MODE=read_only  # 只读
```

## 沙箱模式

### WorkspaceWrite

限制性沙箱模式，只允许执行安全的命令。

**允许的程序**：
- `pwd` - 显示当前目录
- `ls` - 列出目录内容
- `cat` - 显示文件内容
- `rg` - ripgrep 搜索
- `grep` - 文本搜索
- `find` - 查找文件
- `head` - 显示文件开头
- `tail` - 显示文件结尾
- `wc` - 统计行数、字数等
- `sed` - 文本编辑
- `awk` - 文本处理
- `echo` - 输出文本
- `stat` - 显示文件状态
- `uname` - 显示系统信息
- `which` - 查找命令路径
- `env` - 显示环境变量
- `printf` - 格式化输出

### DangerFullAccess

无限制模式，允许执行任何命令。

**警告**：此模式存在安全风险，仅用于受信任的环境。

### ReadOnly

只读模式，不允许任何写入操作。

**限制**：
- 不允许执行写入文件系统的命令
- 不允许修改环境变量
- 不允许执行可能修改系统状态的命令

## 批准策略

### ApprovalPolicy

```rust
pub enum ApprovalPolicy {
    AlwaysAsk,    // 总是询问
    OnRequest,    // 仅在请求时询问
    Never,         // 从不询问
}
```

### 策略说明

- **AlwaysAsk**: 每次工具调用都需要用户批准
- **OnRequest**: 仅在工具请求批准时询问
- **Never**: 从不询问，自动批准所有操作

### 环境变量配置

```bash
# 设置批准策略
export OPENJAX_APPROVAL_POLICY=always_ask  # 默认
export OPENJAX_APPROVAL_POLICY=on_request
export OPENJAX_APPROVAL_POLICY=never
```

## 变异操作

### 变异操作定义

变异操作是指可能修改用户环境或文件系统的操作。

### 变异操作工具

以下工具被认为是变异操作：

- **shell**: 执行命令可能修改文件系统
- **apply_patch**: 应用补丁会修改文件

### 非变异操作工具

以下工具被认为是非变异操作：

- **grep_files**: 只读操作
- **read_file**: 只读操作
- **list_dir**: 只读操作

### 批准逻辑

```rust
async fn check_approval(&self, invocation: &ToolInvocation) -> Result<bool> {
    let is_mutating = self.handler.is_mutating(invocation).await;
    
    match self.config.approval_policy {
        ApprovalPolicy::AlwaysAsk => {
            // 总是询问
            self.request_approval(invocation).await
        }
        ApprovalPolicy::OnRequest => {
            // 仅在变异操作时询问
            if is_mutating {
                self.request_approval(invocation).await
            } else {
                Ok(true)
            }
        }
        ApprovalPolicy::Never => {
            // 从不询问
            Ok(true)
        }
    }
}
```

## 使用示例

### 配置沙箱和批准

```rust
use openjax_core::tools::router::{ApprovalPolicy, SandboxMode};

let config = ToolRuntimeConfig {
    approval_policy: ApprovalPolicy::AlwaysAsk,
    sandbox_mode: SandboxMode::WorkspaceWrite,
};

let router = ToolRouter::new();
let result = router.execute(&call, &cwd, config).await?;
```

### 自定义批准逻辑

```rust
use openjax_core::tools::ToolOrchestrator;

let orchestrator = ToolOrchestrator::new_with_approval_handler(
    registry,
    hook_executor,
    sandbox_manager,
    Arc::new(|invocation: &ToolInvocation| async move {
        // 自定义批准逻辑
        if invocation.name == "shell" {
            // shell 命令需要批准
            Ok(true)
        } else {
            // 其他命令自动批准
            Ok(false)
        }
    }),
);
```

## 安全最佳实践

1. **默认使用限制性沙箱**：默认使用 `WorkspaceWrite` 模式
2. **谨慎使用 DangerFullAccess**：仅在受信任的环境中使用
3. **启用批准策略**：使用 `AlwaysAsk` 或 `OnRequest` 策略
4. **审查变异操作**：特别关注 shell 和 apply_patch 操作
5. **日志记录**：记录所有工具执行，特别是变异操作
6. **定期审计**：定期审查工具执行日志

## 相关文档

- [核心组件](core-components.md) - 了解核心组件
- [使用指南](usage-guide.md) - 学习如何使用
