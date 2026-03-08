# 02 组件边界与职责

## 边界原则

- 协议边界在 Gateway。
- 业务执行边界在 Core。
- 交互边界在 Client。

## Rust Core（`openjax-core`）

### 负责

- Agent 生命周期与回合执行。
- Tool 编排与调用约束。
- Sandbox 与 approval 域语义。

### 不负责

- HTTP/SSE 协议处理。
- API Key 鉴权与外部访问控制。
- 前端会话展示逻辑。

## Rust Gateway（`openjax-gateway`）

### 负责

- 对外 API（`start_session` / `submit_turn` / `resolve_approval` / `shutdown_session`）。
- 对外事件流（`stream_events` SSE）。
- API Key 鉴权、限流、审计、指标与日志。
- 会话上下文与请求生命周期编排。

### 不负责

- 重写或复制 Core 的 agent/tool/sandbox 逻辑。
- 引入未在 phase-2 登记的协议字段。

## React Client

### 负责

- 对话 UI、会话列表、审批交互。
- 客户端状态机（连接/会话/turn）。
- 调用 SDK 或协议客户端并处理错误反馈。

### 不负责

- 业务编排与执行策略决策。
- 权限模型后端判定。

## 跨层契约

- Gateway 对 Core 仅传递执行意图与必要上下文，不泄露前端视图细节。
- Client 对 Gateway 仅依赖 phase-2 公共字段：
  - `request_id`、`session_id`、`turn_id`、`event_seq`、`timestamp`
- 协议变更顺序：phase-2 -> DECISIONS -> phase-3/4/5。
