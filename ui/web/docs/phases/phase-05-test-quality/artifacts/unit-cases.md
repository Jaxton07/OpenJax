# Unit Test Cases

## reducer
- tool_call_started 创建 running step。
- tool_call_completed 更新为 success 并写 output。
- approval_requested 进入 waiting。
- error 将当前 step 置 failed 或创建失败 step。
- 未知事件不抛异常。

## component
- 默认折叠渲染。
- 点击切换展开/收起。
- 缺少 code/output 时不渲染对应模块。
