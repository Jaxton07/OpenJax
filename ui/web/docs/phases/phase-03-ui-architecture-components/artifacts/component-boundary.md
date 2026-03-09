# Component Boundary Plan (Frozen v1)

## 目标
- 在 `MessageList` 中新增结构化渲染分支，且不影响现有纯文本消息路径。

## 组件拆分
- `ToolStepList`
  - 输入: `steps: ToolStep[]`
  - 职责: 负责步骤列表渲染与 key 稳定性，不处理业务聚合。
- `ToolStepCard`
  - 输入: `step: ToolStep`、`defaultExpanded?: boolean`
  - 职责: 头部 + 展开容器；本地维护展开状态（或由上层受控，v1 建议本地状态）。
- `StepStatusBadge`
  - 输入: `status: ToolStepStatus`
  - 职责: 状态文案和状态类映射，禁止在上层重复写状态样式逻辑。
- `StepBody`
  - 输入: `description?`、`code?`、`output?`
  - 职责: 详情区块渲染；缺失字段时不渲染对应模块。

## MessageList 集成规则
- 新增分支:
  - 若 `message.toolSteps` 为非空数组，渲染 `ToolStepList`。
  - `message.content` 非空时，作为步骤流前置/后置辅助文本（由 Track B 定位）。
- 保持旧分支:
  - 无 `toolSteps` 时沿用 `message.content` 文本气泡。

## 目录与文件建议
- `ui/web/src/components/tool-steps/ToolStepList.tsx`
- `ui/web/src/components/tool-steps/ToolStepCard.tsx`
- `ui/web/src/components/tool-steps/StepStatusBadge.tsx`
- `ui/web/src/components/tool-steps/StepBody.tsx`

## 边界约束
- 展示组件不直接依赖 gateway event 或 reducer。
- 数据聚合、主键幂等处理必须留在状态层（Track A）。
- 组件层只消费已规范化的 `ToolStep`。
