# 05 Access and Permission UI

## API Key 输入流程

- 用户在设置页输入 API Key。
- 前端本地安全存储策略由实现阶段确定（需避免明文日志）。
- 每次请求自动附加 `Authorization` 头。

## 权限反馈

- 401 显示认证失效并引导重新配置。
- 403 显示权限不足并提示联系管理员或切换 key。

## 审批交互

- 接收 `approval_requested` 后弹出审批面板。
- 用户提交后调用 `resolve_approval`。
- 接收 `approval_resolved` 后关闭面板并更新消息流状态。
