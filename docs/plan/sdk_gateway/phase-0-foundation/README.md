执行前先看 WORKFLOW + TODO

# Phase 0 - Foundation

## 目标

建立统一术语、范围边界、阶段开工门禁，确保后续阶段在同一语义和约束下执行。

## 前置阅读

1. [../WORKFLOW.md](../WORKFLOW.md)
2. [../TODO.md](../TODO.md)
3. [./TODO.md](./TODO.md)

## 输入产物

- 当前仓库结构与模块现状
- 路线决策：`Rust 内核 + Rust Gateway + React`

## 输出产物

- 术语表与范围边界文档
- 阶段门禁（Go/No-Go）
- 后续阶段文档骨架

## 开工条件（Go/No-Go）

- `WORKFLOW` 已确认并可执行
- 全局/阶段 TODO 已创建并可追踪
- phase-1 的入口与任务边界可定位

## 完成定义（DoD）

- phase-0 `TODO.md` 全部任务完成
- phase-1 可以在不读取其他阶段细节的情况下开工

## 关联代码入口

- `openjax-core/`
- `openjax-protocol/`
- `openjaxd/`

## 回写要求

- 变更阶段状态时同步更新根 `README.md` 与根 `TODO.md`
- 决策变化同步记录到 `../DECISIONS.md`
