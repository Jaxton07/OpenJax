执行前先看 WORKFLOW + TODO

# Phase 6 - Gateway Development

## 目标

落地 Rust Gateway 代码实现，覆盖会话 API、双输出模式（SSE + Polling）、审批、clear/compact 命令桥接与可观测。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [../phase-2-protocol-standards/INDEX.md](../phase-2-protocol-standards/INDEX.md)
4. [../phase-3-gateway-design/INDEX.md](../phase-3-gateway-design/INDEX.md)
5. [./TODO.md](./TODO.md)

## 输入产物

- phase-2 协议契约（已冻结）
- phase-3 Gateway 设计（已评审通过）

## 输出产物

- `openjax-gateway` 可运行服务
- 契约一致的 API/SSE/Polling 能力
- 基础日志、指标、审计埋点

## 开工条件（Go/No-Go）

- phase-2 和 phase-3 均为 done
- 协议变更流程已锁定（先 phase-2，再 DECISIONS）

## 完成定义（DoD）

- 核心接口实现完成并通过基础集成验证
- clear 已可用，compact 至少返回 `NOT_IMPLEMENTED` 或已实现
- 关键路径日志与错误码符合 phase-2 约束

## 关联代码入口

- `openjax-core/`
- `openjaxd/`
- `openjax-protocol/`

## 回写要求

- 任何协议字段变更先回写 phase-2 与 DECISIONS
