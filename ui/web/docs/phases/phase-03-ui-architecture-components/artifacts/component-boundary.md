# Component Boundary Plan

## 建议组件
- `ToolStepList`: 负责步骤列表容器与排序。
- `ToolStepCard`: 单步骤卡片主体。
- `StepStatusBadge`: 状态标签展示。
- `StepBody`: 描述、代码、输出详情区。

## 责任划分
- 容器组件负责数据与事件绑定。
- 展示组件负责渲染，不直接依赖 reducer 细节。

## 复用原则
- 普通消息路径不受影响。
- ToolStep 组件可单测渲染，不依赖真实网络。
