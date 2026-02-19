# Codex vs OpenJax 工具对比

本文档对比 Codex 和 OpenJax 中相同工具的实现差异，帮助识别改进方向。

---

## 1. read_file

### Codex 实现

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `file_path` | string | - | 绝对路径 |
| `offset` | number | 1 | 1-indexed 起始行号 |
| `limit` | number | 2000 | 最大行数 |
| `mode` | enum | `Slice` | `Slice` 或 `Indentation` |
| `indentation` | object | - | 缩进配置（Indentation 模式） |
| `indentation.anchor_line` | number | offset | 锚点行号 |
| `indentation.max_levels` | number | 0 | 最大缩进深度（0=无限制） |
| `indentation.include_siblings` | boolean | false | 是否包含同级块 |
| `indentation.include_header` | boolean | true | 是否包含头部注释 |
| `indentation.max_lines` | number | limit | 硬限制 |

**输出格式：**
```
L1: fn main() {
L2:     println!("Hello");
L3: }
```

**特性：**
- ✅ 分页读取（offset + limit）
- ✅ 缩进感知读取（Indentation 模式）
- ✅ 自动识别代码块边界
- ✅ 跳过空行
- ✅ 识别注释（`#`, `//`, `--`）
- ✅ 行号显示
- ✅ 超长行截断（500 字符）
- ✅ CRLF 处理
- ✅ 非 UTF8 字符处理

**实现位置：**
[codex-rs/core/src/tools/handlers/read_file.rs](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/read_file.rs)

### OpenJax 实现

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `path` | string | - | 相对路径 |

**输出格式：**
```
fn main() {
    println!("Hello");
}
```

**特性：**
- ✅ 简单读取整个文件
- ✅ 路径验证（相对路径、禁止 `../`）
- ❌ 无分页支持
- ❌ 无缩进感知
- ❌ 无行号显示
- ❌ 无超长行截断

**实现位置：**
[openjax-core/src/tools.rs:253-265](../openjax-core/src/tools.rs#L253-L265)

### 差异总结

| 特性 | Codex | OpenJax |
|-----|-------|---------|
| 分页读取 | ✅ | ❌ |
| 缩进感知 | ✅ | ❌ |
| 行号显示 | ✅ | ❌ |
| 超长行截断 | ✅ | ❌ |
| 路径类型 | 绝对路径 | 相对路径 |
| 注释识别 | ✅ | ❌ |

### 改进建议

1. **添加 offset 和 limit 参数**：支持分页读取大文件
2. **添加行号显示**：便于定位代码
3. **添加超长行截断**：防止输出过长
4. **考虑添加缩进感知模式**：智能读取代码块

---

## 2. list_dir

### Codex 实现

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `dir_path` | string | - | 绝对路径 |
| `offset` | number | 1 | 1-indexed 起始条目 |
| `limit` | number | 25 | 最大条目数 |
| `depth` | number | 2 | 递归深度 |

**输出格式：**
```
Absolute path: /path/to/dir
nested/
  child.txt
  deeper/
    grandchild.txt
root.txt
```

**文件类型标记：**
- `/` - 目录
- `@` - 符号链接
- `?` - 其他类型
- 无后缀 - 普通文件

**特性：**
- ✅ 递归列出目录（depth 控制）
- ✅ 分页支持（offset + limit）
- ✅ 按名称排序
- ✅ 文件类型标记
- ✅ 缩进显示层级
- ✅ 超长条目名截断（500 字符）
- ✅ 显示绝对路径
- ✅ 超过限制时提示

**实现位置：**
[codex-rs/core/src/tools/handlers/list_dir.rs](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/list_dir.rs)

### OpenJax 实现

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `path` | string | `.` | 相对路径 |

**输出格式：**
```
child.txt
deeper
grandchild.txt
nested
root.txt
```

**特性：**
- ✅ 列出当前目录（不递归）
- ✅ 按字母排序
- ❌ 无递归支持
- ❌ 无分页支持
- ❌ 无文件类型标记
- ❌ 无缩进显示
- ❌ 无超长条目名截断

**实现位置：**
[openjax-core/src/tools.rs:267-286](../openjax-core/src/tools.rs#L267-L286)

### 差异总结

| 特性 | Codex | OpenJax |
|-----|-------|---------|
| 递归列出 | ✅ | ❌ |
| 分页支持 | ✅ | ❌ |
| 文件类型标记 | ✅ | ❌ |
| 缩进显示 | ✅ | ❌ |
| 超长条目名截断 | ✅ | ❌ |
| 显示绝对路径 | ✅ | ❌ |
| 路径类型 | 绝对路径 | 相对路径 |

### 改进建议

1. **添加 depth 参数**：支持递归列出目录
2. **添加 offset 和 limit 参数**：支持分页
3. **添加文件类型标记**：便于识别目录、符号链接等
4. **添加缩进显示**：清晰展示层级结构
5. **添加超长条目名截断**：防止输出过长

---

## 3. grep_files

### Codex 实现

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `pattern` | string | - | 搜索模式 |
| `include` | string | - | glob 模式过滤（可选） |
| `path` | string | - | 搜索路径（可选） |
| `limit` | number | 100 | 最大结果数（最大 2000） |

**输出格式：**
```
/path/to/file_a.rs
/path/to/file_b.rs
```

**特性：**
- ✅ 使用 ripgrep (`rg`) 命令
- ✅ 支持正则表达式
- ✅ 按修改时间倒序排序
- ✅ 支持 glob 模式过滤
- ✅ 分页支持（limit）
- ✅ 30 秒超时
- ✅ 仅返回文件路径（不返回行号和内容）
- ✅ 无匹配时返回 `No matches found.`

**实现位置：**
[codex-rs/core/src/tools/handlers/grep_files.rs](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/grep_files.rs)

### OpenJax 实现

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `pattern` | string | - | 搜索模式 |
| `path` | string | `.` | 搜索路径 |

**输出格式：**
```
file_a.rs:1:fn main() {
file_b.rs:2:    println!("Hello");
```

**特性：**
- ✅ 使用 Rust 的 walkdir 和字符串匹配
- ✅ 递归搜索所有文件
- ✅ 返回行号和内容
- ❌ 不支持正则表达式（仅简单字符串匹配）
- ❌ 性能较差（逐文件读取）
- ❌ 无 glob 模式过滤
- ❌ 无分页支持
- ❌ 无超时控制
- ❌ 无匹配时返回 `(no matches)`

**实现位置：**
[openjax-core/src/tools.rs:288-324](../openjax-core/src/tools.rs#L288-L324)

### 差异总结

| 特性 | Codex | OpenJax |
|-----|-------|---------|
| 搜索引擎 | ripgrep | walkdir + 字符串匹配 |
| 正则表达式 | ✅ | ❌ |
| 性能 | 高 | 低 |
| glob 过滤 | ✅ | ❌ |
| 分页支持 | ✅ | ❌ |
| 超时控制 | ✅ | ❌ |
| 返回内容 | 文件路径 | 文件路径:行号:内容 |
| 排序 | 按修改时间 | 按文件路径 |

### 改进建议

1. **使用 ripgrep**：大幅提升性能，支持正则表达式
2. **添加 glob 模式过滤**：按文件类型过滤
3. **添加 limit 参数**：支持分页
4. **添加超时控制**：防止长时间搜索
5. **考虑返回格式**：Codex 仅返回文件路径，OpenJax 返回行号和内容，各有优劣

---

## 4. shell

### Codex 实现

**工具类型：**
- `shell` - 标准 shell 执行
- `shell_command` - 使用会话 shell（保持状态）
- `exec` - 统一执行接口
- `local_shell` - 本地 shell 执行
- `container.exec` - 容器内执行

**参数（shell）：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `command` | string | - | 命令 |
| `workdir` | string | - | 工作目录（可选） |
| `timeout_ms` | number | - | 超时时间（可选） |
| `sandbox_permissions` | enum | - | 沙箱权限（可选） |
| `justification` | string | - | 理由（可选） |
| `prefix_rule` | array | - | 前缀规则（可选） |

**参数（shell_command）：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `command` | string | - | 命令 |
| `workdir` | string | - | 工作目录（可选） |
| `login` | boolean | true | 是否使用登录 shell |
| `timeout_ms` | number | - | 超时时间（可选） |
| `sandbox_permissions` | enum | - | 沙箱权限（可选） |
| `justification` | string | - | 理由（可选） |
| `prefix_rule` | array | - | 前缀规则（可选） |

**特性：**
- ✅ 支持多种 shell 类型（Bash、Zsh、PowerShell）
- ✅ 支持登录 shell 和非登录 shell
- ✅ 支持会话 shell（保持状态）
- ✅ 支持沙箱权限管理
- ✅ 支持网络权限控制
- ✅ 支持命令前缀规则
- ✅ 支持 apply_patch 拦截
- ✅ 支持审批策略
- ✅ 支持安全命令检测（`is_known_safe_command`）
- ✅ 支持事件发射（开始/结束）
- ✅ 支持工具编排器（ToolOrchestrator）
- ✅ 支持依赖环境变量注入

**实现位置：**
[codex-rs/core/src/tools/handlers/shell.rs](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/shell.rs)

### OpenJax 实现

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `cmd` | string | - | 命令 |
| `require_escalated` | boolean | false | 是否需要审批 |
| `timeout_ms` | number | 30000 | 超时时间（毫秒） |

**特性：**
- ✅ 仅支持 zsh
- ✅ 支持审批策略
- ✅ 支持沙箱模式（WorkspaceWrite 和 DangerFullAccess）
- ✅ 路径验证
- ✅ 命令白名单（WorkspaceWrite 模式下）
- ❌ 不支持其他 shell 类型
- ❌ 不支持会话 shell
- ❌ 不支持网络权限控制
- ❌ 不支持命令前缀规则
- ❌ 不支持安全命令检测
- ❌ 不支持事件发射
- ❌ 不支持工具编排器

**实现位置：**
[openjax-core/src/tools.rs:326-389](../openjax-core/src/tools.rs#L326-L389)

### 差异总结

| 特性 | Codex | OpenJax |
|-----|-------|---------|
| Shell 类型 | Bash/Zsh/PowerShell | 仅 Zsh |
| 会话 shell | ✅ | ❌ |
| 登录 shell | ✅ | ❌ |
| 沙箱权限管理 | ✅ | ✅ |
| 网络权限控制 | ✅ | ❌ |
| 命令前缀规则 | ✅ | ❌ |
| 安全命令检测 | ✅ | ❌ |
| 事件发射 | ✅ | ❌ |
| 工具编排器 | ✅ | ❌ |
| apply_patch 拦截 | ✅ | ❌ |

### 改进建议

1. **支持多种 shell 类型**：至少支持 Bash 和 PowerShell
2. **添加会话 shell 支持**：保持 shell 状态（如环境变量、别名）
3. **添加登录 shell 支持**：加载用户配置
4. **添加网络权限控制**：允许/禁止网络访问
5. **添加命令前缀规则**：限制可执行的命令前缀
6. **添加安全命令检测**：自动识别安全命令，无需审批
7. **添加事件发射**：记录工具执行的开始和结束
8. **添加工具编排器**：支持更复杂的执行流程

---

## 架构差异

### Codex 架构

```
ToolRouter (路由)
    ↓
ToolRegistry (注册表)
    ↓
ToolOrchestrator (编排器)
    ↓
ToolHandler (处理器)
    ↓
ToolRuntime (运行时)
```

**核心组件：**
- `ToolRouter` - 路由 API 响应到工具调用
- `ToolRegistry` - 工具注册表，处理器分发
- `ToolOrchestrator` - 沙箱选择、审批流程、重试逻辑
- `ToolHandler` - 工具处理器 trait
- `ToolRuntime` - 工具运行时

**特性：**
- ✅ 清晰的分层架构
- ✅ 可扩展的注册机制
- ✅ 统一的编排器
- ✅ 支持多种运行时
- ✅ 支持钩子（BeforeToolUse、AfterToolUse）

### OpenJax 架构

```
ToolRouter (路由)
    ↓
直接执行
```

**核心组件：**
- `ToolRouter` - 工具路由器
- 各工具函数 - 直接实现

**特性：**
- ✅ 简单直接
- ❌ 缺少注册机制
- ❌ 缺少编排器
- ❌ 缺少运行时抽象
- ❌ 缺少钩子支持

### 改进建议

1. **添加 ToolRegistry**：支持工具注册和发现
2. **添加 ToolOrchestrator**：统一处理审批、沙箱、重试逻辑
3. **添加 ToolRuntime 抽象**：支持多种运行时
4. **添加钩子支持**：在工具执行前后插入自定义逻辑
5. **添加遥测支持**：记录工具执行指标

---

## 总结

### 优先级改进建议

**高优先级：**
1. ✅ `grep_files` 使用 ripgrep：大幅提升性能
2. ✅ `read_file` 添加 offset 和 limit：支持分页读取
3. ✅ `list_dir` 添加 depth 参数：支持递归列出
4. ✅ `shell` 添加安全命令检测：减少不必要的审批

**中优先级：**
5. ✅ `read_file` 添加行号显示：便于定位代码
6. ✅ `list_dir` 添加文件类型标记：便于识别文件类型
7. ✅ `shell` 支持多种 shell 类型：提高兼容性
8. ✅ 添加 ToolOrchestrator：统一处理审批和沙箱逻辑

**低优先级：**
9. ✅ `read_file` 添加缩进感知模式：智能读取代码块
10. ✅ `shell` 添加会话 shell：保持 shell 状态
11. ✅ 添加钩子支持：扩展工具执行流程
12. ✅ 添加遥测支持：记录工具执行指标

### 参考实现

- [Codex Tool System](/Users/ericw/work/code/ai/codex/docs/tool-system.md)
- [Codex Architecture Reference](/Users/ericw/work/code/ai/openJax/docs/codex-architecture-reference.md)
