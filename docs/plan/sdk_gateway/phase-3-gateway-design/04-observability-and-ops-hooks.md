# 04 Observability and Ops Hooks

## 日志字段（最小集）

- `request_id`
- `session_id`
- `turn_id`
- `event_seq`
- `route`
- `latency_ms`
- `result_code`

## 指标基线

- 请求吞吐：按路由统计 QPS
- 延迟：P50/P95/P99
- 错误率：按错误码统计
- 事件流：活跃连接数、断开率

## 审计记录

- 审批请求与审批决策必须可追溯到 `approval_id` 与操作者标识。

## 运维钩子

- 健康检查：进程活性、core 可用性
- 就绪检查：可接收新会话
