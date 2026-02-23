# Codex API & 快速参考

## 核心类型速查

### Protocol Types

```rust
// 提交操作
Op::UserTurn {
    items: Vec<UserInput>,
    cwd: PathBuf,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    model: String,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
}

// 审批策略
AskForApproval::AlwaysAsk | OnRequest | Never | OnFailure | UnlessTrusted

// 沙箱策略
SandboxPolicy::ReadOnly { roots: [...] }
SandboxPolicy::WorkspaceWrite { writable_roots: [...] }
SandboxPolicy::DangerFullAccess
SandboxPolicy::ExternalSandbox { ... }
```

### 工具调用

```rust
// 通用工具响应
struct ToolCallOutput {
    exit_code: i32,
    aggregated_output: Output,
    duration: Duration,
}

// 执行参数
ExecParams {
    command: Vec<String>,
    cwd: PathBuf,
    expiration: ExecExpiration,
    env: HashMap<String, String>,
    sandbox_permissions: SandboxPermissions,
}
```

## 常用函数

### 沙箱管理

```rust
// 获取平台沙箱
get_platform_sandbox(windows_enabled: bool) -> Option<SandboxType>

// 评估补丁安全
assess_patch_safety(
    action: &ApplyPatchAction,
    policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> SafetyCheck
```

### 工具执行

```rust
// 处理 exec 工具调用
process_exec_tool_call(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    sandbox_cwd: &Path,
) -> Result<ExecToolCallOutput>

// 应用补丁
apply_patch(
    turn_context: &TurnContext,
    action: ApplyPatchAction,
) -> InternalApplyPatchInvocation
```

## 文件路径

```
codex-rs/
├── core/src/
│   ├── lib.rs              # 库入口
│   ├── codex.rs           # Codex 主类
│   ├── client.rs           # 模型客户端
│   ├── exec.rs            # 执行工具
│   ├── apply_patch.rs     # 补丁工具
│   ├── safety.rs          # 安全评估
│   ├── exec_policy.rs     # 执行策略
│   ├── agent/             # 代理编排
│   ├── tools/             # 工具系统
│   │   ├── handlers/     # 工具处理器
│   │   └── sandboxing/   # 沙箱集成
│   └── sandboxing/       # 沙箱实现
│       ├── seatbelt.rs    # macOS
│       ├── landlock.rs    # Linux
│       └── windows_sandbox.rs
├── protocol/src/
│   └── protocol.rs        # 协议定义
└── tui/src/               # TUI 实现
```

## CLI 命令速查

| 命令 | 说明 |
|------|------|
| `codex` | 启动交互式 TUI |
| `codex exec "prompt"` | 非交互执行 |
| `codex review` | 代码审查 |
| `codex resume [id]` | 恢复会话 |
| `codex sandbox macos` | macOS 沙箱调试 |
| `codex sandbox linux` | Linux 沙箱调试 |
| `codex mcp add` | 添加 MCP 服务器 |

## 环境变量

```bash
OPENAI_API_KEY          # API 密钥
OPENAI_BASE_URL         # API 基础 URL
CODEX_SANDBOX_MODE     # 沙箱模式
CODEX_APPROVAL_POLICY  # 审批策略
```

## 补丁格式

```
*** Begin Patch
*** Add File: src/new.rs
+// 新文件内容
*** Update File: src/existing.rs
@@
 old_line
+new_line
*** Delete File: src/to_delete.rs
*** End Patch
```
