执行前先看 WORKFLOW + TODO

# Phase 5 - Delivery & Ops

## 目标

定义网关与前端交付阶段的测试门禁、发布流程、运维手册与风险回滚策略。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [../phase-2-protocol-standards/INDEX.md](../phase-2-protocol-standards/INDEX.md)
4. [../phase-3-gateway-design/INDEX.md](../phase-3-gateway-design/INDEX.md)
5. [../phase-4-web-react/INDEX.md](../phase-4-web-react/INDEX.md)
6. [./TODO.md](./TODO.md)

## 输入产物

- phase-2 协议契约
- phase-3/4 设计文档

## 输出产物

- 测试与验收方案
- 发布前检查清单
- 运行手册
- 风险与回滚方案
- 发布后复盘模板

## 开工条件（Go/No-Go）

- phase-2/3/4 文档完成并可实现
- 关键接口与事件语义不再变更

## 完成定义（DoD）

- 本阶段 `01-05` 文档齐备并可直接用于发版执行
- 每个发布门禁均可对应到具体验收项

## 关联代码入口

- `openjax-core/`
- `openjaxd/`
- `ui/tui/`（交互验收行为参考）

## 回写要求

- 发布后复盘结果若涉及协议/架构调整，需回写 DECISIONS 与 phase-2/3 文档
