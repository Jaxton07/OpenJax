# OpenJax 工具系统文档

欢迎来到 OpenJax 工具系统文档！本文档提供了工具系统的完整指南，从概述到高级扩展。

## 快速开始

如果你是第一次使用 OpenJax 工具系统，建议按以下顺序阅读：

1. [概述](overview.md) - 了解工具系统的核心特性和当前状态
2. [架构设计](architecture.md) - 了解工具系统的架构设计
3. [工具列表](tools-list.md) - 查看所有可用工具
4. [使用指南](usage-guide.md) - 学习如何使用工具系统

## 文档目录

### 核心文档

- [概述](overview.md) - 工具系统的概述、核心特性和当前状态
- [架构设计](architecture.md) - 工具系统的架构设计和模块结构
- [核心组件](core-components.md) - 核心组件的详细说明

### 使用文档

- [工具列表](tools-list.md) - 所有可用工具的详细说明和使用示例
- [使用指南](usage-guide.md) - 如何使用工具系统
- [最佳实践](best-practices.md) - 使用工具系统的最佳实践
- [故障排除](troubleshooting.md) - 常见问题和解决方案

### 扩展文档

- [扩展指南](extension-guide.md) - 如何扩展新工具
- [Hooks 系统](hooks-system.md) - Hooks 系统的详细说明
- [动态工具](dynamic-tools.md) - 动态工具支持

### 扩展门禁

新增工具时，必须先完成权限声明并通过接入校验：

- 新工具必须实现 `PolicyDescriptor`，或提供同义的权限声明能力
- 必须补齐 `allow`、`ask`/`escalate`、`deny` 三类测试
- 缺少权限声明不算接入完成，CI 应将其视为未通过门禁

### 安全文档

- [沙箱和批准](sandbox-and-approval.md) - 沙箱和批准机制

### 参考文档

- [参考资源](references.md) - 参考资源和未来扩展方向

## 按角色阅读

### 开发者

如果你想了解如何使用工具系统：

1. [概述](overview.md)
2. [工具列表](tools-list.md)
3. [使用指南](usage-guide.md)
4. [最佳实践](best-practices.md)

### 贡献者

如果你想为工具系统贡献代码：

1. [概述](overview.md)
2. [架构设计](architecture.md)
3. [核心组件](core-components.md)
4. [扩展指南](extension-guide.md)
5. [最佳实践](best-practices.md)

### 高级用户

如果你想深入了解工具系统：

1. [架构设计](architecture.md)
2. [核心组件](core-components.md)
3. [Hooks 系统](hooks-system.md)
4. [动态工具](dynamic-tools.md)
5. [沙箱和批准](sandbox-and-approval.md)

## 工具系统特性

OpenJax 工具系统提供以下核心特性：

- **统一的接口抽象**：所有工具实现相同的 `ToolHandler` trait
- **动态注册和分发**：支持运行时注册和分发工具
- **丰富的输出格式**：支持 Text 和 Json 输出，包含成功标志
- **工具规范定义**：JSON Schema 定义工具的输入和输出
- **Hooks 系统**：支持工具执行前后的钩子
- **集中的沙箱和批准管理**：统一的沙箱策略和批准流程
- **支持多种工具类型**：Function、Mcp、Custom、LocalShell
- **动态工具支持**：支持运行时注册自定义工具
- **Freeform 工具支持**：支持 Lark 语法定义的自由格式工具
- **模块化架构**：工具代码按功能拆分为独立模块，易于维护和扩展

## 与流式系统对齐

工具执行会通过统一事件流输出以下生命周期事件：

1. `tool_call_started`
2. `tool_args_delta`（可选）
3. `tool_call_progress`（可选）
4. `tool_call_completed` 或 `tool_call_failed`

审批事件 `approval_requested/approval_resolved` 与工具事件并流，便于 WebUI/TUI 单时间线渲染。

## 可用工具

当前系统包含以下工具：

- **grep_files** - 使用 ripgrep 进行高性能搜索
- **Read** - 读取文件内容，支持分页和缩进感知
- **list_dir** - 列出目录内容，支持递归和分页
- **Edit** - 在文件中唯一匹配并替换已有文本
- **shell** - 执行 shell 命令，支持批准和沙箱模式
- **process_snapshot** - 只读进程快照
- **system_load** - 只读系统负载与内存指标
- **disk_usage** - 只读磁盘空间指标

详细信息请参考 [工具列表](tools-list.md)。

## 快速示例

### 搜索代码

```bash
tool:grep_files pattern=fn main path=src include=*.rs
```

### 读取文件

```bash
tool:Read file_path=src/lib.rs offset=1 limit=50
```

### 列出目录

```bash
tool:list_dir dir_path=src depth=2
```

### 执行命令

```bash
tool:shell cmd='cargo test' require_escalated=true
```

### 精确文本替换

```bash
tool:Edit file_path=src/lib.rs old_string='let old_value = 1;' new_string='let old_value = 2;'
```

## 获取帮助

如果你在使用过程中遇到问题：

1. 查看 [故障排除](troubleshooting.md) 文档
2. 查看 [最佳实践](best-practices.md) 文档
3. 查看源代码中的注释和文档
4. 提交 Issue 报告问题

## 贡献

欢迎贡献新的工具和改进！详细信息请参考 [参考资源](references.md) 中的贡献指南。

## 相关文档

- [Codex 架构参考](../../../../docs/codex-architecture-reference.md) - Codex 架构详细说明
- [Codex 快速参考](../../../../docs/codex-quick-reference.md) - Codex 快速参考指南
- [项目结构索引](../../../../docs/project-structure-index.md) - 项目结构索引
