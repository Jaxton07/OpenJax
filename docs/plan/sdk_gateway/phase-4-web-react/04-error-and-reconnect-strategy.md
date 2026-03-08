# 04 Error and Reconnect Strategy

## 错误体验

- `UNAUTHENTICATED`：提示重新配置 API Key。
- `RATE_LIMITED`：显示限流提示与建议等待时间。
- `UPSTREAM_UNAVAILABLE`/`TIMEOUT`：提供重试按钮。

## 重连策略

- SSE 断开后执行指数退避重连。
- 重连时携带最近 `event_seq` 进行续读。
- 连续重连失败达到阈值后提示手动恢复。

## 一致性策略

- 重连恢复后，按 `event_seq` 去重与补齐。
- 不允许重复渲染同一事件。
