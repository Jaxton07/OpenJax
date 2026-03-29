# OpenJax 工具文档

本文档详细描述了 OpenJax 中所有已实现的工具及其使用方法。

## 工具概览

| 工具名称 | 功能描述 | 安全级别 |
|---------|---------|---------|
| `read_file` | 读取文件内容 | 只读 |
| `list_dir` | 列出目录内容 | 只读 |
| `grep_files` | 递归搜索文件 | 只读 |
| `shell` | 执行 shell 命令 | 可配置 |
| `write_file` | 写入文件内容 | 可配置 |

---

## 1. read_file

读取指定路径的文件内容。

### 参数

| 参数名 | 类型 | 必需 | 说明 |
|-------|------|------|------|
| `path` | string | 是 | 文件的相对路径 |

### 使用示例

```bash
tool:read_file path=src/lib.rs
tool:read_file path=openjax-core/src/tools.rs
```

### 安全特性

- 路径验证：禁止使用绝对路径
- 路径验证：禁止 `../` 父目录遍历
- 路径验证：禁止通过符号链接逃逸工作区

### 实现位置

[tools.rs:253-265](../openjax-core/src/tools.rs#L253-L265)

---

## 2. list_dir

列出指定目录下的文件和子目录。

### 参数

| 参数名 | 类型 | 必需 | 说明 |
|-------|------|------|------|
| `path` | string | 否 | 目录的相对路径，默认为 `.` |

### 使用示例

```bash
tool:list_dir path=.
tool:list_dir path=openjax-core/src
```

### 输出格式

返回按字母顺序排序的文件名列表，每行一个文件名。

### 安全特性

- 路径验证：与 `read_file` 相同

### 实现位置

[tools.rs:267-286](../openjax-core/src/tools.rs#L267-L286)

---

## 3. grep_files

在指定目录下递归搜索包含特定模式的文件。

### 参数

| 参数名 | 类型 | 必需 | 说明 |
|-------|------|------|------|
| `pattern` | string | 是 | 要搜索的文本模式 |
| `path` | string | 否 | 搜索的根目录，默认为 `.` |

### 使用示例

```bash
tool:grep_files pattern=fn main path=.
tool:grep_files pattern=ToolRouter path=openjax-core/src
```

### 输出格式

返回匹配的行，格式为 `文件路径:行号:行内容`，每行一个匹配项。

如果没有匹配项，返回 `(no matches)`。

### 安全特性

- 路径验证：与 `read_file` 相同

### 实现位置

[tools.rs:288-324](../openjax-core/src/tools.rs#L288-L324)

---

## 4. shell

执行 shell 命令，支持沙箱模式和审批策略。

### 参数

| 参数名 | 类型 | 必需 | 说明 |
|-------|------|------|------|
| `cmd` | string | 是 | 要执行的 shell 命令 |
| `require_escalated` | boolean | 否 | 是否需要审批，默认为 `false` |
| `timeout_ms` | number | 否 | 超时时间（毫秒），默认为 `30000` |

### 使用示例

```bash
tool:shell cmd='zsh -c "cargo test"' require_escalated=true timeout_ms=60000
tool:shell cmd='ls -la' timeout_ms=5000
```

### 输出格式

返回包含退出码、标准输出和标准错误的格式化字符串：

```
exit_code=0
stdout:
<标准输出内容>
stderr:
<标准错误内容>
```

### 审批策略

审批决策由 Policy Center 统一管理，通过注入 `PolicyRuntime` 配置：

```rust
use openjax_policy::{runtime::PolicyRuntime, schema::DecisionKind, store::PolicyStore};
agent.set_policy_runtime(Some(PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]))));
```

默认（无 policy_runtime）：mutating 工具和有风险标签的 shell 命令需要审批，只读工具自动放行。

### 沙箱模式

通过环境变量 `OPENJAX_SANDBOX_MODE` 配置：

#### WorkspaceWrite（默认）

限制性模式，仅允许安全的只读命令。

**允许的程序：**
- `pwd`, `ls`, `cat`, `rg`, `grep`, `find`, `head`, `tail`, `wc`
- `sed`, `awk`, `echo`, `stat`, `uname`, `which`, `env`, `printf`

**禁止的操作：**
- 网络命令：`curl`, `wget`, `ssh`, `scp`, `nc`, `nmap`, `ping`
- 权限提升：`sudo`
- Shell 操作符：`&&`, `||`, `|`, `;`, `>`, `<`, `` ` ``, `$()`
- 破坏性命令：`rm -rf /`
- 路径逃逸：绝对路径、`../`、`~/`

#### DangerFullAccess

无限制模式，允许执行任意命令。

### 安全特性

- 路径验证：所有路径参数都经过验证
- 命令白名单：WorkspaceWrite 模式下仅允许白名单程序
- 操作符过滤：禁止危险的 shell 操作符
- 超时保护：防止命令无限期运行

### 实现位置

[tools.rs:326-389](../openjax-core/src/tools.rs#L326-L389)

---

## 5. write_file

写入文件内容，支持创建新文件或覆盖已有文件。

### 参数

| 参数名 | 类型 | 必需 | 说明 |
|-------|------|------|------|
| `file_path` | string | 是 | 文件路径（相对工作区根目录） |
| `content` | string | 是 | 文件内容 |

### 使用示例

```bash
tool:write_file file_path=src/lib.rs content='fn main() {
    println!("Hello");
}'
```

### 输出格式

返回操作结果：

```
written src/lib.rs (42 bytes)
```

### 安全特性

- 路径验证：所有路径都经过验证
- 父目录不存在时自动创建
- 文件已存在时直接覆盖

### 实现位置

[handlers/write_file.rs](../openjax-core/src/tools/handlers/write_file.rs)

---

## 工具路由

所有工具调用通过 `ToolRouter` 统一分发：

```rust
match call.name.as_str() {
    "read_file" => read_file(call, cwd).await,
    "list_dir" => list_dir(call, cwd).await,
    "grep_files" => grep_files(call, cwd).await,
    "shell" => shell(call, cwd, config).await,
    "write_file" => write_file(call, cwd).await,
    _ => Err(anyhow!("unknown tool: {}", call.name))
}
```

### 实现位置

[tools.rs:121-164](../openjax-core/src/tools.rs#L121-L164)

---

## 环境变量配置

### 模型后端

```bash
# OpenAI API
export OPENAI_API_KEY="your-api-key"
export OPENJAX_MODEL="gpt-4.1-mini"
export OPENAI_BASE_URL="https://api.openai.com/v1"

# MiniMax API
export OPENJAX_MINIMAX_API_KEY="your-api-key"
export OPENJAX_MINIMAX_MODEL="codex-MiniMax-M2.1"
export OPENJAX_MINIMAX_BASE_URL="https://api.minimax.chat/v1"
```

### 运行时策略

```bash
# 审批策略
# 审批策略现由 PolicyRuntime 注入管理（见上方"审批策略"章节）

# 沙箱模式
export OPENJAX_SANDBOX_MODE="workspace_write"  # | danger_full_access
```

---

## 测试

所有工具都有对应的单元测试，位于 [tools.rs:1124-1327](../openjax-core/src/tools.rs#L1124-L1327)。

运行测试：

```bash
# 运行所有工具测试
zsh -c "cargo test -p openjax-core tools"

# 运行特定测试
zsh -c "cargo test -p openjax-core write_file_works"
```

---

## 相关文档

- [项目结构索引](project-structure-index.md)
- [安全文档](security.md)
- [配置文档](config.md)
- [Codex 架构参考](codex-architecture-reference.md)
