# Phase 04 - Implementation & Integration

## 目标
- 把实现拆成可并行、可分 PR、可独立验收的轨道执行计划。

## 边界
- In Scope:
  - Track A: 状态与 reducer
  - Track B: 消息渲染
  - Track C: Tool 卡片 UI/样式
  - 合并顺序与依赖定义
- Out of Scope:
  - 灰度发布策略（Phase 06）

## 输入
- Phase 02 数据契约
- Phase 03 组件与交互规范

## 输出
- 三条 track 任务清单
- PR 粒度拆分计划

## 依赖关系
- Track A 先于 Track B。
- Track B 与 Track C 可部分并行，最终在 MessageList 汇合。
