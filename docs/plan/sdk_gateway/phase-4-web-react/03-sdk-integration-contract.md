# 03 SDK Integration Contract

## 调用顺序

1. 初始化 SDK（注入 API Key）。
2. `start_session`。
3. 建立 `stream_events`。
4. `submit_turn`。
5. 按需 `resolve_approval`。
6. `shutdown_session`。

## 返回字段约束

- 前端只依赖统一字段：`request_id`、`session_id`、`turn_id`、`event_seq`、`timestamp`。
- 禁止前端依赖网关内部扩展字段。

## 封装建议

- SDK 层封装重试、断连恢复、错误结构解析。
- 业务页面仅处理状态机与 UI 呈现。
