# Streaming & Tool Call 重构计划

本文档集用于在实际代码改动前，统一本次重构的范围、目标架构、协议定义、分步开发任务与验收标准。

## 文档索引
- [01-目标与范围](file:///Users/ericw/work/code/ai/openJax/docs/plan/refactor/stream&tool_call/01-目标与范围.md)
- [02-单阶段执行引擎架构](file:///Users/ericw/work/code/ai/openJax/docs/plan/refactor/stream&tool_call/02-单阶段执行引擎架构.md)
- [03-事件协议-v2](file:///Users/ericw/work/code/ai/openJax/docs/plan/refactor/stream&tool_call/03-事件协议-v2.md)
- [04-tool_calls与调度设计](file:///Users/ericw/work/code/ai/openJax/docs/plan/refactor/stream&tool_call/04-tool_calls与调度设计.md)
- [05-迁移与灰度策略](file:///Users/ericw/work/code/ai/openJax/docs/plan/refactor/stream&tool_call/05-迁移与灰度策略.md)
- [06-分步开发任务](file:///Users/ericw/work/code/ai/openJax/docs/plan/refactor/stream&tool_call/06-分步开发任务.md)
- [07-验收与检查清单](file:///Users/ericw/work/code/ai/openJax/docs/plan/refactor/stream&tool_call/07-验收与检查清单.md)
- [08-生产灰度执行清单](file:///Users/ericw/work/code/ai/openJax/docs/plan/refactor/stream&tool_call/08-生产灰度执行清单.md)

## 关键原则
- 采用原路径就地重构，不建立平行产品目录。
- 默认路径切换到真实流式；合成增量仅作为受控降级。
- 支持模型一次返回多个 tool calls，并按依赖并发执行。
- 工具结果使用结构化结果回注模型，替代纯文本 trace 拼接。
- 先定义契约，再做实现，最后做灰度与观测闭环。
