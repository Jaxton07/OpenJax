# 01 目标架构

## 架构目标

- 对外提供稳定可版本化 API 与流式事件接口（HTTP + SSE）。
- 对内复用 Rust 内核能力，避免重复实现 agent/tool/sandbox。
- 为 Web 与未来移动端提供同一协议接入面。

## 分层与职责

- Core Layer：`openjax-core`
  - 负责 agent loop、tool 调用编排、sandbox、approval 语义。
- Gateway Layer：`openjax-gateway`（待创建）
  - 负责 HTTP API、SSE、API Key 鉴权、会话编排、可观测。
- Client Layer：React Web（v1）+ Mobile（future）
  - 负责交互和状态呈现，不承载业务编排。

## 依赖方向（固定）

- Client -> Gateway -> Core
- Core 不依赖 Gateway，不感知 HTTP、SSE、鉴权。
- Gateway 通过适配层调用 Core，不把协议层细节下沉到 Core。

## 关键数据流

### A. 同步请求流

1. Client 调用 Gateway API。
2. Gateway 校验 API Key，生成 `request_id`。
3. Gateway 将请求映射为 Core 操作。
4. Gateway 返回结构化响应（含统一字段）。

### B. 流式事件流

1. Client 建立 SSE 连接。
2. Gateway 订阅 Core 执行事件。
3. Gateway 统一封装事件包并递增 `event_seq`。
4. Client 基于 `event_seq` 渲染与断点恢复。

## v1 架构约束

- 运行模式固定为“Gateway 内嵌 openjax-core”。
- v1 不引入 daemon 双模式，不引入多租户语义。
- 协议字段来源统一于 phase-2；实现层不得反向定义协议。

## 开发入口（用于后续实现）

- Gateway crate（待创建）：`openjax-gateway/`
- Core 参考入口：`openjax-core/src/agent`、`openjax-core/src/tools`
- 协议参考入口：`openjax-protocol/src/lib.rs`
