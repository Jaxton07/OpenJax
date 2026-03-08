执行前先看 WORKFLOW + TODO

# Phase 8 - Core Enhancements

## 目标

补齐 `openjax-core` 上下文管理增强，重点实现 compact 能力并完善 clear/compact 对应事件语义。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [../phase-2-protocol-standards/INDEX.md](../phase-2-protocol-standards/INDEX.md)
4. [../phase-6-gateway-dev/INDEX.md](../phase-6-gateway-dev/INDEX.md)
5. [./TODO.md](./TODO.md)

## 输入产物

- phase-2 契约中 clear/compact 定义
- phase-6 网关落地中的上下文管理调用需求

## 输出产物

- core compact 能力（从 `NOT_IMPLEMENTED` 升级为可执行）
- clear/compact 对应的稳定行为与事件
- 回归测试与兼容性验证

## 开工条件（Go/No-Go）

- phase-6 已验证 clear 行为可用
- compact 需求边界在协议层稳定

## 完成定义（DoD）

- compact 在 core 可执行并满足稳定性要求
- 协议层无需破坏性变更即可接入 compact 实现
- 关键测试通过并可回归

## 关联代码入口

- `openjax-core/src/agent`
- `openjax-core/src/model`
- `openjax-core/src/tools`

## 回写要求

- 若 compact 行为需要协议变更，先回写 phase-2 与 DECISIONS
