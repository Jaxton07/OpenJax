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

- `handlers/`: 通用工具处理器（read_file/list_dir/grep_files/shell/apply_patch/edit_file_range）
- `system/`: 系统类只读工具（process_snapshot/system_load/disk_usage）
- `apply_patch/`: apply_patch 解析与执行子模块
- `orchestrator.rs`: 工具编排与审批事件联动
- `spec.rs`: 工具 schema 定义
- `tool_builder.rs`: 默认工具注册

## 流式事件约定（与 gateway/webui 对齐）

一次工具调用生命周期事件：

1. `ToolCallStarted`
2. `ToolCallArgsDelta`（可选，多次）
3. `ToolCallProgress`（可选，多次）
4. `ToolCallCompleted` 或 `ToolCallFailed`

审批事件 `ApprovalRequested/ApprovalResolved` 与上述事件在同一 turn 时间线并流输出。

## 迁移说明

工具文档统一维护在 `openjax-core/src/tools/docs/`。如与其他旧文档存在冲突，请以该目录内容为准。
