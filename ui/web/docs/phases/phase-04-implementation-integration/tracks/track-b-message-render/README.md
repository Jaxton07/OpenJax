# Track B - Message Render

## 目标
- 在不破坏现有消息渲染的前提下，接入结构化 ToolStep 渲染分支。

## 上游文档（编码前必读）
- [Phase 01 - Event Inventory](../../../phase-01-requirements-boundaries/artifacts/event-inventory.md)
- [Phase 02 - Message Model](../../../phase-02-data-contract-protocol/artifacts/message-model.md)
- [Phase 02 - Backward Compatibility](../../../phase-02-data-contract-protocol/artifacts/backward-compat.md)
- [Phase 03 - Component Boundary](../../../phase-03-ui-architecture-components/artifacts/component-boundary.md)
- [Phase 03 - Decisions](../../../phase-03-ui-architecture-components/DECISIONS.md)

## 任务范围
- MessageList 增加结构化路径。
- 普通消息路径继续沿用现有样式与逻辑。
- 处理 mixed messages（文本与步骤混排）。

## 交付物
- 渲染分支规则说明。
- 回退逻辑说明。
