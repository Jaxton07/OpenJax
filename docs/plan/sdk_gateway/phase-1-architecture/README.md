执行前先看 WORKFLOW + TODO

# Phase 1 - Architecture

## 目标

冻结系统目标架构与边界：明确 `Rust Core`、`Rust Gateway`、`React` 的职责、依赖方向和数据流。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [../phase-0-foundation/01-terms-and-boundaries.md](../phase-0-foundation/01-terms-and-boundaries.md)
4. [./TODO.md](./TODO.md)

## 输入产物

- phase-0 术语与边界
- 当前仓库模块现状（core/protocol/daemon/cli/ui）

## 输出产物

- 目标架构文档
- 组件职责与接口边界
- 非功能性要求（性能、稳定性、可维护性）

## 开工条件（Go/No-Go）

- phase-0 门禁检查通过
- 路线已锁定为 Rust + Rust Gateway + React

## 完成定义（DoD）

- 架构图与职责边界可指导 phase-2 协议设计
- phase-1 TODO 全部闭合

## 关联代码入口

- `openjax-core/src/agent`
- `openjax-core/src/tools`
- `openjax-protocol/src/lib.rs`
- `openjaxd/src/main.rs`

## 回写要求

- 如边界变化，必须同步更新 `../DECISIONS.md`
- 完成后更新根 `README.md` 与根 `TODO.md` 状态
