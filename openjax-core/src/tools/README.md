# OpenJax 工具系统

工具系统文档主目录：[`openjax-core/src/tools/docs/`](./docs/README.md)

## 快速链接

- [文档首页](./docs/README.md)
- [概述](./docs/overview.md)
- [架构设计](./docs/architecture.md)
- [工具列表](./docs/tools-list.md)
- [扩展指南](./docs/extension-guide.md)
- [沙箱和批准](./docs/sandbox-and-approval.md)

## 代码结构

- `handlers/`: 通用工具处理器（read/list_dir/grep_files/glob_files/shell/edit/write_file）
- `system/`: 系统类只读工具（process_snapshot/system_load/disk_usage）
- `orchestrator.rs`: 工具编排与审批事件联动
- `spec.rs`: 工具 schema 定义
- `tool_builder.rs`: 默认工具注册

## 当前受支持工具面

- 文件/代码工具：`Read`、`list_dir`、`grep_files`、`glob_files`、`write_file`、`Edit`
- 命令执行工具：`shell`（含兼容别名 `exec_command`）
- 系统观测工具：`process_snapshot`、`system_load`、`disk_usage`

上述工具通过 `tool_builder.rs` 和 `spec.rs` 统一注册与暴露，native tool calling 请求直接消费该工具面。

## 流式事件约定（与 gateway/webui 对齐）

一次工具调用生命周期事件：

1. `ToolCallStarted`
2. `ToolCallArgsDelta`（可选，多次）
3. `ToolCallProgress`（可选，多次）
4. `ToolCallCompleted` 或 `ToolCallFailed`

审批事件 `ApprovalRequested/ApprovalResolved` 与上述事件在同一 turn 时间线并流输出。

## shell 结果语义（Phase 5 已完成）

- 模型通道：使用 `model_content` 作为 `tool_result` 内容，避免把展示元数据注入模型上下文。
- 展示通道：使用 `display_output`（含完整可观测文本）和 `shell_metadata`（结构化字段）供事件/UI 消费。
- 该拆分由 `ToolExecOutcome` 统一承载，planner/tool_action 按通道职责消费对应字段。

## 迁移说明

工具文档统一维护在 `openjax-core/src/tools/docs/`。如与其他旧文档存在冲突，请以该目录内容为准。
