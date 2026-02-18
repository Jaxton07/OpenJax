# OpenJax 工具文档

本文档详细描述了 OpenJax 中所有已实现的工具及其使用方法。

## 工具概览

| 工具名称 | 功能描述 | 安全级别 |
|---------|---------|---------|
| `read_file` | 读取文件内容 | 只读 |
| `list_dir` | 列出目录内容 | 只读 |
| `grep_files` | 递归搜索文件 | 只读 |
| `exec_command` | 执行 shell 命令 | 可配置 |
| `apply_patch` | 应用文件补丁 | 可配置 |

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

## 4. exec_command

执行 shell 命令，支持沙箱模式和审批策略。

### 参数

| 参数名 | 类型 | 必需 | 说明 |
|-------|------|------|------|
| `cmd` | string | 是 | 要执行的 shell 命令 |
| `require_escalated` | boolean | 否 | 是否需要审批，默认为 `false` |
| `timeout_ms` | number | 否 | 超时时间（毫秒），默认为 `30000` |

### 使用示例

```bash
tool:exec_command cmd='zsh -c "cargo test"' require_escalated=true timeout_ms=60000
tool:exec_command cmd='ls -la' timeout_ms=5000
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

通过环境变量 `OPENJAX_APPROVAL_POLICY` 配置：

| 策略 | 说明 |
|-----|------|
| `always_ask` | 总是询问用户（默认） |
| `on_request` | 仅在 `require_escalated=true` 时询问 |
| `never` | 从不询问 |

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

## 5. apply_patch

应用文件补丁，支持多种文件操作。

### 参数

| 参数名 | 类型 | 必需 | 说明 |
|-------|------|------|------|
| `patch` | string | 是 | 补丁文本 |

### 补丁格式

补丁必须以 `*** Begin Patch` 开始，以 `*** End Patch` 结束。

#### 5.1 Add File - 添加新文件

```
*** Begin Patch
*** Add File: path/to/file.txt
+第一行内容
+第二行内容
*** End Patch
```

#### 5.2 Delete File - 删除文件

```
*** Begin Patch
*** Delete File: path/to/file.txt
*** End Patch
```

#### 5.3 Update File - 更新文件

使用 hunk 格式进行精确更新：

```
*** Begin Patch
*** Update File: path/to/file.txt
@@
 上下文行1
-要删除的行
+要添加的行
 上下文行2
@@
 另一个 hunk
-删除的行
+添加的行
*** End Patch
```

**Hunk 格式说明：**
- `@@`：标记 hunk 开始
- ` `（空格）：上下文行（必须匹配）
- `-`：删除的行（必须匹配）
- `+`：添加的行

#### 5.4 Move File - 移动文件

```
*** Begin Patch
*** Move File: from.txt -> to.txt
*** End Patch
```

#### 5.5 Rename File - 重命名文件

```
*** Begin Patch
*** Rename File: old.txt -> new.txt
*** End Patch
```

### 使用示例

```bash
tool:apply_patch patch='*** Begin Patch
*** Add File: new.rs
+// new file
+fn main() {
+    println!("Hello");
+}
*** End Patch'
```

### 输出格式

返回操作摘要，每行一个操作：

```
patch applied successfully
ADD new.rs
UPDATE src/lib.rs
DELETE old.txt
MOVE from.txt -> to.txt
```

### 回滚机制

如果在应用补丁过程中任何操作失败，所有已应用的操作都会自动回滚，确保工作区状态一致。

### 安全特性

- 路径验证：所有路径都经过验证
- 重复操作检查：同一文件不能有多个操作
- 存在性检查：
  - Add File：目标文件不能已存在
  - Delete File：目标文件必须存在且为文件
  - Update File：目标文件必须存在
  - Move/Rename File：源文件必须存在，目标文件不能已存在
- Hunk 匹配验证：Update File 的上下文必须完全匹配

### 实现位置

[tools.rs:391-821](../openjax-core/src/tools.rs#L391-L821)

---

## 工具路由

所有工具调用通过 `ToolRouter` 统一分发：

```rust
match call.name.as_str() {
    "read_file" => read_file(call, cwd).await,
    "list_dir" => list_dir(call, cwd).await,
    "grep_files" => grep_files(call, cwd).await,
    "exec_command" => exec_command(call, cwd, config).await,
    "apply_patch" => apply_patch_tool(call, cwd).await,
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
export OPENJAX_APPROVAL_POLICY="always_ask"  # | on_request | never

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
zsh -c "cargo test -p openjax-core apply_patch_add_file_works"
```

---

## 相关文档

- [项目结构索引](project-structure-index.md)
- [安全文档](security.md)
- [配置文档](config.md)
- [Codex 架构参考](codex-architecture-reference.md)
