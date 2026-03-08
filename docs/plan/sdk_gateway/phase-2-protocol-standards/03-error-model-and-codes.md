# 03 Error Model and Codes

## 错误结构

```json
{
  "request_id": "req_xxx",
  "timestamp": "2026-03-08T12:01:00Z",
  "error": {
    "code": "INVALID_ARGUMENT",
    "message": "input is required",
    "retryable": false,
    "details": {
      "field": "input"
    }
  }
}
```

## 分层错误码

### 4xx

- `UNAUTHENTICATED`: 缺失或非法 API Key。
- `FORBIDDEN`: API Key 无权限访问目标资源。
- `INVALID_ARGUMENT`: 参数不合法。
- `NOT_FOUND`: 会话或审批单不存在。
- `CONFLICT`: 状态冲突（如重复 resolve）。
- `RATE_LIMITED`: 触发限流。

### 5xx

- `INTERNAL`: 网关内部错误。
- `UPSTREAM_UNAVAILABLE`: 内核或依赖不可用。
- `STREAM_BROKEN`: 流式通道中断。
- `TIMEOUT`: 内核执行超时。
- `NOT_IMPLEMENTED`: 接口已登记但当前版本未实现（如 `compact` 在 core 未落地时）。

## 语义约束

- `message` 面向调用方可读，禁止暴露敏感内部栈信息。
- `retryable=true` 仅用于建议可重试错误。
- 业务失败必须返回结构化错误，不使用纯文本错误体。
