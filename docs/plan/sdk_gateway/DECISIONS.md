# SDK Gateway 决策日志（ADR 简版）

> 记录会影响后续阶段执行的关键决策。每条决策应可追溯。

## ADR-0001 文档治理模式

- 日期：2026-03-08
- 状态：accepted
- 背景：项目由个人长期维护，需要降低上下文膨胀和遗漏风险。
- 决策：采用“严格门禁 + 两层 TODO（全局阶段看板 + 阶段执行清单）”。
- 影响：
  - 阶段开始前必须完成文档前置检查。
  - 文档读取顺序固定，默认禁止跨阶段读取。
- 备选方案：轻量迭代（被拒绝，遗漏风险更高）。

## ADR-0002 文档信息分层

- 日期：2026-03-08
- 状态：accepted
- 背景：希望文档主要服务代理执行，避免无关历史进入上下文。
- 决策：根目录仅保留导航/节奏/决策；细节全部下沉到阶段目录。
- 影响：
  - 根 README 只做状态面板和跳转。
  - 每阶段维护独立 README/INDEX/TODO。
- 备选方案：根目录维护完整细节（被拒绝，上下文易膨胀）。

## ADR-0003 v1 协议与运行形态默认值

- 日期：2026-03-08
- 状态：accepted
- 背景：phase-2 到 phase-5 文档补全需要固定实现前提，避免跨阶段反复讨论。
- 决策：
  - 流式协议：HTTP + SSE
  - 鉴权：API Key
  - Gateway 运行形态：内嵌 openjax-core
- 影响：
  - phase-2 协议文档按该组合冻结。
  - phase-3/4/5 仅引用已冻结契约，不额外引入 WebSocket/JWT/daemon 双栈复杂度。
- 备选方案：WebSocket、JWT、daemon 兼容双轨（保留为后续演进选项）。

## ADR-0004 Phase-1 架构冻结默认项

- 日期：2026-03-08
- 状态：accepted
- 背景：phase-1 需要完成可执行级架构定义，避免 phase-2/3 反复漂移。
- 决策：
  - Gateway 运行模式固定为“内嵌 openjax-core”。
  - Gateway 与 Core 边界固定：Gateway 负责协议/鉴权/会话编排/观测；Core 负责 agent/tool/sandbox。
  - NFR 采用可测数字门槛（用于 phase-5 验收）。
  - v1 明确单租户个人部署，不考虑多租户。
  - 阶段状态修正：phase-1 `in_progress`，phase-2 `blocked`。
- 影响：
  - phase-2 在 phase-1 冻结前不可推进为实施阶段。
  - 后续阶段不得引入多租户或双轨运行复杂度。
- 备选方案：phase-1 与 phase-2 并行推进（被拒绝，增加返工与上下文膨胀风险）。

## ADR-0005 会话 clear/compact 与命令桥接

- 日期：2026-03-08
- 状态：accepted
- 背景：第三方聊天软件（如 Telegram）通常是单对话入口，缺少“新建对话”按钮，需要通过命令触发上下文管理。
- 决策：
  - 在 v1 协议中增加显式接口：`clear`、`compact`。
  - 命令识别放在 Gateway 层，默认映射 `/clear`、`/compact` 到对应 API。
  - `compact` 在 core 未实现前返回 `NOT_IMPLEMENTED`，保留协议不变。
- 影响：
  - Web/移动端/IM 机器人共享同一会话管理能力。
  - core 不承担聊天命令解析职责，保持执行内核边界稳定。
- 备选方案：仅做文本命令隐式处理（被拒绝，协议不可见且审计困难）。

## ADR-0006 双输出模式（SSE + Non-Streaming Polling）

- 日期：2026-03-08
- 状态：accepted
- 背景：部分第三方聊天软件不支持流式渲染，需要非流式结果获取方式。
- 决策：
  - v1 同时支持两种输出模式：
    - 流式：SSE `stream_events`
    - 非流式：`submit_turn` 后轮询 `GET /sessions/{session_id}/turns/{turn_id}`
  - 两种模式共享同一 `turn_id`、错误模型与鉴权策略。
- 影响：
  - Web 可继续使用流式体验。
  - Telegram 等第三方接入可使用轮询模式稳定工作。
- 备选方案：仅支持流式（被拒绝，三方接入兼容性不足）。
