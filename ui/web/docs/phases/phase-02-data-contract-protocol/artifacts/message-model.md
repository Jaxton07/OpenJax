# Message Model Draft

## 目标
在不破坏现有文本渲染的前提下，引入结构化 ToolStep。

## 建议类型（草案）
- `ToolStepStatus`: `running | success | waiting | failed`
- `ToolStepType`: `think | tool | shell | approval | summary`
- `ToolStep`:
  - `id` (required)
  - `type` (required)
  - `title` (required)
  - `subtitle` (optional)
  - `status` (required)
  - `time` (required)
  - `description` (optional)
  - `code` (optional)
  - `output` (optional)
  - `delta` (optional)

## ChatMessage 扩展策略
- 保留 `content: string` 作为兼容字段。
- 新增可选 `toolSteps?: ToolStep[]`。
- 当 `toolSteps` 为空或不可用时，按原有文本路径渲染。
