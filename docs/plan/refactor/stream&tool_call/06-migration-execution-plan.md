# 06 Migration Execution Plan

状态：`done`

## PR 切分模板（PR-A ~ PR-E）

1. PR-A（阶段 1+2）
- 变更范围：目标架构冻结、事件契约冻结、移除遗留事件分支。
- 回归命令：`cargo check`
- 风险点：事件契约变更导致消费者 match 缺失。
- 回滚策略：整包回滚协议与消费者改动，恢复上个稳定 tag。

2. PR-B（阶段 3）
- 变更范围：provider 流读取切换到 `streaming/parser`，orchestrator 全链路接入。
- 回归命令：`cargo check`，`cargo test -p openjax-core --test m6_submit_stream`
- 风险点：provider 边界条件下 SSE 尾包处理。
- 回滚策略：仅回滚 provider parser 接入提交，保留 streaming 基础模块。

3. PR-C（阶段 4）
- 变更范围：gateway mapper 拆分、`ReplayBuffer` 装配、state 职责瘦身。
- 回归命令：`cargo check`，`cargo test -p openjax-gateway`
- 风险点：SSE 恢复和回放窗口错误语义回退。
- 回滚策略：回滚 gateway mapper 模块与 state 路由变更。

4. PR-D（阶段 5）
- 变更范围：工具生命周期补齐 `args_delta/progress/failed`，审批拒绝/超时失败映射。
- 回归命令：`cargo check`，`cargo test -p openjax-core --test m21_tool_streaming_events`，`cargo test -p tui_next`
- 风险点：旧消费者仅依赖 `ToolCallCompleted` 的行为差异。
- 回滚策略：保留新事件定义，回滚 core 发射点改动。

5. PR-E（阶段 7+8）
- 变更范围：测试矩阵收口、指标命名统一、runbook 与文档状态更新。
- 回归命令：`cargo test --workspace`
- 风险点：文档与代码埋点命名不一致。
- 回滚策略：文档可独立回滚，不影响运行时路径。

## 执行原则

1. 每个 PR 都可独立 `cargo check` 与关键测试通过。
2. 每个 PR 提供回归命令与结果。
3. 严禁跨 PR 混入无关改动。
