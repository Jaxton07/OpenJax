# SDK Gateway 文档体系总览

本目录是 `Rust 内核 + Rust Gateway + React 前端` 路线的唯一权威计划入口。

## 当前状态面板

- 当前阶段：`phase-6-gateway-dev`（in_progress）
- 当前里程碑：phase-4 设计阶段收口，进入网关代码实施
- 下一步：初始化网关 crate，打通会话 API 与基础事件流

## 阅读顺序（固定）

1. [WORKFLOW.md](./WORKFLOW.md)
2. [TODO.md](./TODO.md)
3. 当前阶段的 `README.md`
4. 当前阶段的 `TODO.md`
5. 当前任务对应文档

## 阶段导航

- [phase-0-foundation](./phase-0-foundation/README.md)
- [phase-1-architecture](./phase-1-architecture/README.md)
- [phase-2-protocol-standards](./phase-2-protocol-standards/README.md)
- [phase-3-gateway-design](./phase-3-gateway-design/README.md)
- [phase-4-web-react](./phase-4-web-react/README.md)
- [phase-5-delivery-ops](./phase-5-delivery-ops/README.md)
- [phase-6-gateway-dev](./phase-6-gateway-dev/README.md)
- [phase-7-react-dev](./phase-7-react-dev/README.md)
- [phase-8-core-enhancements](./phase-8-core-enhancements/README.md)

## 已完成阶段摘要（仅三行）

### phase-0-foundation

- 目标：建立文档治理规则、边界和门禁。
- 结果：两层 TODO + 低上下文膨胀工作流已固化。
- 链接：[phase-0 README](./phase-0-foundation/README.md)

### phase-1-architecture

- 目标：冻结系统架构、边界职责与 NFR 门槛。
- 结果：Gateway/Core 边界与 v1 约束已固定，可直接驱动 phase-2。
- 链接：[phase-1 README](./phase-1-architecture/README.md)

### phase-2-protocol-standards

- 目标：冻结 v1 协议、事件、错误、兼容与安全基线。
- 结果：完成 clear/compact 与双输出模式（SSE + Polling）契约并通过评审。
- 链接：[phase-2 README](./phase-2-protocol-standards/README.md)

### phase-3-gateway-design

- 目标：完成 Gateway 运行、状态、故障与可观测的实现级设计。
- 结果：设计文档与评审清单均通过，可直接作为 phase-4/开发输入。
- 链接：[phase-3 README](./phase-3-gateway-design/README.md)

### phase-4-web-react

- 目标：完成 Web 端信息架构、状态机、接入与恢复策略设计。
- 结果：设计文档与评审清单均通过，可直接进入 React 开发阶段。
- 链接：[phase-4 README](./phase-4-web-react/README.md)

## 维护约定

- 新增细节文档必须挂到对应阶段 `INDEX.md`，禁止散落在根目录。
- 阶段切换时，仅更新本文件“当前状态面板”和根 [TODO.md](./TODO.md)。
- 重大方案调整先记录到 [DECISIONS.md](./DECISIONS.md)。
