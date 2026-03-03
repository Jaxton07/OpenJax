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

## read_file

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
tool:read_file file_path=src/lib.rs offset=1 limit=50

# 使用缩进感知模式
tool:read_file file_path=src/lib.rs mode=indentation indentation='{"anchor_line": 100, "max_levels": 2}'

# 读取整个文件
tool:read_file file_path=README.md
```

## list_dir

列出目录内容，支持递归和分页。

### 功能

- 递归列出（depth 参数）
- 分页支持（offset 和 limit）
- 文件类型标记（/ 目录、@ 符号链接、? 其他）
- 缩进显示层级结构

### 参数

- `dir_path` (必需): 目录路径
- `offset` (可选): 起始条目号（1-indexed，默认：1）
- `limit` (可选): 最大条目数（默认：25）
- `depth` (可选): 最大递归深度（默认：2）

### 输出

- 目录条目，带缩进和类型标记
- 格式："<indent><name><type_marker>"

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
- apply_patch 命令拦截

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

## edit_file_range

按行范围精确编辑文件，适合小范围定点改动。

### 功能

- 1-indexed 行范围替换（`start_line..=end_line`）
- 用 `new_text` 替换指定行段
- 支持通过空字符串删除行段
- 保留原文件是否以换行结尾的行为

### 参数

- `file_path` (必需): 文件路径
- `start_line` (必需): 起始行号（1-indexed，包含）
- `end_line` (必需): 结束行号（1-indexed，包含）
- `new_text` (必需): 替换文本（可为空字符串）

### 输出

- 编辑摘要
- 格式：`edit applied successfully\nUPDATE <path>:<start>-<end>`

### 示例

```bash
# 将第 10-12 行替换为两行
tool:edit_file_range file_path=src/lib.rs start_line=10 end_line=12 new_text='new line a\nnew line b'

# 删除第 20-25 行
tool:edit_file_range file_path=src/lib.rs start_line=20 end_line=25 new_text=''
```

## apply_patch

应用补丁到工作区，支持添加、删除、移动、重命名、更新文件。参考 Codex 的实现进行了模块化重构，并添加了 Freeform 工具支持和多级模糊匹配算法。

### 功能

- **模块化架构**：代码拆分为 8 个独立模块，职责清晰
  - `types.rs`: 数据类型定义
  - `parser.rs`: 补丁解析逻辑
  - `heredoc.rs`: Heredoc 提取
  - `matcher.rs`: 匹配算法（含多级模糊匹配）
  - `applier.rs`: 补丁应用逻辑
  - `planner.rs`: 补丁规划逻辑
  - `tool.rs`: 工具入口
  - `grammar.lark`: Lark 语法文件（Freeform 工具）
- **Freeform 工具支持**：支持 Lark 语法定义的自由格式工具
- **多级模糊匹配**：类似 git apply 的智能匹配算法
  - Level 0: 精确匹配
  - Level 1: 忽略尾部空白
  - Level 2: 忽略首尾空白
  - Level 3: Unicode 标准化（破折号、智能引号等）
- **解析补丁格式**：支持多种操作
- **回滚机制**：失败时自动回滚
- **路径验证**：防止逃逸工作区根目录

### 参数

- `patch` (必需): 补丁文本

### 补丁格式

```
*** Begin Patch
[ one or more file operations ]
*** End Patch
```

### 支持的操作

- **Add File**: 添加新文件
  ```
  *** Add File: new_file.rs
  +// new file content
  ```
- **Delete File**: 删除文件
  ```
  *** Delete File: old_file.rs
  ```
- **Move File**: 移动文件
  ```
  *** Move File: old_file.rs -> new_file.rs
  ```
- **Rename File**: 重命名文件
  ```
  *** Rename File: old_name.rs -> new_name.rs
  ```
- **Update File**: 更新文件内容
  ```
  *** Update File: target.rs
  @@
   context line
   -old line
   +new line
   another context
  ```

### Update File 高级特性

- **上下文标记**：使用 `@@` 标记代码上下文
  ```
  *** Update File: src/lib.rs
  @@ fn main
   fn main() {
  -    println!("old");
  +    println!("new");
   }
  ```
- **移动并更新**：在更新时移动文件
  ```
  *** Update File: old.rs
  *** Move to: new.rs
  @@
  -old
  +new
  ```
- **文件末尾标记**：使用 `*** End of File` 标记文件末尾

### 输出

- 补丁应用摘要
- 格式："ADD <path>\nUPDATE <path>\nDELETE <path>\nMOVE <from> -> <to>"

### 示例

```bash
# 添加新文件
tool:apply_patch patch='*** Begin Patch\n*** Add File: new.rs\n+// new content\n*** End Patch'

# 更新文件
tool:apply_patch patch='*** Begin Patch\n*** Update File: src/lib.rs\n@@ fn main\n fn main() {\n-    println!("old");\n+    println!("new");\n }\n*** End Patch'

# 移动并更新
tool:apply_patch patch='*** Begin Patch\n*** Update File: old.rs\n*** Move to: new.rs\n@@\n-old\n+new\n*** End Patch'

# 删除文件
tool:apply_patch patch='*** Begin Patch\n*** Delete File: obsolete.rs\n*** End Patch'

# 复杂补丁（多个操作）
tool:apply_patch patch='*** Begin Patch\n*** Add File: new.rs\n+// new\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** Delete File: old.rs\n*** End Patch'
```

### 模块结构

```
openjax-core/src/tools/apply_patch/
├── mod.rs              # 模块导出
├── types.rs            # 数据类型定义
├── parser.rs           # 补丁解析逻辑
├── heredoc.rs          # Heredoc 提取
├── matcher.rs          # 匹配算法（多级模糊匹配）
├── applier.rs          # 补丁应用逻辑
├── planner.rs          # 补丁规划逻辑
├── tool.rs             # 工具入口
└── grammar.lark        # Lark 语法文件（Freeform 工具）
```

### Freeform 工具支持

apply_patch 支持 Freeform 工具格式，使用 Lark 语法定义补丁语法。Freeform 工具允许 AI 模型直接输出补丁内容，无需 JSON 包装。

**Lark 语法文件**：[grammar.lark](../apply_patch/grammar.lark)

**Freeform 工具类型**：
- `type`: "grammar"
- `syntax`: "lark"
- `definition`: Lark 语法定义字符串

### 多级模糊匹配算法

为了提高补丁应用成功率，实现了类似 `git apply` 的多级模糊匹配算法：

1. **精确匹配**（Level 0）：完全相等
2. **忽略尾部空白**（Level 1）：`trim_end()`
3. **忽略首尾空白**（Level 2）：`trim()`
4. **Unicode 标准化**（Level 3）：转换 Unicode 标点符号
   - 各种破折号 → `-`
   - 智能引号 → `'` 和 `"`
   - 不换行空格 → 普通空格

### 与 Codex 的对比

| 特性 | Codex | OpenJax |
|------|--------|---------|
| 代码拆分 | ✅ 独立 crate | ✅ 模块化目录 |
| Freeform 支持 | ✅ Lark 语法 | ✅ Lark 语法 |
| 多级模糊匹配 | ✅ 4 级匹配 | ✅ 4 级匹配 |
| Unicode 标准化 | ✅ | ✅ |
| Heredoc 支持 | ✅ | ✅ |
| 回滚机制 | ✅ | ✅ |

## 工具对比

| 工具 | 变异操作 | 超时 | 沙箱支持 | 主要用途 |
|------|---------|------|---------|---------|
| grep_files | 否 | 30s | 是 | 搜索文件内容 |
| read_file | 否 | 无 | 是 | 读取文件 |
| list_dir | 否 | 无 | 是 | 列出目录 |
| edit_file_range | 是 | 无 | 是 | 按行范围精确编辑 |
| shell | 是 | 可配置 | 是 | 执行命令 |
| apply_patch | 是 | 无 | 是 | 应用补丁 |

## 相关文档

- [使用指南](usage-guide.md) - 学习如何使用这些工具
- [扩展指南](extension-guide.md) - 学习如何添加新工具
