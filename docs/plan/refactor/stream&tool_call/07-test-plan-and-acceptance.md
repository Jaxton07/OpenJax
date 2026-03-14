# 07 Test Plan And Acceptance

状态：`in_progress`

## Core 测试

1. orchestrator 事件顺序：`started -> delta* -> completed/error`。
2. replay buffer 越窗错误与回放正确性。
3. bounded sink 背压策略行为验证。
4. parser 对多 chunk SSE 的解析正确性。

## Gateway 集成测试

1. SSE 首包时延与事件连续性。
2. `Last-Event-ID` / `after_event_seq` 恢复正确性。
3. lagged 恢复失败时返回规范错误。
4. 多 session 并发隔离。

## Tool/Approval 测试

1. `ToolCallArgsDelta` 拼接一致性。
2. `ToolCallProgress` 顺序稳定性。
3. `ToolCallFailed` 与审批拒绝/超时路径一致性。

## 验收门槛

1. `cargo check` 全 workspace 通过。
2. 核心集成测试通过：
- `cargo test -p openjax-core --test m6_submit_stream`
- `cargo test -p openjax-gateway`
- `cargo test -p tui_next`
3. 关键路径无回归：turn 流式输出、审批、工具调用。
