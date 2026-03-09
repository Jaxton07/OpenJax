# Phase 02 - Data Contract & Protocol Mapping

## 目标
- 定义 Tool Step 的结构化数据契约与 Gateway 事件映射规则。

## 边界
- In Scope:
  - ToolStep 字段与状态集合
  - StreamEvent 到 Step 的映射
  - 向后兼容和降级规则
- Out of Scope:
  - UI 组件实现细节

## 输入
- Phase 01 范围与差异文档
- 现有 chat types 与 event reducer

## 输出
- `artifacts/message-model.md`
- `artifacts/gateway-event-mapping.md`
- `artifacts/backward-compat.md`

## 完成判定
- 数据契约冻结并通过评审。
- 所有关键事件映射有明确归属。
