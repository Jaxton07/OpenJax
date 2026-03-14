# Stream & Tool Call Refactor Index

目标：在不做兼容层的前提下，完成 OpenJax 后端流式链路重构，建立 `openjax-core/src/streaming/` 独立子系统，统一事件语义，降低 `planner/model/gateway` 耦合。

## 状态约定

- `planned`: 已设计未开始
- `in_progress`: 进行中
- `done`: 已完成

## 阅读顺序

1. [00-current-state-audit.md](./00-current-state-audit.md) `done`
2. [01-target-architecture.md](./01-target-architecture.md) `done`
3. [02-event-contract-and-api.md](./02-event-contract-and-api.md) `done`
4. [03-core-streaming-module-design.md](./03-core-streaming-module-design.md) `in_progress`
5. [04-gateway-stream-pipeline.md](./04-gateway-stream-pipeline.md) `in_progress`
6. [05-tool-call-streaming-and-approval.md](./05-tool-call-streaming-and-approval.md) `in_progress`
7. [06-migration-execution-plan.md](./06-migration-execution-plan.md) `planned`
8. [07-test-plan-and-acceptance.md](./07-test-plan-and-acceptance.md) `in_progress`
9. [08-observability-and-runbook.md](./08-observability-and-runbook.md) `planned`

## 阶段目标总览

1. 阶段 0：盘点现状、耦合点、瓶颈与删除清单。
2. 阶段 1：冻结目标架构、职责边界、并发模型。
3. 阶段 2：冻结事件契约、SSE 事件表、错误与恢复语义。
4. 阶段 3：完成 core streaming 模块设计与落地接口。
5. 阶段 4：完成 gateway 流水线瘦身与 replay/broadcast 策略升级。
6. 阶段 5：完成 tool call 增量流与 approval 并流语义。
7. 阶段 6：制定按 PR 切分的迁移执行方案。
8. 阶段 7：完成测试矩阵、压力验收门槛、回归基线。
9. 阶段 8：完成指标、告警、故障手册。

## 当前落地进度（本轮）

- 已新增 `openjax-core/src/streaming/` 模块骨架（event/orchestrator/parser/sink/replay）。
- 已将 `planner` 的核心流式发射逻辑接入 `ResponseStreamOrchestrator`，移除字符级 delta 发射。
- 已扩展 `openjax-protocol::Event`：`ToolCallArgsDelta/ToolCallProgress/ToolCallFailed`。
- 已同步 `openjax-gateway/openjaxd/ui-tui` 对新增工具流式事件的消费分支。
- 已移除 `AssistantDelta` 遗留事件并更新协议文档/schema。
- 已将 provider 的 `parse_sse_data_line` 入口收敛到 `streaming/parser`。
- 已将 gateway 事件回放窗口替换为 `streaming::ReplayBuffer` 并引入容量配置。
