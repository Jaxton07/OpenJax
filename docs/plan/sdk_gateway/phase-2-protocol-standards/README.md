执行前先看 WORKFLOW + TODO

# Phase 2 - Protocol Standards

## 目标

冻结网关 v1 协议与标准，作为后续 Gateway 实现与前端接入的唯一契约来源。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [../phase-1-architecture/01-target-architecture.md](../phase-1-architecture/01-target-architecture.md)
4. [./TODO.md](./TODO.md)

## 输入产物

- phase-1 目标架构与职责边界
- 当前 Rust Core/Protocol/Daemon 能力现状

## 输出产物

- API 契约（HTTP + SSE）
- 事件模型与错误模型
- 版本与兼容策略
- API Key 安全基线

## 开工条件（Go/No-Go）

- phase-1 边界已稳定，不再调整核心职责分层
- 路线决策固定：HTTP + SSE / API Key / Gateway 内嵌 openjax-core

## 完成定义（DoD）

- 本阶段 `01-05` 文档齐备并可独立指导 phase-3/4
- `TODO.md` 任务全部完成并有验收记录
- phase-3/4 文档仅引用本阶段契约，不自行扩展未登记字段

## 关联代码入口

- `openjax-protocol/src/lib.rs`
- `openjax-core/src/agent`
- `openjaxd/src/main.rs`

## 回写要求

- 协议字段或语义变化，先改本阶段文档，再更新 `../DECISIONS.md`
- 变更完成后由引用方（phase-3/4/5）同步回链
