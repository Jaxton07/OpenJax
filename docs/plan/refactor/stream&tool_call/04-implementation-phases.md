# 04. 分步实施计划

## 阶段 0：准备与开关

1. 增加统一特性开关：
   - `OPENJAX_STREAM_DISPATCHER_ENABLED=1`
   - `OPENJAX_STREAM_DISPATCH_PROBE_MS=80`
   - `OPENJAX_STREAM_TEXT_FAST_PATH_DEFAULT=1`
2. 保留现有 `OPENJAX_DIRECT_PROVIDER_STREAM`，作为回退兜底开关。
3. 在日志中打印当前策略矩阵，便于线上确认。

验收：

1. 开关关闭时行为与当前版本一致。
2. 开关开启时无编译告警，核心路径可运行。

## 阶段 1：实现 Dispatcher（不改外部协议）

1. 在 `openjax-core` 增加 dispatcher 模块：
   - 输入：provider stream delta + structured tool signal
   - 输出：锁定分支后的事件流
2. 实现 `PROBING -> LOCKED_*` 状态机。
3. 分发器只做判定与路由，不触碰审批逻辑。

验收：

1. 纯文本 turn 走 `LOCKED_TEXT`，输出连续。
2. tool_call turn 走 `LOCKED_TOOL_CALL`，无文本误发。

## 阶段 2：Orchestrator 分支对接与参数收敛

1. 工具参数增量缓冲与收敛：
   - `tool_args_delta` 累积
   - 参数 JSON 校验通过后发 `tool_call_ready`
2. 工具执行器对接审批：
   - 触发 `approval_requested/resolved`
3. 执行状态事件标准化：
   - `started/progress/completed/failed`

验收：

1. 工具参数不完整时不会提前执行。
2. 审批卡片只在工具执行层触发。

## 阶段 3：Gateway 事件瘦身与兼容处理

1. 核心目标：减少重复语义事件。
2. 建议策略：
   - `assistant_message` 降级为兼容，前端默认不依赖。
   - 以 `response_*` 为唯一文本主路径。
3. 保持 `event_seq` 回放语义不变。

验收：

1. 旧前端可继续使用。
2. 新 WebUI 在瘦身模式下事件量下降且功能完整。

## 阶段 4：灰度发布与回滚预案

1. 灰度维度：
   - 按环境
   - 按用户组
   - 按模型路由
2. 观测通过后逐步扩大流量。
3. 回滚策略：
   - 一键关闭 dispatcher 开关，恢复旧路径。

## 里程碑与交付物

1. M1：Dispatcher + 单测通过。
2. M2：Tool call 收敛 + 审批事件链路通过。
3. M3：Gateway 兼容瘦身 + WebUI 无感切换。
4. M4：灰度稳定，默认启用新路径。

