# `src/components` 索引

该目录存放 Web UI 的可视组件与组件级测试，重点是聊天主界面、设置弹窗、以及工具步骤卡片。

## 目录职责

- 页面级组件：`LoginPage`、`SettingsModal`
- 聊天核心组件：`Sidebar`、`MessageList`、`Composer`
- 设置子模块：`settings/*`
- Tool Step 子模块：`tool-steps/*`

## 关键文件

- `SettingsModal.tsx`：设置弹窗入口，组合通用设置和 Provider 管理。
- `settings/ProviderEditorPanel.tsx`：Provider 编辑卡片壳层（关闭按钮、动画容器、滚动重置）。
- `settings/ProviderForm.tsx`：Provider 创建/编辑表单内容。
- `settings/ProviderListPanel.tsx`：Provider 列表、选择/编辑/删除入口。
- `MessageList.tsx`：消息渲染分发（文本、tool steps、reasoning）。
- `tool-steps/ToolStepList.tsx`：Tool Step 列表容器。
- `tool-steps/ToolStepCard.tsx`：单个 Tool Step 卡片。
- `tool-steps/ApprovalStepCard.tsx`：审批类步骤卡片。

## 测试文件

- `MessageList.test.tsx`
- `SettingsModal.test.tsx`
- `tool-steps/*.test.tsx`

## 维护建议

- 新增设置相关 UI，优先放在 `settings/` 子目录。
- Tool Step 相关改动尽量限制在 `tool-steps/`，减少对聊天主渲染链路的影响。

## 上层文档

- 返回 Web 模块总文档：[ui/web/README.md](../../README.md)
