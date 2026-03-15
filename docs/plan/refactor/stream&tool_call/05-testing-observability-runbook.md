# 05. 测试、观测与排障手册

## 1. 测试矩阵

### 1.1 单元测试（core）

1. Dispatcher 状态机转移：
   - `PROBING -> LOCKED_TEXT`
   - `PROBING -> LOCKED_TOOL_CALL`
   - probe timeout 行为
2. 误发防护：
   - tool_call 分支不发文本 delta
3. 参数收敛：
   - `tool_args_delta` 拼接与 JSON 有效性判定
4. 分支锁定一致性：
   - 同 turn 不允许分支反复切换

### 1.2 集成测试（gateway + webui）

1. 纯文本请求：
   - 连续流输出、自动滚动、完成收敛
2. 工具请求：
   - 工具卡片、审批卡片、执行状态完整展示
3. 断线恢复：
   - `after_event_seq` 回放成功
   - 回放越窗错误提示正确
4. 兼容性：
   - 老前端仍可消费事件

### 1.3 性能压测

1. 长文本连续输出（>5k 字符）
2. 高频 delta（每秒数十到百级）
3. 混合场景（文本 + 多工具调用）

## 2. 关键指标（必须埋点）

按 turn 采样下列时间点：

1. `provider_recv_ts`
2. `dispatcher_lock_ts`
3. `gateway_emit_ts`
4. `browser_recv_ts`
5. `browser_commit_ts`

输出指标：

1. `ttft_ms` = first_token - submit
2. `dispatch_probe_ms`
3. `delta_emit_gap_p50/p95`
4. `browser_commit_gap_p50/p95`
5. `mistaken_branch_count`（误分支计数）
6. `tool_args_incomplete_count`

## 3. 日志建议

1. 生产默认采样日志，不逐 delta 打印。
2. Debug 模式可开启：
   - 分支判定日志
   - tool 参数收敛日志
   - 每 turn 汇总指标日志
3. 日志分级：
   - `INFO`：turn 级摘要
   - `DEBUG`：判定细节与事件节流统计
   - `WARN/ERROR`：分支冲突、协议异常、执行失败

## 4. 常见问题排查

1. 症状：输出“一顿一顿”
   - 检查 provider 侧 delta 间隔
   - 检查 dispatcher probe 是否过长
   - 检查 gateway 是否逐事件重日志
2. 症状：tool 事件跑到文本区
   - 检查分支锁定是否生效
   - 检查前端是否仅渲染 `response_text_delta`
3. 症状：审批卡片不出现
   - 检查工具执行层是否发 `approval_requested`
   - 检查权限策略是否短路

## 5. 验收标准（Go/No-Go）

1. 纯文本体验达到“连续、丝滑、可感知优于旧链路”。
2. tool/approval 路径无语义回归。
3. 新旧协议兼容，不阻断现有客户端。
4. 出现异常时可通过开关一键回退。

