执行前先看 WORKFLOW + TODO

# Phase 4 - Web React

## 目标

定义 React 前端在对话和管理场景下的交互模型与接入契约，确保与 phase-2 协议和 phase-3 设计一致。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [../phase-2-protocol-standards/INDEX.md](../phase-2-protocol-standards/INDEX.md)
4. [../phase-3-gateway-design/INDEX.md](../phase-3-gateway-design/INDEX.md)
5. [./TODO.md](./TODO.md)

## 输入产物

- phase-2 协议契约
- phase-3 网关行为约束

## 输出产物

- 前端页面信息架构
- 会话与流式状态机
- SDK 接入约束
- 错误与重连策略
- API Key 交互与权限提示

## 开工条件（Go/No-Go）

- phase-2 协议冻结
- phase-3 状态与失败语义已稳定

## 完成定义（DoD）

- 本阶段 `01-05` 文档齐备，可指导 React 实现
- 不引入协议外字段与事件

## 关联代码入口

- `ui/tui/`（交互行为参考）
- `python/openjax_sdk/src/openjax_sdk`（SDK 交互参考）

## 回写要求

- 若前端需要新字段，先回写 phase-2 契约与 DECISIONS
