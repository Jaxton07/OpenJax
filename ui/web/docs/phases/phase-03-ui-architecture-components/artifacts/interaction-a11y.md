# Interaction & A11y Spec (Frozen v1)

## 交互规则
- 默认折叠，点击卡片头部切换展开/收起。
- 展开时显示 `description/code/output`。
- 无 `code/output` 时对应区块不渲染（不保留空占位）。
- 状态展示优先使用 `status` 字段，禁止从文案推导状态。
- `failed` 与 `waiting` 状态在折叠态也必须可见。

## 异常与边界交互
- 字段缺失时展示安全默认值（如 `title=tool`），不抛 UI 异常。
- 超长 `code/output` 支持折行与垂直滚动，不撑破容器。
- 多步骤混排时，单卡片展开不影响其他卡片状态。

## 可访问性规则
- 折叠按钮元素使用 `button`（禁止 `div` + onClick）。
- 折叠按钮必须有 `aria-expanded`。
- 折叠按钮必须绑定 `aria-controls` 到详情容器 id。
- 详情容器建议加 `role="region"` 并设置 `aria-labelledby`。
- 键盘支持 `Enter` 和 `Space` 触发展开（`button` 原生支持）。
- 焦点可见性必须保留，不可全局去掉 `outline`。

## 测试检查点（供 Phase 05 复用）
- 键盘 Tab 可聚焦到每个步骤头部。
- 屏幕阅读器可读出展开状态变化。
- 无详情字段时不出现空白 `region`。
