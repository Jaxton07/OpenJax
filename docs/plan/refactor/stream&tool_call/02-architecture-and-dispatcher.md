# 02. 架构与分发器设计

## 1. 总体架构

```text
Provider Stream
   |
   v
[Stream Dispatcher]
   |-- Fast Path (text passthrough) ----------> Response SSE (data plane)
   |
   |-- Orchestrator Path (ReAct loop) --------> Tool/Approval/Turn SSE (control plane)
```

核心原则：

1. 单入口统一接收模型流。
2. 一次判定后锁定分支，turn 内不反复切换。
3. 数据面与控制面事件分开设计和处理。

## 2. 分发器职责边界

分发器负责：

1. 启动判定窗口，收集早期流片段。
2. 识别是否存在结构化工具调用信号。
3. 锁定分支并把流移交执行器。
4. 输出判定日志与统计指标。

分发器不负责：

1. 工具审批策略判断。
2. 工具执行超时重试。
3. 业务层面的“是否允许某工具”策略。

## 3. 状态机

```text
INIT
  -> PROBING
PROBING
  -> LOCKED_TEXT         (检测到纯文本信号且窗口结束)
  -> LOCKED_TOOL_CALL    (检测到结构化 tool_call 信号)
  -> LOCKED_TEXT         (超时未见 tool_call 信号，按文本处理)
LOCKED_TEXT
  -> COMPLETED / ERROR
LOCKED_TOOL_CALL
  -> TOOL_RUNNING -> COMPLETED / ERROR
```

状态规则：

1. `PROBING` 期间内容暂存，不立即发送到前端文本区。
2. 一旦 `LOCKED_TOOL_CALL`，禁止把 probing 缓冲内容以文本事件发送。
3. 一旦 `LOCKED_TEXT`，可立即 flush 缓冲并进入直通输出。

## 4. 判定策略（优先级从高到低）

1. Provider 原生结构化字段（强信号）
   - 如 tool_calls/function_call 事件或增量字段。
2. 适配器明确语义标签（中信号）
   - adapter 层将 provider 信号映射为统一 `tool_call_delta`。
3. 文本内容启发式（弱信号，默认关闭）
   - 仅用于兼容不支持结构化工具流的 provider。
   - 必须受 feature flag 保护。

## 5. 双分支执行器

### 5.1 Fast Path（默认）

1. 目标：最短链路输出文本。
2. 行为：
   - `response_started`
   - `response_text_delta*`
   - `response_completed`
3. 严禁在热路径做重解析、重拼接和高频日志。

### 5.2 Orchestrator Path（按需）

1. 目标：确保工具调用正确性和可解释性。
2. 行为：
   - 发 `tool_call_started`
   - 增量收集参数 `tool_args_delta`
   - 参数收齐后 `tool_call_ready`
   - 触发执行并发 `tool_call_progress/completed/failed`
   - 需要审批时由工具层发 `approval_requested/resolved`

## 6. 组件映射建议（OpenJax）

1. `openjax-core`
   - 新增 dispatcher 抽象模块，位于 model stream 与 planner/tool 之间。
   - 分发器作为默认且唯一入口，不再依赖兼容开关切换主路径。
2. `openjax-gateway`
   - 保持事件 envelope，不增加重复语义事件。
   - 继续支持 `after_event_seq` 回放恢复。
3. `ui/webui`
   - 仅订阅数据面文本事件进行高频渲染。
   - 控制面事件用于工具卡片和审批卡片展示，不触发全文本重绘。
