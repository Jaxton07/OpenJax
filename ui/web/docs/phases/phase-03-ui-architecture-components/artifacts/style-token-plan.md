# Style & Token Plan (Frozen v1)

## 命名策略
- 状态类统一采用 BEM 修饰符:
  - 卡片: `step-card step-card--{status}`
  - 徽标: `step-status step-status--{status}`
- 结构类:
  - `step-head`、`step-meta`、`step-body`、`step-code`、`step-output`
- 禁止复用 `status-{status}` 这类无前缀类名，避免与现有消息样式串色。

## 与现有样式对齐
- 优先复用 `ui/web/src/styles/app.css` 里的基础 token（边框、文字、背景、间距）。
- 新增 token 仅在工具步骤块内声明，避免污染全局。
- 首版样式放在 `app.css` 的 tool step 分区；后续再按体量拆独立文件。

## 状态色建议（v1）
- `running`: 中性蓝
- `success`: 中性绿
- `waiting`: 中性黄/棕
- `failed`: 中性红

## 响应式要求
- 移动端卡片宽度跟随容器，不得横向溢出。
- `code/output` 使用 `white-space: pre-wrap` + `overflow-wrap: anywhere`。
- 高度超限时使用 `max-height + overflow: auto`。
