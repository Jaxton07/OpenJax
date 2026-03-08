执行前先看 WORKFLOW + TODO

# Phase 3 - Gateway Design

## 目标

基于 phase-2 协议契约完成 Rust Gateway 设计，覆盖运行拓扑、请求生命周期、状态模型、可观测与失败处理。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [../phase-2-protocol-standards/INDEX.md](../phase-2-protocol-standards/INDEX.md)
4. [./TODO.md](./TODO.md)

## 输入产物

- phase-2 冻结协议
- openjax-core 当前执行模型

## 输出产物

- Gateway 运行架构
- 全链路请求与事件流设计
- 会话/回合状态模型
- 可观测与故障处理策略

## 开工条件（Go/No-Go）

- phase-2 协议文档已冻结
- 不新增未登记 API 字段或事件语义

## 完成定义（DoD）

- 本阶段 `01-05` 文档齐备，可直接指导网关实现
- 文档内所有接口字段仅引用 phase-2 契约

## 关联代码入口

- `openjax-core/src/agent`
- `openjax-core/src/tools`
- `openjaxd/src/main.rs`

## 回写要求

- 若需要协议扩展，先回写 phase-2 与 DECISIONS，再更新本阶段文档
