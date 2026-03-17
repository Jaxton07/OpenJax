# ui/web

`ui/web` 是 OpenJax 的 React Web 前端（Vite + TypeScript），通过 `openjax-gateway` 提供聊天会话、流式输出与审批交互。

## 职责

- 提供多会话聊天 UI（侧边栏、消息区、输入区）。
- 对接 gateway API：会话创建、turn 提交、状态轮询、timeline hydration、SSE 订阅。
- 支持审批交互（approve/reject）。
- 支持结构化 Tool Step 卡片渲染（`message.kind=tool_steps`）。
- 支持按事件分段的思考流渲染（`reasoning_delta`，默认折叠，按事件时间线与正文/tool 交错显示）。
- 本地持久化设置与会话（`localStorage`）。
- 提供 SSE 与 polling 两种输出模式。

## 子目录文档导航

- 组件层索引：[`src/components/README.md`](src/components/README.md)
- 逻辑层索引：[`src/lib/README.md`](src/lib/README.md)
- 样式层索引：[`src/styles/README.md`](src/styles/README.md)

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
    ├── pic
    │   └── icon
    │       └── index.tsx
    ├── components
    │   ├── Composer.tsx
    │   ├── LoginPage.tsx
    │   ├── MessageList.test.tsx
    │   ├── MessageList.tsx
    │   ├── SettingsModal.tsx
    │   ├── Sidebar.tsx
    │   └── tool-steps
    │       ├── StepBody.tsx
    │       ├── StepStatusBadge.tsx
    │       ├── ToolStepCard.test.tsx
    │       ├── ToolStepCard.tsx
    │       └── ToolStepList.tsx
    ├── hooks
    │   ├── useChatApp.ts
    │   └── useStreamRenderSnapshot.ts
    ├── lib
    │   ├── errors.test.ts
    │   ├── errors.ts
    │   ├── session-events
    │   │   ├── assistant.ts
    │   │   ├── assistant.test.ts
    │   │   ├── reducer.ts
    │   │   ├── reducer.test.ts
    │   │   ├── sequence.ts
    │   │   ├── tools.ts
    │   │   └── tools.test.ts
    │   ├── gatewayClient.test.ts
    │   ├── gatewayClient.ts
    │   ├── storage.test.ts
    │   └── storage.ts
    │   ├── streamPerf.ts
    │   ├── streamRenderStore.test.ts
    │   ├── streamRenderStore.ts
    │   ├── streamRuntime.test.ts
    │   └── streamRuntime.ts
    ├── styles
    │   ├── app.css
    │   ├── base.css
    │   ├── layout.css
    │   ├── messages.css
    │   ├── composer.css
    │   ├── login.css
    │   ├── responsive.css
    │   ├── settings.tokens.css
    │   ├── settings.shell.css
    │   ├── settings.general.css
    │   ├── settings.provider.css
    │   ├── settings.controls.css
    │   └── settings.css
    ├── test
    │   └── setup.ts
    └── types
        ├── chat.ts
        ├── gateway.ts
        └── markstream-shims.d.ts
```

## 关键实现映射

- `src/hooks/useChatApp.ts`：应用状态机、会话管理、SSE 重连与 polling 流程。
- `src/lib/gatewayClient.ts`：gateway HTTP/SSE 客户端封装（含 timeline 拉取接口）。
- `src/lib/session-events/reducer.ts`：将流式事件折叠为本地会话状态与消息列表（含 `message.kind` 分流）。
- `src/lib/timeline/buildTimeline.ts`：将消息展开为事件级时间线项，按 `event_seq` 主排序。
- `src/lib/streamRenderStore.ts`：正文 delta 的运行时拼接缓存（按 `session+turn` 聚合）。
- `src/lib/streamRuntime.ts`：文本流事件处理与顺序门控工具。
- `src/lib/storage.ts`：设置与会话本地存储（`openjax:web:*`）。
- `src/styles/app.css`：样式聚合入口（通过 `@import` 组合各模块样式）。
- `src/styles/base.css`：全局基础样式与 CSS 变量。
- `src/styles/layout.css`：应用布局、侧栏、顶部区域样式。
- `src/styles/messages.css`：消息区、reasoning、tool step 样式。
- `src/styles/composer.css`：输入区与发送区样式。
- `src/styles/login.css`：登录页样式。
- `src/styles/responsive.css`：非设置页的响应式规则。
- `src/styles/settings.css`：设置页样式聚合入口（通过 `@import` 组合 settings 子模块）。
- `src/styles/settings.tokens.css`：设置页主题 token。
- `src/styles/settings.shell.css`：设置弹窗壳层与导航布局。
- `src/styles/settings.general.css`：通用设置面板样式。
- `src/styles/settings.provider.css`：Provider 列表与表单样式。
- `src/styles/settings.controls.css`：设置页按钮与状态样式。
- `src/components/MessageList.tsx`：按时间线项渲染文本/思考/tool 卡片，支持 reasoning 与 tool 的交错顺序。
- `src/components/tool-steps/*`：Tool 卡片组件层（列表/卡片/状态徽标/详情体）。
- `src/pic/icon/index.tsx`：统一 SVG 图标组件出口，供页面复用。
- `src/types/gateway.ts`：网关协议类型定义（请求/响应/事件）。
- `src/types/chat.ts`：本地会话与消息模型（`ChatMessage.kind`、`ToolStep` 等）。

## 消息模型（当前）

- `ChatMessage.kind = "text" | "tool_steps"`。
- `kind=text`：渲染传统文本气泡。
- `kind=tool_steps`：渲染结构化步骤卡片（可折叠、状态徽标、详情区）。
- assistant 文本消息支持 `reasoningBlocks`：
  - 数据来源：`reasoning_delta`。
  - 分段规则：收到 `reasoning_delta` 时追加到当前未关闭段；遇到 `response_text_delta` / tool 事件 / completed / error / turn_completed 关闭当前段；后续 reasoning 自动新开段。
  - 展示规则：每段一个独立时间线卡片，默认折叠，可与 tool 卡片按事件顺序交错显示。
- Markdown 渲染策略：
  - assistant 正文默认使用 `markstream-react` 渲染。
  - reasoning 展开内容使用 `markstream-react` 渲染，支持流式未完成状态。
  - 渲染前会对原始 HTML 标签做转义，避免脚本注入执行。
  - 可通过 `OPENJAX_WEB_ASSISTANT_RENDER_MODE=text`（或 `VITE_OPENJAX_WEB_ASSISTANT_RENDER_MODE=text`）回退为纯文本正文渲染。
  - 其他取值（含未配置）默认按 markdown 处理。
- 流式 Markdown 注意事项：
  - 在流式过程中，未闭合代码块/列表属于正常中间态，最终 `response_completed` 后会收敛为完整结构。
- 目前 reducer 保留 `role=tool` 文本双写路径（过渡用）。
- 旧 `assistant + toolSteps` 结构不再兼容，渲染按 `kind` 判定。

## 时间线恢复（Hydration）

- 冷启动恢复入口：`GET /api/v1/sessions/{session_id}/timeline`
- 处理方式：`useChatApp` 将 timeline 事件交给 `applySessionEvents` 统一归并重建会话。
- 主排序键：`event_seq`（`timestamp` 仅兜底）。
- 事件覆盖：`user_message`、`reasoning_delta`、tool/approval、`response_text_delta`、`response_completed` 等。
- 说明：`/messages` 仍保留用于兼容展示，但不再作为时间线恢复主入口。


## 运行与测试

从仓库根目录执行：

```bash
zsh -lc "make run-web-dev"
zsh -lc "cd ui/web && pnpm install"
zsh -lc "cd ui/web && pnpm dev"
zsh -lc "cd ui/web && pnpm build"
zsh -lc "cd ui/web && pnpm test"
zsh -lc "cd ui/web && pnpm test -- src/lib/session-events/reducer.test.ts src/components/MessageList.test.tsx src/components/tool-steps/ToolStepCard.test.tsx"
```

默认开发地址：`http://127.0.0.1:5173`。
若使用 `make run-web-dev`，会同时启动 gateway（默认 `127.0.0.1:8765`）和前端开发服务，`Ctrl+C` 可一起停止。

## 设置项

在左下角设置面板可配置：

- `Gateway Base URL`（默认 `http://127.0.0.1:8765`）
- `Output Mode`（`sse` 或 `polling`）

登录凭据（Owner Key）在登录页输入，不在设置弹窗中维护。

### SSE 重连策略

- 网络抖动或临时网关错误会自动重连（指数退避，最多 6 次）。
- 如果是鉴权失败（如 `401/UNAUTHENTICATED`、`403/FORBIDDEN`，常见于 API Key 与 gateway 启动配置不匹配），前端会直接在页面显示错误并停止重连，避免无效重试。

## 网关接口兼容性

当前客户端对接接口：

- `POST /api/v1/sessions`
- `POST /api/v1/sessions/{session_id}/turns`
- `GET /api/v1/sessions/{session_id}/turns/{turn_id}`
- `GET /api/v1/sessions/{session_id}/timeline`
- `GET /api/v1/sessions/{session_id}/events`
- `POST /api/v1/sessions/{session_id}/approvals/{approval_id}:resolve`
- `POST /api/v1/sessions/{session_id}:clear`
- `POST /api/v1/sessions/{session_id}:compact`
- `DELETE /api/v1/sessions/{session_id}`
