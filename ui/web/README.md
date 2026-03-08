# ui/web

`ui/web` 是 OpenJax 的 React Web 前端（Vite + TypeScript），通过 `openjax-gateway` 提供聊天会话、流式输出与审批交互。

## 职责

- 提供多会话聊天 UI（侧边栏、消息区、输入区）。
- 对接 gateway API：会话创建、turn 提交、状态轮询、SSE 订阅。
- 支持审批交互（approve/reject）。
- 本地持久化设置与会话（`localStorage`）。
- 提供 SSE 与 polling 两种输出模式。

## 文件树

```text
ui/web/
├── README.md
├── index.html
├── package.json
├── pnpm-lock.yaml
├── tsconfig.json
├── tsconfig.node.json
├── vite.config.ts
└── src
    ├── App.tsx
    ├── main.tsx
    ├── components
    │   ├── ApprovalPanel.tsx
    │   ├── Composer.tsx
    │   ├── MessageList.tsx
    │   ├── SettingsModal.tsx
    │   └── Sidebar.tsx
    ├── hooks
    │   └── useChatApp.ts
    ├── lib
    │   ├── errors.test.ts
    │   ├── errors.ts
    │   ├── eventReducer.test.ts
    │   ├── eventReducer.ts
    │   ├── gatewayClient.ts
    │   ├── storage.test.ts
    │   └── storage.ts
    ├── styles
    │   └── app.css
    ├── test
    │   └── setup.ts
    └── types
        ├── chat.ts
        └── gateway.ts
```

## 关键实现映射

- `src/hooks/useChatApp.ts`：应用状态机、会话管理、SSE 重连与 polling 流程。
- `src/lib/gatewayClient.ts`：gateway HTTP/SSE 客户端封装。
- `src/lib/eventReducer.ts`：将流式事件折叠为本地会话状态与消息列表。
- `src/lib/storage.ts`：设置与会话本地存储（`openjax:web:*`）。
- `src/components/*`：侧边栏、审批面板、消息列表、输入框、设置弹窗。
- `src/types/gateway.ts`：网关协议类型定义（请求/响应/事件）。

## 运行与测试

从仓库根目录执行：

```bash
zsh -lc "cd ui/web && pnpm install"
zsh -lc "cd ui/web && pnpm dev"
zsh -lc "cd ui/web && pnpm build"
zsh -lc "cd ui/web && pnpm test"
```

默认开发地址：`http://127.0.0.1:5173`。

## 设置项

在左下角设置面板可配置：

- `Gateway Base URL`（默认 `http://127.0.0.1:8765`）
- `API Key`
- `Output Mode`（`sse` 或 `polling`）

## 网关接口兼容性

当前客户端对接接口：

- `POST /api/v1/sessions`
- `POST /api/v1/sessions/{session_id}/turns`
- `GET /api/v1/sessions/{session_id}/turns/{turn_id}`
- `GET /api/v1/sessions/{session_id}/events`
- `POST /api/v1/sessions/{session_id}/approvals/{approval_id}:resolve`
- `POST /api/v1/sessions/{session_id}:clear`
- `POST /api/v1/sessions/{session_id}:compact`
- `DELETE /api/v1/sessions/{session_id}`
