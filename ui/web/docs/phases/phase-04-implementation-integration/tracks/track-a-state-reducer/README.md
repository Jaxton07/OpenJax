# Track A - State & Reducer

## 目标
- 让状态层可表达 ToolStep 结构化数据并支持幂等更新。

## 上游文档（编码前必读）
- [Phase 01 - Scope Matrix](../../../phase-01-requirements-boundaries/artifacts/scope-matrix.md)
- [Phase 02 - Message Model](../../../phase-02-data-contract-protocol/artifacts/message-model.md)
- [Phase 02 - Gateway Event Mapping](../../../phase-02-data-contract-protocol/artifacts/gateway-event-mapping.md)
- [Phase 02 - Backward Compatibility](../../../phase-02-data-contract-protocol/artifacts/backward-compat.md)
- [Phase 02 - Decisions](../../../phase-02-data-contract-protocol/DECISIONS.md)

## 任务范围
- 扩展 chat types（ToolStep 与 message payload）。
- 调整 event reducer 的 step 聚合逻辑。
- 保持旧消息回退不受影响。

## 交付物
- 类型定义变更说明。
- reducer 映射逻辑说明。

## 已实施行为（2026-03-09）
- 在 `ChatMessage` 中引入 `toolSteps?: ToolStep[]`，并补齐 `ToolStep` 相关类型定义。
- `applyStreamEvent` 已支持 tool/approval/error 事件到 step 的结构化聚合。
- 保留旧 `role=tool` 文本消息双写路径，保障 Track B 前 UI 可见性。
- 保持 `event_seq` 幂等去重与异常 payload 容错，不中断会话流。
