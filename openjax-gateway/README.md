# openjax-gateway

`openjax-gateway` 是 OpenJax 的 HTTP/SSE 网关模块，负责把 `openjax-core` 的会话与事件能力暴露为面向 Web/SDK 的 API。

## 职责

- 提供会话生命周期 API：创建、清理、关闭会话。
- 提供 turn 提交与查询 API：提交输入并轮询 turn 结果。
- 提供 SSE 事件流：支持 `Last-Event-ID` / `after_event_seq` 回放。
- 处理审批转发：将网关审批请求映射到 core 的 `ApprovalHandler`。
- 提供请求上下文、中间件鉴权、访问日志与统一错误模型。

## 文件树

```text
openjax-gateway/
├── Cargo.toml
├── src
│   ├── auth.rs
│   ├── error.rs
│   ├── handlers.rs
│   ├── lib.rs
│   ├── main.rs
│   ├── middleware.rs
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

受保护路由需 `Authorization: Bearer <api_key>`。

## 关键实现映射

- `src/lib.rs`：组装 Axum Router、CORS、全局中间件与受保护路由。
- `src/main.rs`：网关启动入口，读取 `OPENJAX_GATEWAY_BIND`（默认 `127.0.0.1:8765`）。
- `src/state.rs`：`AppState`/`SessionRuntime`、事件缓存回放、turn 与审批状态管理。
- `src/handlers.rs`：HTTP 处理函数与 core 事件映射到网关事件。
- `src/middleware.rs`：请求 ID、鉴权、访问日志。
- `src/error.rs`：统一错误响应结构（`code/message/retryable/details`）。
- `src/auth.rs`：API Key 环境变量加载与 Bearer Token 解析。
- `tests/gateway_api.rs`：鉴权、`/clear`、审批幂等、SSE 回放窗口等集成测试。

## 环境变量

- `OPENJAX_GATEWAY_BIND`：监听地址，默认 `127.0.0.1:8765`。
- `OPENJAX_GATEWAY_API_KEYS`：逗号分隔 API keys（优先）。
- `OPENJAX_API_KEYS`：兼容 API keys 变量（后备）。
- `OPENJAX_GATEWAY_WEB_DIR`：可选，web 静态目录（默认自动尝试 `<bin>/../web`）。
- `OPENJAX_APPROVAL_TIMEOUT_MS`：审批超时毫秒（由 core 读取）。

## 本地开发

从仓库根目录执行：

```bash
zsh -lc "cargo build -p openjax-gateway"
zsh -lc "cargo test -p openjax-gateway"
zsh -lc "OPENJAX_GATEWAY_API_KEYS=dev-key cargo run -p openjax-gateway"
```
