# Style & Token Plan

## 命名策略
- 卡片状态类：`step-card--{status}`。
- 徽标状态类：`step-status--{status}`。
- 避免复用同一全局类造成串色。

## 与现有样式对齐
- 字体、间距、边框优先沿用 `ui/web/src/styles/app.css` 现有 token。
- 新增样式保持低耦合，可放独立块后再考虑合并。

## 响应式要求
- 移动端不溢出。
- 长命令与长输出支持换行和滚动。
