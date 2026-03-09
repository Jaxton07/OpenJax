# Interaction & A11y Spec

## 交互规则
- 点击卡片头部切换展开/收起。
- 展开时显示 `description/code/output`。
- 无 `code/output` 时对应区域不渲染。

## 可访问性规则
- 折叠按钮必须有 `aria-expanded`。
- 折叠按钮必须绑定 `aria-controls` 到 body 区域。
- 键盘支持 `Enter` 和 `Space` 触发展开。
- 焦点可见性必须保留。
