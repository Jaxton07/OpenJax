# llama.cpp WebUI 流式输出与 tool_calls 实现调研

## 1. 调研背景
本文整理了本地 `llama.cpp` 仓库中 WebUI 的流式输出实现，重点回答以下问题：
- 流式输出是如何从前端发起并持续渲染的；
- tool_calls 在流式场景下如何增量拼接；
- 停止生成、异常处理、服务端 SSE 推送如何协同；
- 当前实现采用的技术栈与关键设计取舍。

调研范围：
- 前端：`/Users/ericw/work/code/ai/llama.cpp/tools/server/webui`
- 后端对照：`/Users/ericw/work/code/ai/llama.cpp/tools/server`

---

## 2. 技术栈与架构定位

### 2.1 前端栈
- 框架：SvelteKit + Svelte 5（runes 状态模型）
- 构建：Vite
- 样式：TailwindCSS
- 本地存储：IndexedDB（Dexie）
- 通信：HTTP `fetch` + ReadableStream 手动 SSE 解析

### 2.2 运行与部署形态
- 开发期通过 Vite proxy 将 `/v1`、`/props`、`/models` 转发到 `http://localhost:8080`。
- SvelteKit 配置为 hash 路由、静态适配、inline bundle，最终产物可被 server 静态托管。

### 2.3 分层职责
- `routes/`：页面路由与入口；
- `components/`：界面与交互；
- `stores/`：会话状态与业务编排；
- `services/`：API 通信与流解析实现。

---

## 3. 端到端流式链路

### 3.1 发送入口
用户发送消息后，调用链为：
1. `ChatScreen` 触发发送动作；
2. `chatStore.sendMessage()` 创建用户消息与 assistant 占位消息；
3. `streamChatCompletion()` 设置回调、状态、AbortController；
4. `ChatService.sendMessage()` 发起 `POST /v1/chat/completions`（`stream: true`）；
5. 进入 `handleStreamResponse()` 持续读取与分发增量 chunk。

### 3.2 增量处理对象
每个流事件会被解析为以下主要增量类型：
- `delta.content`：正文 token 增量；
- `delta.reasoning_content`：推理内容增量；
- `delta.tool_calls`：工具调用增量；
- `timings` / `prompt_progress`：性能与进度数据。

### 3.3 完成与落库
- 收到 `data: [DONE]` 后触发完成路径；
- 将已聚合内容写回消息（DB + 内存状态）；
- 清理 loading/streaming/processing 状态；
- 在路由模式下刷新模型相关状态。

---

## 4. 前端流解析机制（核心）

### 4.1 解析方式
WebUI 采用手动 SSE 行解析：
1. `response.body.getReader()` 获取字节流；
2. `TextDecoder` 逐段解码；
3. 基于 `
` 分割事件行；
4. 仅处理 `data:` 前缀；
5. `data === [DONE]` 作为终止信号；
6. 其余 `data` 视为 JSON chunk 解析。

这意味着其协议语义与 SSE 一致，但实现上不依赖浏览器 `EventSource`。

### 4.2 内容聚合策略
- `content` 与 `reasoning_content` 分开累计，再统一回调；
- 推理块通过开关控制标签包裹，保证 UI 渲染结构稳定；
- `onChunk`、`onReasoningChunk` 实时更新页面，形成“边生成边显示”。

---

## 5. tool_calls 的流式拼接策略

### 5.1 增量难点
`tool_calls` 在流式响应中通常以“分片 delta”出现，尤其 `function.arguments` 可能被拆成多段字符串，不能直接覆盖。

### 5.2 当前实现
前端通过 `mergeToolCallDeltas(existing, deltas, indexOffset)` 聚合：
- 依据 `delta.index` 定位目标 tool call；
- 对 `id/type/function.name` 执行字段更新；
- 对 `function.arguments` 执行字符串追加；
- 用 `indexOffset` 处理跨批次连续到来的索引对齐。

### 5.3 对 UI 的影响
- 每次增量拼接后会序列化为 JSON 字符串并写回当前 assistant 消息；
- 因此工具调用参数可以在前端逐步可见，而不必等待完整响应结束。

---

## 6. 停止生成与错误处理

### 6.1 停止流程
停止按钮触发 `chatStore.stopGenerationForChat(convId)`：
1. 先尝试保存当前已流出的 partial 文本；
2. 触发 AbortController 中止网络读取；
3. 清理 loading/streaming/processing 状态；
4. 保证用户停止后仍能保留可恢复的中间输出。

### 6.2 错误分类
- HTTP 非 2xx：解析结构化错误并回传上下文信息（如 token/context）；
- 网络级异常：转换为更友好的连接错误；
- 流解析异常：进入 `onError`，清理状态并回收失败消息。

---

## 7. 服务端 SSE 对应实现

### 7.1 路由
服务端注册了 `/v1/chat/completions`，由对应 handler 进入 completions 流程。

### 7.2 推送机制
- 响应头 `content_type = text/event-stream`；
- 首包与后续包都经格式化函数转换为 SSE 文本块；
- 无更多数据时输出 `data: [DONE]\n\n`（OAI chat/completions 语义）；
- 若客户端断开或 `should_stop` 为真，流发送结束。

### 7.3 关键意义
前端“手动 SSE 解析”与后端“标准 event-stream 推送”严格对齐，协议边界清晰，具备较好可迁移性。

---

## 8. 关键设计总结
- 采用 `fetch + ReadableStream`，统一了请求与流处理模型，便于附加鉴权、错误处理、Abort 控制。
- 前端将 `content`、`reasoning`、`tool_calls`、`timings` 分离建模，适合扩展到更复杂 agentic 交互。
- `tool_calls` 的 delta 拼接逻辑是实现稳定多工具调用展示的核心。
- 停止生成时先保存 partial 再 abort，提高用户体验与数据完整性。
- 前后端在 `[DONE]` 和 `data:` 语义上保持一致，降低了协议错配风险。

---

## 9. 可复用到重构工作的要点
- 保留“事件类型分层 + 聚合器”的前端设计，不将所有增量混成单文本流。
- 将 `tool_calls` 拼接策略独立为纯函数，便于测试覆盖异常分片场景。
- 明确“停止优先保存”策略，避免用户中断导致内容丢失。
- 继续沿用 `timings/prompt_progress` 的并行事件上报，支撑后续可观测性与性能看板。
