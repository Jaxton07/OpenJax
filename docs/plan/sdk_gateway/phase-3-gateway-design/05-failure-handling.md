# 05 Failure Handling

## 超时

- 请求超时返回 `TIMEOUT`。
- 超时不应导致会话状态损坏。

## 断连

- SSE 断开后客户端可重连并携带断点。
- 网关应在窗口期内支持按 `event_seq` 续读。

## 幂等

- `resolve_approval` 对同一 `approval_id` 幂等处理：
  - 首次成功返回 resolved
  - 重复请求返回 `CONFLICT` 或等价幂等响应（需一致）

## 不可用降级

- core 临时不可用返回 `UPSTREAM_UNAVAILABLE`。
- 记录错误并保留请求追踪信息。
