# Backward Compatibility Plan

## 兼容目标
- 老会话、老事件、不完整 payload 不得导致页面崩溃。

## 策略
- 消息降级:
  - 无结构化字段时自动降级纯文本渲染。
  - `toolSteps` 非数组或空数组时，按 `content` 渲染。
- 事件容错:
  - 未知事件类型记录日志并跳过，不中断会话流。
  - payload 字段缺失时用默认值，不抛异常。
- 字段默认值:
  - `title=tool`
  - `status=running`
  - `time=event.timestamp`
  - `output=""`（仅在需要展示输出时填充）
- 审批兼容:
  - 即使步骤流不完整，ApprovalPanel 仍按 `approval_id` 独立可用。

## 验证清单
- 旧会话历史加载可读。
- 新旧消息混排无布局错误。
- reducer 遇到未知事件不抛异常。
- 缺少 `tool_call_id` 的事件仍可生成可读 step。
- 审批事件缺少 `tool_call_id` 不影响审批流转。
