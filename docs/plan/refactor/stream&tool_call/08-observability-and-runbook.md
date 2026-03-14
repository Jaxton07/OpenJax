# 08 Observability And Runbook

状态：`done`

## 核心指标

1. TTFT（首 token/首 delta 时间）。
2. delta 吞吐（chars/s 或 tokens/s）。
3. replay 越窗率。
4. lagged 恢复成功率。
5. tool 流失败率（`tool_call_failed`）。

## 日志与追踪

1. 每个 turn 记录：turn_id、event_seq、event_type、stream_source。
2. 每个 tool_call 记录：tool_call_id、phase、耗时、结果。
3. 每个 approval 记录：approval_id、决策、超时。

## 告警建议

1. TTFT 超过阈值持续 5 分钟。
2. replay 越窗率高于阈值。
3. tool_call_failed 在单模型路由下突增。

## 运行手册

1. 首先确认 provider 健康与网络链路。
2. 再确认 gateway replay window/capacity 配置。
3. 最后根据 `tool_call_id/approval_id/turn_id` 关联排障。

## 指标命名与实现对齐建议

1. `stream_ttft_ms`：首个 `response_text_delta` 与 `turn_started` 的时间差。
2. `stream_delta_chars_per_sec`：单位时间输出字符吞吐。
3. `tool_fail_rate`：`tool_call_failed / tool_call_started`。
4. `lagged_recovery_rate`：`sse_lagged_recovered / (sse_lagged_recovered + sse_lagged_recovery_failed)`。
5. `replay_window_exceeded_rate`：`REPLAY_WINDOW_EXCEEDED` 错误占 SSE 请求比例。

## 统一日志关联键

1. `session_id`
2. `turn_id`
3. `event_seq`
4. `tool_call_id`
5. `approval_id`

## 告警落地基线

1. `stream_ttft_ms` P95 连续 5 分钟超过基线阈值（如 3s）。
2. `replay_window_exceeded_rate` 5 分钟窗口超过 1%。
3. `tool_fail_rate` 在固定模型路由下较 7 天均值突增 2 倍以上。
