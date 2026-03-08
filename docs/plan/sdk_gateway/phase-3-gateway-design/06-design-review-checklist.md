# 06 Design Review Checklist

## 评审目标

确认 Gateway 设计可直接进入实现，不存在协议漂移、状态歧义或不可落地约束。

## 必检项

### A. 协议对齐

- [x] phase-3 文档中 API、字段、错误语义全部与 phase-2 一致。
- [x] 双输出模式（SSE + Polling）在生命周期与失败处理中均有覆盖。
- [x] `clear/compact` 与命令桥接路径在设计中可落地。

### B. 运行与状态模型

- [x] Gateway 内嵌 core 的运行拓扑无循环依赖。
- [x] session/turn 状态迁移完整且终态唯一。
- [x] 审批链路（request -> resolve）状态闭环完整。

### C. 故障与恢复

- [x] 超时、断连、上游不可用场景有明确处理路径。
- [x] SSE 断点续读与 Polling 终态查询语义一致。
- [x] 幂等行为（尤其 `resolve_approval`）定义明确。

### D. 可观测与运维

- [x] 最小日志字段可支持端到端追踪。
- [x] 指标项可支撑 phase-5 验收（延迟、错误率、连接稳定性）。
- [x] runbook 需要的诊断字段在设计中已预留。

## 评审结论记录

- 评审日期：2026-03-08
- 参与人：ericw, codex
- 结论：`pass`
- 变更项（如有）：无
- 是否需要更新 DECISIONS：`no`
