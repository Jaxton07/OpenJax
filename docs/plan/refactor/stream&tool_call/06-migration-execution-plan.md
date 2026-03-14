# 06 Migration Execution Plan

状态：`planned`

## PR 切分建议

1. PR-1（已完成）
- 新增 streaming 模块骨架。
- planner 接入 orchestrator，移除字符级 delta 发射。
- protocol 增加 tool 流式事件。

2. PR-2
- provider 层改为共用 parser（openai/anthropic）。
- 删除 provider 内重复 SSE 行解析。

3. PR-3
- gateway mapper 拆分与 replay 抽象替换。
- 可配置 replay window/channel capacity。

4. PR-4
- 工具执行链路发射 args_delta/progress/failed。
- approval 中断语义统一。

5. PR-5
- 清理旧事件路径与文档更新。

## 执行原则

1. 每个 PR 都可独立 `cargo check` 与关键测试通过。
2. 每个 PR 提供回归命令与结果。
3. 严禁跨 PR 混入无关改动。
