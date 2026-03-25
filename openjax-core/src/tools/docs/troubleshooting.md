# 故障排除

本文档提供了 OpenJax 工具系统的常见问题和解决方案。

## 常见问题

### 1. 工具未找到

**错误**：`ToolNotFound("unknown tool: xxx")`

**原因**：工具未注册或名称拼写错误

**解决**：
- 检查工具名称是否正确
- 确认工具已在 `build_default_tool_registry()` 中注册
- 检查工具名称的大小写

```rust
// 检查工具是否已注册
let router = ToolRouter::new();
let specs = build_all_specs();
for spec in specs {
    println!("Available tool: {}", spec.name);
}
```

### 2. 参数解析失败

**错误**：`Internal("failed to parse arguments: xxx")`

**原因**：参数格式不正确或类型不匹配

**解决**：
- 检查参数格式是否符合 JSON Schema
- 确认参数类型正确
- 检查必需参数是否提供

```bash
# 示例：正确的参数格式
tool:grep_files pattern="fn main" path=src include=*.rs

# 示例：错误的参数格式
tool:grep_files pattern=fn main path=src include=*.rs
```

### 3. 路径验证失败

**错误**：`Internal("path escapes workspace: xxx")`

**原因**：路径超出工作区或包含父目录遍历

**解决**：
- 使用相对路径
- 避免使用 `..` 或绝对路径
- 确认路径在工作区范围内

```bash
# ✅ 好的做法：使用相对路径
tool:read_file file_path=src/lib.rs

# ❌ 不好的做法：使用绝对路径
tool:read_file file_path=/path/to/file.rs

# ❌ 不好的做法：使用父目录遍历
tool:read_file file_path=../../file.rs
```

### 4. 批准被拒绝

**错误**：`ApprovalRejected("command rejected by user")`

**原因**：用户拒绝了工具执行，或 Policy Center 决策为拒绝

**解决**：
- 确认用户输入 `y` 同意执行
- 检查 Policy Center 中对应工具的规则配置
- 若需跳过审批，可在策略规则中将对应工具配置为 `DecisionKind::Allow`

```rust
// 示例：通过 Policy Center 规则放行特定工具
// 在策略规则中设置 DecisionKind::Allow，使该工具无需用户确认直接执行
```

### 5. 超时

**错误**：`Internal("operation timed out after xxx ms")`

**原因**：操作执行时间超过超时限制

**解决**：
- 增加超时时间
- 优化操作性能
- 检查是否有死锁或无限循环

```bash
# 增加超时时间
tool:shell cmd='cargo test' timeout_ms=120000
```

### 6. 沙箱限制

**错误**：`Internal("command not allowed in sandbox: xxx")`

**原因**：命令在当前沙箱模式下不被允许

**解决**：
- 检查命令是否在允许列表中
- 调整沙箱模式
- 使用替代命令

```bash
# 查看允许的命令
# WorkspaceWrite 模式允许：pwd, ls, cat, rg, grep, find, head, tail, wc, sed, awk, echo, stat, uname, which, env, printf

# 调整沙箱模式
export OPENJAX_SANDBOX_MODE=danger_full_access
```

### 7. 文件未找到

**错误**：`Internal("file not found: xxx")`

**原因**：文件路径不正确或文件不存在

**解决**：
- 检查文件路径是否正确
- 确认文件存在
- 使用 `list_dir` 查看目录结构

```bash
# 先列出目录
tool:list_dir dir_path=src

# 然后读取文件
tool:read_file file_path=src/lib.rs
```

### 8. 权限错误

**错误**：`Internal("permission denied: xxx")`

**原因**：没有足够的权限访问文件或执行命令

**解决**：
- 检查文件权限
- 使用 `require_escalated` 参数提升权限
- 确认沙箱模式允许该操作

```bash
# 提升权限
tool:shell cmd='sudo cargo install' require_escalated=true
```

### 9. macOS seatbelt 后端不可用

**错误特征**：
- 日志中出现：`sandbox audit backend unavailable backend="macos_seatbelt"`
- `reason` 包含：
  - `macos_seatbelt_unavailable_cached`
  - `macos_seatbelt_apply_denied`
  - `macos_seatbelt_unknown_nonzero`

**原因**：
- 当前运行环境不允许 `sandbox-exec` 应用策略，或 profile 与运行器不兼容。

**快速诊断**：
```bash
# 最小探针（应为 EXIT:0）
sandbox-exec -p "(version 1) (deny default) (allow process*) (allow file-read*)" /bin/sh -c "true"; echo EXIT:$?

# 模拟 agent 运行器路径
sandbox-exec -p "(version 1) (deny default) (allow process*) (allow file-read*)" /bin/sh -c "echo hi"; echo EXIT:$?
```

**当前实现行为**：
- seatbelt 后端优先使用 `/bin/sh -c` 执行。
- 若探针失败会缓存不可用状态并直接降级（避免每条命令先失败一次）。
- 降级策略由 `OPENJAX_SANDBOX_DEGRADE_POLICY` 控制（`ask_then_allow` / `deny`）。

## 调试技巧

### 1. 启用调试日志

```bash
export RUST_LOG=openjax_core=debug
```

### 2. 检查工具注册

```rust
use openjax_core::tools::build_all_specs;

let specs = build_all_specs();
for spec in specs {
    println!("Tool: {}", spec.name);
    println!("Description: {}", spec.description);
    println!("Input Schema: {}", serde_json::to_string_pretty(&spec.input_schema).unwrap());
}
```

### 3. 测试工具调用

```bash
# 直接测试工具调用
tool:grep_files pattern=fn main

# 测试参数
tool:read_file file_path=src/lib.rs offset=1 limit=10
```

### 4. 查看工具规范

```rust
use openjax_core::tools::build_all_specs;

let specs = build_all_specs();
for spec in specs {
    println!("Tool: {}", spec.name);
    println!("Description: {}", spec.description);
    println!("Input Schema: {}", serde_json::to_string_pretty(&spec.input_schema).unwrap());
}
```

### 5. 使用单元测试

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
}
```

## 性能问题

### 1. 工具执行缓慢

**可能原因**：
- 同步 I/O 操作
- 未设置超时
- 大量数据处理

**解决方案**：
- 使用异步 I/O
- 设置合理的超时
- 使用分页处理大数据

```rust
// 使用异步 I/O
let content = tokio::fs::read_to_string(path).await?;

// 设置超时
use tokio::time::{timeout, Duration};
let result = timeout(Duration::from_secs(30), operation).await?;
```

### 2. 内存占用过高

**可能原因**：
- 一次性读取大文件
- 未释放资源
- 缓存过多数据

**解决方案**：
- 使用流式处理
- 及时释放资源
- 限制缓存大小

```rust
// 使用流式处理
use tokio::io::{AsyncBufReadExt, BufReader};

let file = File::open(path).await?;
let reader = BufReader::new(file);
let mut lines = reader.lines();

while let Some(line) = lines.next_line().await? {
    // 处理每一行
}
```

## 编译问题

### 1. 类型错误

**错误**：类型不匹配

**解决**：
- 检查 trait 实现
- 确认类型签名
- 使用类型注解

```rust
// 添加类型注解
let args: MyToolArgs = serde_json::from_str(&arguments)?;
```

### 2. 依赖问题

**错误**：依赖版本冲突

**解决**：
- 更新依赖
- 清理缓存
- 重新构建

```bash
# 更新依赖
cargo update

# 清理缓存
cargo clean

# 重新构建
cargo build
```

## 获取帮助

如果以上方法都无法解决问题，可以：

1. 查看源代码中的注释和文档
2. 查看测试用例了解使用方法
3. 查看相关文档链接
4. 提交 Issue 报告问题

## 相关文档

- [使用指南](usage-guide.md) - 学习如何使用
- [最佳实践](best-practices.md) - 学习最佳实践
