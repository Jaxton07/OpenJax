# 06 Contract Review Checklist

## 评审目标

在进入 phase-3 实现前，确认 phase-2 协议契约可执行、无歧义、可测试。

## 必检项

### A. API 契约一致性

- [x] `start_session / submit_turn / resolve_approval / clear / compact / shutdown_session` 与字段命名一致。
- [x] 非流式轮询接口 `GET /sessions/{session_id}/turns/{turn_id}` 字段与状态语义一致。
- [x] 响应体统一包含 `request_id` 与 `timestamp`。
- [x] `session_id / turn_id` 出现条件清晰无冲突。
- [x] 聊天命令桥接映射清晰：`/clear`、`/compact` 均映射到显式接口。

### B. SSE 事件一致性

- [x] 事件类型集合与 phase-3/4 引用一致。
- [x] `event_seq` 单调递增与断点续读语义明确。
- [x] `turn_started -> ... -> turn_completed` 顺序约束可验证。
- [x] 双模式一致性明确：SSE 与 Polling 对同一 `turn_id` 的终态语义一致。

### C. 错误模型一致性

- [x] 错误结构固定为 `code/message/retryable/details`。
- [x] 4xx/5xx 错误码分层完整，无重复语义。
- [x] 重试语义（`retryable`）与错误码含义一致。
- [x] `compact` 未实现时 `NOT_IMPLEMENTED` 语义明确。

### D. 兼容与变更策略

- [x] 向后兼容规则明确（新增可选字段、不可移除已发布字段）。
- [x] 破坏性变更流程清晰（phase-2 -> DECISIONS -> phase-3/4/5）。
- [x] 弃用策略可执行（标注 deprecated + 替代字段）。

### E. 安全基线

- [x] API Key 鉴权规则明确。
- [x] 日志脱敏要求明确（不可记录 key 明文）。
- [x] 审计字段可支撑问题定位与追踪。

## 评审结论记录

- 评审日期：2026-03-08
- 参与人：ericw, codex
- 结论：`pass`
- 变更项（如有）：无
- 是否需要更新 DECISIONS：`yes`（已新增 ADR-0005、ADR-0006）
