# 实施计划：多 Shell 支持和 apply_patch 拦截

## 概述

本计划旨在为 OpenJax 添加以下两个关键功能：
1. **多 Shell 支持**：支持 Bash、Zsh、PowerShell（当前仅支持 Zsh）
2. **apply_patch 拦截**：在 shell 中检测并拦截 apply_patch 命令

## 背景

### 当前状态

| 特性 | OpenJax | Codex |
|------|---------|-------|
| Shell 支持 | 仅 Zsh | Bash/Zsh/PowerShell |
| apply_patch 拦截 | ❌ | ✅ |

### Codex 的实现参考

#### 1. Shell 抽象（Codex）

Codex 使用 `Shell` 结构来抽象不同的 shell：

```rust
pub struct Shell {
    pub shell_type: ShellType,
    pub shell_path: PathBuf,
    pub shell_snapshot: Receiver<ShellSnapshot>,
}

pub enum ShellType {
    Bash,
    Zsh,
    PowerShell,
}
```

关键方法：
- `derive_exec_args(command, use_login_shell)`: 生成 shell 执行参数
- `shell_type`: shell 类型枚举

#### 2. apply_patch 拦截（Codex）

Codex 在 `shell.rs` 中实现了 `intercept_apply_patch` 函数：

```rust
pub(crate) async fn intercept_apply_patch(
    command: &[String],
    cwd: &Path,
    timeout_ms: Option<u64>,
    session: &Session,
    turn: &TurnContext,
    tracker: Option<&SharedTurnDiffTracker>,
    call_id: &str,
    tool_name: &str,
) -> Result<Option<ToolOutput>, FunctionCallError>
```

拦截逻辑：
1. 检测命令是否以 `apply_patch` 开头
2. 使用 `codex_apply_patch::maybe_parse_apply_patch_verified` 解析
3. 如果成功解析，调用专门的 apply_patch 工具
4. 记录模型警告，建议使用 apply_patch 工具

## 实施计划

### 阶段 1：Shell 抽象（2-3 天）

#### 1.1 创建 Shell 类型枚举

**文件**: `openjax-core/src/tools/shell.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    PowerShell,
}

impl ShellType {
    pub fn default() -> Self {
        Self::Zsh
    }

    pub fn executable_name(&self) -> &str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::PowerShell => "pwsh",
        }
    }

    pub fn login_flag(&self) -> &str {
        match self {
            Self::Bash => "--login",
            Self::Zsh => "-l",
            Self::PowerShell => "-Login",
        }
    }
}
```

#### 1.2 创建 Shell 结构

```rust
#[derive(Debug, Clone)]
pub struct Shell {
    pub shell_type: ShellType,
    pub shell_path: PathBuf,
}

impl Shell {
    pub fn new(shell_type: ShellType) -> Result<Self> {
        let executable = shell_type.executable_name();
        let shell_path = which::which(executable)
            .ok_or_else(|| anyhow!("{} not found", executable))?;

        Ok(Self {
            shell_type,
            shell_path,
        })
    }

    pub fn derive_exec_args(&self, command: &str, use_login_shell: Option<bool>) -> Vec<String> {
        let login_flag = use_login_shell.unwrap_or(true);
        let flag = self.shell_type.login_flag();
        let args = if login_flag {
            vec![flag, "-c", command]
        } else {
            vec!["-c", command]
        };

        args
    }
}
```

#### 1.3 更新 ToolRuntimeConfig

在 `openjax-core/src/tools/router.rs` 中添加：

```rust
pub struct ToolRuntimeConfig {
    pub approval_policy: ApprovalPolicy,
    pub sandbox_mode: SandboxMode,
    pub shell_type: ShellType,  // 新增
}

impl Default for ToolRuntimeConfig {
    fn default() -> Self {
        Self {
            approval_policy: ApprovalPolicy::OnRequest,
            sandbox_mode: SandboxMode::WorkspaceWrite,
            shell_type: ShellType::default(),
        }
    }
}
```

#### 1.4 更新 shell

修改 `openjax-core/src/tools/shell.rs`：

```rust
pub async fn shell(
    call: &ToolCall,
    cwd: &Path,
    config: ToolRuntimeConfig,
) -> Result<String> {
    let command = call.args.get("cmd")
        .ok_or_else(|| anyhow!("shell requires cmd='<shell command>'"))?
        .to_string();

    let shell = Shell::new(config.shell_type)?;

    let shell_args = shell.derive_exec_args(&command, None);

    info!(
        command = %command,
        shell_type = ?config.shell_type,
        "shell started"
    );

    // ... 其余逻辑保持不变 ...
}
```

#### 1.5 添加 Shell 检测函数

```rust
pub fn detect_user_shell() -> ShellType {
    if cfg!(windows) {
        return ShellType::PowerShell;
    }

    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.contains("bash") {
        ShellType::Bash
    } else if shell.contains("zsh") {
        ShellType::Zsh
    } else {
        ShellType::default()
    }
}
```

### 阶段 2：apply_patch 拦截（1-2 天）

#### 2.1 创建 apply_patch 拦截模块

**文件**: `openjax-core/src/tools/apply_patch_interceptor.rs`

```rust
use crate::tools::apply_patch::apply_patch_tool;
use crate::tools::context::ToolInvocation;
use crate::tools::error::FunctionCallError;
use crate::tools::context::ToolOutput;

pub async fn intercept_apply_patch(
    command: &[String],
    cwd: &Path,
    timeout_ms: Option<u64>,
    turn_context: &crate::tools::context::ToolTurnContext,
    call_id: &str,
    tool_name: &str,
) -> Result<Option<ToolOutput>, FunctionCallError> {
    if command.len() < 2 || command[0] != "apply_patch" {
        return Ok(None);
    }

    let patch_input = command[1..].join(" ");

    match apply_patch_tool::parse_and_apply(
        cwd,
        &patch_input,
        turn_context,
        call_id,
    ).await {
        Ok(result) => {
            tracing::warn!(
                "apply_patch was requested via {}. Use apply_patch tool instead.",
                tool_name
            );
            Ok(Some(result))
        }
        Err(e) => Err(FunctionCallError::Internal(e.to_string())),
    }
}
```

#### 2.2 更新 shell 集成拦截

在 `openjax-core/src/tools/shell.rs` 中添加拦截逻辑：

```rust
pub async fn shell(
    call: &ToolCall,
    cwd: &Path,
    config: ToolRuntimeConfig,
    turn_context: &ToolTurnContext,
    call_id: &str,
) -> Result<String> {
    let command = call.args.get("cmd")
        .ok_or_else(|| anyhow!("shell requires cmd='<shell command>'"))?
        .to_string();

    let shell = Shell::new(config.shell_type)?;
    let shell_args = shell.derive_exec_args(&command, None);

    info!(
        command = %command,
        shell_type = ?config.shell_type,
        "shell started"
    );

    // 拦截 apply_patch
    if let Some(output) = apply_patch_interceptor::intercept_apply_patch(
        &command.split_whitespace().collect::<Vec<_>>(),
        cwd,
        None,
        turn_context,
        call_id,
        "shell",
    ).await? {
        return Ok(format_tool_output(&output));
    }

    // ... 其余逻辑保持不变 ...
}
```

### 阶段 3：测试（1-2 天）

#### 3.1 Shell 抽象测试

创建 `openjax-core/src/tools/shell.rs` 的测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_executable_name() {
        assert_eq!(ShellType::Bash.executable_name(), "bash");
        assert_eq!(ShellType::Zsh.executable_name(), "zsh");
        assert_eq!(ShellType::PowerShell.executable_name(), "pwsh");
    }

    #[test]
    fn test_shell_type_login_flag() {
        assert_eq!(ShellType::Bash.login_flag(), "--login");
        assert_eq!(ShellType::Zsh.login_flag(), "-l");
        assert_eq!(ShellType::PowerShell.login_flag(), "-Login");
    }

    #[test]
    fn test_shell_derive_exec_args_with_login() {
        let shell = Shell::new(ShellType::Bash).unwrap();
        let args = shell.derive_exec_args("echo hello", Some(true));
        assert_eq!(args, vec!["--login", "-c", "echo hello"]);
    }

    #[test]
    fn test_shell_derive_exec_args_without_login() {
        let shell = Shell::new(ShellType::Zsh).unwrap();
        let args = shell.derive_exec_args("echo hello", Some(false));
        assert_eq!(args, vec!["-c", "echo hello"]);
    }
}
```

#### 3.2 apply_patch 拦截测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_intercept_non_apply_patch_command() {
        let temp = tempdir().expect("create tempdir");
        let cwd = temp.path();

        let command = vec!["echo".to_string(), "hello".to_string()];
        let result = intercept_apply_patch(
            &command,
            cwd,
            None,
            &ToolTurnContext::default(),
            "test-call-id",
            "test-tool",
        ).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_intercept_apply_patch_command() {
        let temp = tempdir().expect("create tempdir");
        let cwd = temp.path();

        let patch = r#"*** Begin Patch
*** Add File: test.txt
+Hello world
*** End Patch"#;

        let command = vec!["apply_patch".to_string(), patch.to_string()];
        let result = intercept_apply_patch(
            &command,
            cwd,
            None,
            &ToolTurnContext::default(),
            "test-call-id",
            "test-tool",
        ).await.unwrap();

        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_intercept_apply_patch_with_valid_patch() {
        let temp = tempdir().expect("create tempdir");
        let cwd = temp.path();

        let patch = r#"*** Begin Patch
*** Add File: test.txt
+Hello world
*** End Patch"#;

        let command = vec!["apply_patch".to_string(), patch.to_string()];
        let result = intercept_apply_patch(
            &command,
            cwd,
            None,
            &ToolTurnContext::default(),
            "test-call-id",
            "test-tool",
        ).await.unwrap();

        assert!(result.is_some());
    }
}
```

### 阶段 4：文档更新（0.5 天）

更新以下文档：

1. `openjax-core/src/tools/README.md`
2. `docs/tools.md`（如果存在）
3. `CLAUDE.md`（更新工具系统说明）

### 阶段 5：集成测试（0.5 天）

确保所有现有测试仍然通过，新功能正常工作。

## 实施顺序

| 阶段 | 任务 | 预计时间 | 优先级 |
|------|------|---------|--------|
| 1 | Shell 抽象 | 2-3 天 | P0 |
| 2 | apply_patch 拦截 | 1-2 天 | P0 |
| 3 | 测试 | 1-2 天 | P0 |
| 4 | 文档更新 | 0.5 天 | P1 |
| 5 | 集成测试 | 0.5 天 | P1 |
| **总计** | **5-8 天** | - |

## 成功标准

### 功能标准

- ✅ 支持 Bash、Zsh、PowerShell 三种 shell
- ✅ 自动检测用户 shell 类型
- ✅ 在 shell 中拦截 apply_patch 命令
- ✅ 拦截后调用专门的 apply_patch 工具
- ✅ 记录警告信息

### 质量标准

- ✅ 所有新功能通过单元测试
- ✅ 所有现有测试仍然通过
- ✅ 代码审查通过
- ✅ 文档完整更新

## 风险管理

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| Shell 检测不准确 | 命令可能使用错误的 shell | 提供配置选项覆盖 |
| apply_patch 拦截误判 | 正常命令被拦截 | 严格匹配 `apply_patch` 前缀 |
| Windows 兼容性 | PowerShell 行为可能不同 | 单独测试 Windows 路径 |

## 后续优化

1. **Shell 配置文件**：允许用户配置首选 shell
2. **Shell 环境变量**：更智能的 shell 检测
3. **apply_patch 语法增强**：支持更多补丁语法变体
4. **性能优化**：缓存 shell 检测结果