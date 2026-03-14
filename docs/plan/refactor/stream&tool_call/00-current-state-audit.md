# 00 Current State Audit

状态：`done`

## 当前实现事实

1. Gateway 已使用 SSE 并支持 `Last-Event-ID/after_event_seq` 回放。
2. Core 已有 `ResponseStarted/ResponseTextDelta/ResponseCompleted/ResponseError` 生命周期事件。
3. 流式消费逻辑此前主要分散在 `agent/planner.rs` 内。
4. Tool call 事件此前缺乏参数增量与进度语义。

## 耦合点

1. `planner` 同时承担模型流读取、解析、事件发射与 fallback 决策。
2. `gateway` 的事件映射函数过大，夹杂 turn 状态更新与协议映射。
3. provider 解析逻辑重复，缺少通用 parser 抽象。

## 性能与可维护性瓶颈

1. 字符级 delta 发射导致事件量膨胀。
2. 回放窗口与广播容量固定常量，缺少可配置能力。
3. 工具调用生命周期在 UI/Gateway 侧可观测信息不足。

## 删除/替换清单（第一版）

1. 逐步移除 `planner` 中流式细节控制。
2. 逐步移除网关巨型映射中的业务状态耦合。
3. 统一 provider SSE 解析入口，减少重复代码路径。
