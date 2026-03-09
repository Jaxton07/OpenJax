# Message Model (Frozen v1)

## 目标
- 在不破坏现有文本渲染的前提下，引入结构化 `ToolStep`，支撑步骤卡片渲染。

## 类型定义（v1 冻结）
- `ToolStepStatus`: `running | success | waiting | failed`
- `ToolStepType`: `think | tool | shell | approval | summary`
- `ToolStep`:
  - `id` (required): step 主键。优先 `tool_call_id`；缺失时使用 `turn_id + event_seq`。
  - `type` (required): 步骤类型，决定图标与子文案。
  - `title` (required): 头部主标题；缺失时默认 `tool`。
  - `subtitle` (optional): 次级信息（如路径、命令来源）。
  - `status` (required): 当前状态。
  - `time` (required): 事件时间戳（ISO 字符串）。
  - `description` (optional): 步骤说明。
  - `code` (optional): 执行命令或调用片段。
  - `output` (optional): 输出摘要（可截断）。
  - `delta` (optional): 变更摘要（如 `+8 -0`）。
  - `approvalId` (optional): 当 `type=approval` 时记录审批主键。
  - `toolCallId` (optional): 记录工具调用 id（便于追踪/聚合）。
  - `meta` (optional): 扩展字段，类型 `Record<string, unknown>`。

## ChatMessage 扩展策略
- 保留 `content: string` 作为兼容字段（不可删除）。
- 新增可选 `toolSteps?: ToolStep[]`。
- 渲染优先级:
  - `toolSteps` 非空: 渲染结构化步骤流，并保留 `content` 作为辅助文本。
  - `toolSteps` 为空或不可用: 完全按旧文本路径渲染。

## 默认值与约束
- 缺失 `title` -> `tool`
- 缺失 `status` -> `running`
- 缺失 `time` -> 使用事件 `timestamp`
- `output` 建议在 UI 层做长度限制，原始值保留在 state。
- `toolSteps` 必须按消息内展示顺序排序（不在渲染层重新排序）。
