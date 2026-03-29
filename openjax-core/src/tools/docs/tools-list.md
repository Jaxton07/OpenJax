# 工具列表

本文档列出了 OpenJax 工具系统中的所有工具及其使用方法。

## grep_files

使用 ripgrep 进行高性能搜索。

### 功能

- 正则表达式搜索
- Glob 过滤（如 `*.rs`）
- 分页支持（limit 参数）
- 30 秒超时控制

### 参数

- `pattern` (必需): 正则表达式模式
- `include` (可选): Glob 过滤模式
- `path` (可选): 搜索目录（默认：当前目录）
- `limit` (可选): 最大结果数（默认：100，最大：2000）

### 输出

- 匹配的文件路径列表，每行一个
- 如果没有匹配，返回 "No matches found."

### 示例

```bash
# 搜索包含 "fn main" 的 Rust 文件
tool:grep_files pattern=fn main path=src include=*.rs limit=10

# 搜索所有文件中的 "TODO"
tool:grep_files pattern=TODO

# 搜索特定目录
tool:grep_files pattern=error path=src/tools
```

## Read

读取文件内容，支持分页和缩进感知。

### 功能

- 分页读取（offset 和 limit）
- 显示行号（L1: content 格式）
- 超长行截断（500 字符）
- 缩进感知模式

### 参数

- `file_path` (必需): 文件路径
- `offset` (可选): 起始行号（1-indexed，默认：1）
- `limit` (可选): 最大行数（默认：2000）
- `mode` (可选): 读取模式（"slice" 或 "indentation"，默认："slice"）
- `indentation` (可选): 缩进感知选项（仅当 mode="indentation" 时使用）

### 输出

- 文件内容，每行格式为 "L<line_number>: <content>"
- 支持缩进感知模式，返回上下文相关的行

### 示例

```bash
# 读取文件的前 50 行
tool:Read file_path=src/lib.rs offset=1 limit=50

# 使用缩进感知模式
tool:Read file_path=src/lib.rs mode=indentation indentation='{"anchor_line": 100, "max_levels": 2}'

# 读取整个文件
tool:Read file_path=README.md
```

## list_dir

列出目录内容，支持递归和分页。

### 功能

- 递归列出（depth 参数）
- 分页支持（offset 和 limit）
- 文件类型标记（/ 目录、@ 符号链接、? 其他）
- 文件条目附带大小（如 `1.2 KB`）
- 空目录返回 `(empty directory)`
- 缩进显示层级结构

### 参数

- `dir_path` (必需): 目录路径
- `offset` (可选): 起始条目号（1-indexed，默认：1）
- `limit` (可选): 最大条目数（默认：25）
- `depth` (可选): 最大递归深度（默认：2）

### 输出

- 文件条目：`<indent><name>  (<size>)`
- 目录条目：`<indent><name>/`
- 符号链接：`<indent><name>@`
- 空目录：`(empty directory)`

### 示例

```bash
# 列出当前目录
tool:list_dir dir_path=.

# 递归列出 src 目录
tool:list_dir dir_path=src depth=3

# 分页列出
tool:list_dir dir_path=src offset=1 limit=50
```

## shell

执行 shell 命令，支持批准和沙箱模式。支持 Bash、Zsh、PowerShell 三种 shell。

### 功能

- 执行 shell 命令（Bash/Zsh/PowerShell）
- 支持批准策略
- 沙箱模式限制
- 自动检测用户 shell 类型

### 参数

- `cmd` (必需): 要执行的命令
- `require_escalated` (可选): 是否需要提升权限（默认：false）
- `timeout_ms` (可选): 超时时间（默认：30000ms）

### 输出

- 命令执行结果
- 输出包含 `result_class/command/exit_code/backend/degrade_reason/policy_decision/runtime_allowed/runtime_deny_reason/stdout/stderr`

### 沙箱限制

- **WorkspaceWrite**: 由 `openjax-core/src/sandbox/` 中的策略、能力映射与 runtime 后端决定
- **DangerFullAccess**: 无限制

### 示例

```bash
# 运行测试
tool:shell cmd='cargo test' require_escalated=true timeout_ms=60000

# 列出文件
tool:shell cmd='ls -la'

# 搜索文件
tool:shell cmd='rg "pattern" src/'
```

## process_snapshot

只读采集进程快照，避免直接使用 `ps/top` 的平台差异和沙箱拒绝风险。

### 参数

- `sort_by` (可选): 排序字段，`cpu` 或 `memory`（默认：`cpu`）
- `limit` (可选): 最大返回条数（默认：10，范围：1..=100）
- `user` (可选): 用户名过滤

### 输出

- JSON 对象：
  - `timestamp`
  - `host`
  - `items[]`: `{ pid, name, cpu_pct, memory_bytes, memory_pct, user, status }`
  - `meta`: `{ sort_by, limit, sampled_at_ms }`

### 示例

```bash
tool:process_snapshot sort_by=cpu limit=10
tool:process_snapshot sort_by=memory limit=5 user=ericw
```

## system_load

只读采集主机负载指标。

### 参数

- `include_cpu` (可选): 是否包含 CPU 指标（默认：true）
- `include_memory` (可选): 是否包含内存指标（默认：true）

### 输出

- JSON 对象：
  - `timestamp`
  - `cpu`: `{ logical_cores, usage_pct }`（可选）
  - `memory`: `{ total_bytes, used_bytes, used_pct, swap_total_bytes, swap_used_bytes }`（可选）
  - `load_avg`: `{ one, five, fifteen }`

### 示例

```bash
tool:system_load include_cpu=true include_memory=true
tool:system_load include_cpu=false include_memory=true
```

## disk_usage

只读采集磁盘/挂载点空间指标。

### 参数

- `path` (可选): 目标路径（默认：当前工作目录）
- `include_all_mounts` (可选): 是否返回全部挂载点（默认：false）

### 输出

- JSON 对象：
  - `timestamp`
  - `selected_path`
  - `items[]`: `{ mount_point, fs_name, total_bytes, available_bytes, used_bytes, used_pct }`

### 示例

```bash
tool:disk_usage
tool:disk_usage path=. include_all_mounts=false
tool:disk_usage include_all_mounts=true
```

## Edit

在文件中替换唯一匹配的一段已有文本，适合单文件精确替换。

### 功能

- 使用 `old_string` 在文件中查找匹配文本并替换
- 要求匹配结果唯一（0 次或多次匹配都会失败）
- 使用 `new_string` 作为替换文本
- 兼容不同换行符（LF/CRLF）

### 参数

- `file_path` (必需): 文件路径
- `old_string` (必需): 要被替换的原始文本（必须非空，且唯一匹配）
- `new_string` (必需): 新文本

### 输出

- 成功时返回更新摘要
- 失败时返回 `Edit failed [<class>]` 及原因（例如 `not_found`、`not_unique`、`invalid_args`）

### 示例

```bash
# 唯一命中后进行替换
tool:Edit file_path=src/lib.rs old_string='let retries = 2;' new_string='let retries = 3;'

# 删除一段文本（替换为空）
tool:Edit file_path=src/lib.rs old_string='debug!("temp");\n' new_string=''
```

## 工具对比

| 工具 | 变异操作 | 超时 | 沙箱支持 | 主要用途 |
|------|---------|------|---------|---------|
| grep_files | 否 | 30s | 是 | 搜索文件内容 |
| Read | 否 | 无 | 是 | 读取文件 |
| list_dir | 否 | 无 | 是 | 列出目录 |
| Edit | 是 | 无 | 是 | 唯一匹配文本替换 |
| shell | 是 | 可配置 | 是 | 执行命令 |

## 相关文档

- [使用指南](usage-guide.md) - 学习如何使用这些工具
- [扩展指南](extension-guide.md) - 学习如何添加新工具
