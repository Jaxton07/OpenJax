# Stream + Tool Call 重构方案索引

本目录用于固化 OpenJax 在“高性能流式输出 + 工具调用编排”方向的最终重构方案。

核心目标：

1. 默认获得丝滑、低延迟的文本流式体验。
2. 保留并强化工具调用、审批、人机协作能力。
3. 明确边界：数据面（高频文本流）与控制面（低频编排事件）分离。
4. 可回放、可观测、可灰度，不破坏现有网关接入模型。

## 文档导航（建议阅读顺序）

1. [01-goals-and-decisions.md](./01-goals-and-decisions.md)
   - 背景问题、设计原则、最终决策与非目标
2. [02-architecture-and-dispatcher.md](./02-architecture-and-dispatcher.md)
   - 总体架构、分发器状态机、直通流/编排流双分支
3. [03-stream-tool-protocol.md](./03-stream-tool-protocol.md)
   - 事件协议、字段约束、判定规则、兼容策略
4. [04-implementation-phases.md](./04-implementation-phases.md)
   - 代码改造分阶段计划、里程碑、回滚策略
5. [05-testing-observability-runbook.md](./05-testing-observability-runbook.md)
   - 测试矩阵、性能指标、排障与值班手册

## 一页结论

采用“单入口分发器 + 双分支执行器”：

1. `Fast Path`（默认）
   - 纯文本任务走直通流，尽量直连 provider delta 到前端 SSE。
2. `Orchestrator Path`（按需）
   - 检测到工具调用意图后切换到 ReAct 编排分支，收齐参数再执行工具。
3. 分发器只做路由，不做审批判断
   - 审批逻辑放在工具执行层，避免分发器膨胀。
4. 防误发机制
   - 不依赖“自然语言猜测”。
   - 使用结构化 tool-call 信号 + 短暂判定缓冲窗，避免把工具片段误推送到文本流。

## 术语

1. 数据面（Data Plane）：`response_text_delta` 等高频文本事件。
2. 控制面（Control Plane）：`tool_call_*`、`approval_*`、`turn_*` 等低频状态事件。
3. 分发器（Dispatcher）：在模型流早期阶段判定并锁定分支的组件。
4. 分支锁定（Branch Lock）：一个 turn 内一旦判定分支，后续不可反复切换。

