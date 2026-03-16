# openjax-gateway

`openjax-gateway` 是 OpenJax 的 HTTP/SSE 网关模块，负责把 `openjax-core` 的会话与事件能力暴露为面向 Web/SDK 的 API。

## 职责

- 提供会话生命周期 API：创建、清理、关闭会话。
- 提供 turn 提交与查询 API：提交输入并轮询 turn 结果。
- 提供 SSE 事件流：支持 `Last-Event-ID` / `after_event_seq` 回放。
- 提供会话时间线 API：按 `event_seq` 查询持久化事件，用于前端冷启动恢复。
- 处理审批转发：将网关审批请求映射到 core 的 `ApprovalHandler`。
- 提供请求上下文、中间件鉴权、访问日志与统一错误模型。

## 文件树

```text
openjax-gateway/
├── Cargo.toml
├── src
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── cookie.rs
│   │   ├── rate_limit.rs
│   │   ├── service.rs
│   │   ├── store.rs
│   │   ├── token.rs
│   │   └── types.rs
│   ├── auth_handlers.rs
│   ├── event_mapper/
│   │   ├── mod.rs
│   │   ├── response.rs
│   │   ├── tool.rs
│   │   └── approval.rs
│   ├── error.rs
│   ├── handlers.rs
│   ├── lib.rs
│   ├── main.rs
│   ├── middleware.rs
│   ├── persistence/
│   │   ├── mod.rs
│   │   ├── repository.rs
│   │   ├── sqlite.rs
│   │   └── types.rs
│   └── state.rs
└── tests
    └── gateway_api.rs
```

## 路由概览

- `GET /healthz`
- `GET /readyz`
- `GET /`（可选：托管 web 静态首页，需存在 `index.html`）
- `GET /assets/*path`（可选：托管 web 构建产物资源）
- `POST /api/v1/sessions`
- `POST /api/v1/sessions/:session_id/turns`
- `GET /api/v1/sessions/:session_id/turns/:turn_id`
- `POST /api/v1/sessions/:session_id`（当前用于 `:clear` / `:compact` action 语法）
- `DELETE /api/v1/sessions/:session_id`
- `POST /api/v1/sessions/:session_id/approvals/*approval_action`
- `GET /api/v1/sessions/:session_id/events`
- `GET /api/v1/sessions/:session_id/timeline`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/auth/logout`
- `POST /api/v1/auth/revoke`
- `GET /api/v1/auth/sessions`

受保护业务路由需 `Authorization: Bearer <access_token>`。`/api/v1/auth/login` 使用 owner key。

## 关键实现映射

- `src/lib.rs`：组装 Axum Router、CORS、全局中间件与受保护路由。
- `src/main.rs`：网关启动入口，读取 `OPENJAX_GATEWAY_BIND`（默认 `127.0.0.1:8765`）。
- `src/state.rs`：`AppState`/`SessionRuntime`、事件缓存回放、turn 与审批状态管理、会话重建（`next_event_seq`/`turn_event_seq`）。
- `src/event_mapper/`：core 事件到 gateway 事件的薄映射层（response/tool/approval）。
- `src/handlers.rs`：HTTP 处理函数与 core 事件映射到网关事件，统一发布+落盘入口。
- `src/auth_handlers.rs`：登录、刷新、登出、撤销、会话查询接口。
- `src/middleware.rs`：请求 ID、鉴权、访问日志。
- `src/error.rs`：统一错误响应结构（`code/message/retryable/details`）。
- `src/auth/`：owner key 加载、token 生成哈希、SQLite 持久化、限流与 cookie 逻辑。
- `src/persistence/`：`biz_sessions`/`biz_messages`/`biz_events` 持久化仓储实现。
- `tests/gateway_api.rs`：鉴权、`/clear`、审批幂等、SSE 回放窗口等集成测试。

## 事件持久化模型

- `biz_events`：事件级持久化（时间线恢复主数据源）。
  - 关键列：`session_id`, `event_seq`, `turn_seq`, `turn_id`, `event_type`, `payload_json`, `timestamp`, `stream_source`, `created_at`
  - 关键约束：`UNIQUE(session_id, event_seq)`
  - 关键索引：`(session_id, turn_id, event_seq)`、`(session_id, created_at)`
- `biz_messages`：保留用于旧消息接口与简版历史浏览；时间线恢复不再依赖该表。
- 发布给前端的事件（含 gateway 合成事件与 `user_message`）统一经过发布+落盘链路，避免漏写。

## 环境变量

- `OPENJAX_GATEWAY_BIND`：监听地址，默认 `127.0.0.1:8765`。
- `OPENJAX_GATEWAY_API_KEYS`：逗号分隔 API keys（优先）。
- `OPENJAX_API_KEYS`：兼容 API keys 变量（后备）。
- `OPENJAX_GATEWAY_ACCESS_TTL_MINUTES`：access token TTL（默认 15）。
- `OPENJAX_GATEWAY_REFRESH_TTL_DAYS`：refresh token TTL（默认 30）。
- `OPENJAX_GATEWAY_COOKIE_SECURE`：refresh cookie 是否设置 `Secure`（默认 true）。
- `OPENJAX_GATEWAY_AUTH_RATE_LIMIT_LOGIN_PER_MIN`：登录限流（默认 30）。
- `OPENJAX_GATEWAY_AUTH_RATE_LIMIT_REFRESH_PER_MIN`：刷新限流（默认 120）。
- `OPENJAX_GATEWAY_AUTH_TOKEN_PEPPER`：token 哈希 pepper。
- `OPENJAX_GATEWAY_WEB_DIR`：可选，web 静态目录（默认自动尝试 `<bin>/../web`）。
- `OPENJAX_GATEWAY_EVENT_REPLAY_LIMIT`：SSE 回放窗口大小（默认 1024）。
- `OPENJAX_GATEWAY_EVENT_CHANNEL_CAPACITY`：SSE 广播通道容量（默认 1024）。
- `OPENJAX_APPROVAL_TIMEOUT_MS`：审批超时毫秒（由 core 读取）。

若上述 API Key 变量都未设置，gateway 会在启动时自动生成随机 owner key（仅当前进程有效）并打印到终端。

## 本地开发

从仓库根目录执行：

```bash
zsh -lc "cargo build -p openjax-gateway"
zsh -lc "cargo test -p openjax-gateway"
zsh -lc "cargo run -p openjax-gateway"
```

## WebUI 流式接入（SSE）

1. 建立 SSE 连接：`GET /api/v1/sessions/:session_id/events`。
2. 使用 `Last-Event-ID` 或 `after_event_seq` 做断线恢复。
3. 每条 SSE 的 `data` 是事件信封（`event_seq/turn_seq/type/payload`）。
4. 关键渲染事件：
- `user_message`
- `response_started`
- `reasoning_delta`（思考流增量，建议在正文上方折叠展示）
- `response_text_delta`
- `response_resumed`
- `response_completed`
- `tool_call_started/tool_args_delta/tool_call_ready/tool_call_progress/tool_call_completed/tool_call_failed`
- `approval_requested/approval_resolved`
5. 若收到 `response_error.code=REPLAY_WINDOW_EXCEEDED`，应提示前端重新发起会话流连接。

## Timeline 接口（冷启动恢复）

- `GET /api/v1/sessions/:session_id/timeline`
- 可选参数：`after_event_seq`
- 返回：按 `event_seq` 升序的事件信封数组（结构与 SSE 事件一致，含 `user_message`）
- 推荐用法：前端初始化先拉 timeline 全量/增量，再接入 SSE 实时流。
