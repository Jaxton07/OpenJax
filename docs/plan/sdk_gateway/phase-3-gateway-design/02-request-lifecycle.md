# 02 Request Lifecycle

## 同步请求生命周期

1. 接收 HTTP 请求并校验 API Key。
2. 生成 `request_id` 并写入上下文。
3. 参数校验，映射到 core 操作。
4. 返回同步响应（含 `request_id`、`session_id`、`turn_id` 等）。

## 事件流生命周期

1. 客户端建立 SSE 连接。
2. 网关订阅会话事件并映射为 phase-2 统一事件包。
3. 每条事件附带递增 `event_seq`。
4. 连接中断后可按 `event_seq` 执行断点续读。

## 审批链路

- `approval_requested` 通过 SSE 下发。
- 客户端调用 resolve 接口。
- 网关将决策传递给 core 并发布 `approval_resolved`。
