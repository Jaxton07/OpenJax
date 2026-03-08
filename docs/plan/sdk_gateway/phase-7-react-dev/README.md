执行前先看 WORKFLOW + TODO

# Phase 7 - React Development

## 目标

落地 React Web 界面与前端状态机，实现对话、审批、错误恢复、clear/compact 入口及双模式接入。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [../phase-2-protocol-standards/INDEX.md](../phase-2-protocol-standards/INDEX.md)
4. [../phase-4-web-react/INDEX.md](../phase-4-web-react/INDEX.md)
5. [./TODO.md](./TODO.md)

## 输入产物

- phase-2 协议契约
- phase-4 Web 设计文档（已评审通过）

## 输出产物

- React 页面（会话、聊天、设置）
- 客户端状态机与错误恢复能力
- clear/compact 与审批交互入口

## 开工条件（Go/No-Go）

- phase-4 为 done
- phase-6 至少提供可联调 Gateway 基础接口

## 完成定义（DoD）

- 核心页面可完成端到端对话
- SSE 与 Polling 至少一种可稳定运行，另一种具备兼容路径
- 审批与错误处理体验符合 phase-4 设计

## 关联代码入口

- 前端工作目录（待创建）
- `phase-4-web-react` 设计文档

## 回写要求

- 前端需要新增协议字段时，先回写 phase-2 与 DECISIONS
