# 01 Runtime Architecture

## 运行拓扑（v1）

- Gateway 进程内嵌 `openjax-core`。
- HTTP 层负责同步接口。
- SSE 层负责事件持续推送。
- 会话管理层维护 `session_id -> runtime context`。

## 依赖方向

- Gateway API 层 -> Gateway Service 层 -> Core Adapter 层 -> `openjax-core`
- 协议类型从 phase-2 文档映射，不允许反向由实现定义协议。

## 关键设计约束

- 对外只暴露 phase-2 协议字段。
- 内部状态字段不可泄漏到对外响应。
- 同一会话的事件按 `event_seq` 严格有序输出。
