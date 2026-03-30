# OpenJax Transcript-First Gateway 架构设计（P0 + P1）

## 1. 背景与目标

本设计用于收口 OpenJax 当前 gateway/core 在事件持久化与边界职责上的问题，目标是为后续协议升级做准备，但本阶段不做协议字段升级。

已确认约束：

1. 不做兼容性补丁或兜底降级路径。
2. 不改变 public API 语义（接口功能语义保持不变）。
3. 不引入 `run_id`、`step_id` 等新协议字段。
4. 一次性切换到 `Session Transcript First`。
5. 存储采用本地文件 JSONL 优先，当前阶段不落 SQLite。
6. 保留策略采用 30 天清理。

## 2. 问题陈述（当前实现）

1. 存在“持久化失败仍继续发布”的路径，破坏一致性。
2. core 新事件可能在 gateway 通过 `Option` 映射链被静默丢弃。
3. gateway/core 存在边界混杂和超大文件问题。
4. README 与实现结构存在漂移。
5. `planner_tool_batch` 已形成 dead path，但仍保留在主模块结构中。

## 3. 设计原则

1. **语义单一来源**：`openjax-core` 是事件语义唯一来源（`event_type` + `payload`）。
2. **gateway 薄层化**：`openjax-gateway` 只承担 transport/runtime adapter，不再改写事件语义。
3. **先落盘后发布**：事件仅在 transcript append 成功后允许广播到 SSE。
4. **显式失败优先**：未映射事件与持久化失败必须显式暴露，不能静默吞掉。
5. **模块化收口**：拆分超大文件，按职责解耦，降低后续维护成本。

## 4. 目标架构

### 4.1 总体形态

- 保留 gateway（薄化），不改为客户端直连 core。
- 采用 `TranscriptStore` 作为 gateway 事件持久化唯一入口。
- timeline/replay 数据源统一来自 transcript 文件，不再依赖 SQLite `biz_events/biz_messages`。

### 4.2 模块边界

#### openjax-core

- 输出标准事件语义（type + payload）。
- 不感知 HTTP/SSE、segment、manifest 等网关持久化细节。

#### openjax-gateway

- 负责认证、会话生命周期、SSE 推送、重放窗口、请求上下文。
- 不再做 core 事件二次语义改写，不再做 event alias。
- 封装 `TranscriptStore` 并提供 append/replay/gc 能力。

#### openjax-store

- 当前阶段从 gateway 主链路移除（不作为 timeline/event 主存储）。
- 可保留为未来可选后端，不参与本次 P0/P1 验收路径。

## 5. Transcript 存储设计（File First）

### 5.1 目录结构

每个 session 独立目录，按 segment 管理：

```text
<transcript_root>/
  sessions/
    <session_id>/
      manifest.json
      segments/
        segment-000001.jsonl
        segment-000002.jsonl
        ...
```

### 5.2 记录格式（JSONL）

每行一个 `TranscriptRecord`：

```json
{
  "schema_version": 1,
  "session_id": "sess_xxx",
  "event_seq": 128,
  "turn_seq": 5,
  "turn_id": "turn_5",
  "event_type": "tool_call_completed",
  "stream_source": "model_live",
  "timestamp": "2026-03-30T10:00:00Z",
  "payload": {},
  "request_id": "req_xxx"
}
```

字段规则：

1. `payload` 透传 core，不做 gateway 语义改写。
2. `event_type` 沿用 core 命名，不做 gateway 别名映射。
3. `event_seq` 为单 session 严格递增，且只有 append 成功后可见。
4. `schema_version` 固定写入，为后续升级预留演进位。

### 5.3 manifest 最小结构

```json
{
  "schema_version": 1,
  "session_id": "sess_xxx",
  "last_event_seq": 128,
  "last_turn_seq": 5,
  "active_segment": "segment-000003.jsonl",
  "updated_at": "2026-03-30T10:00:00Z"
}
```

### 5.4 segment 轮转与清理

1. segment 达到阈值（例如 16MB）后切新段。
2. 启动恢复或写入时若 active segment 不可写，切新段并记录告警。
3. 保留策略固定 30 天，按 session/segment 时间戳清理；session 空目录可回收。

## 6. 关键数据流与失败语义

### 6.1 正常路径

`core event -> gateway envelope -> TranscriptStore.append -> append成功 -> SSE publish`

说明：

1. gateway 仅补充 envelope 元字段（如 `session_id/event_seq/timestamp/request_id`）。
2. timeline/replay 统一通过 transcript 读取，不再读取 SQLite 事件表。

### 6.2 失败路径

1. append 失败：当前事件禁止发布到 SSE。
2. 关键事件（`turn/response/tool/approval`）append 失败：当前 turn 转 failed，并尝试写入固定 `response_error` 事件（错误码 `TRANSCRIPT_APPEND_FAILED`）。
3. 若该 `response_error` 事件 append 仍失败：停止当前 turn，返回固定失败响应，内部记录告警；不再递归尝试写入后续错误事件。
4. 未映射 core 事件：显式告警 + 测试门禁失败，不允许静默忽略。

### 6.3 “映射门禁”精确定义

1. 本设计中的“映射”仅指 **core 事件类型 -> gateway envelope 投影处理函数** 的类型覆盖关系，不指 payload 字段改写（payload 不改写）。
2. gateway 需维护一份“已支持 core 事件类型集合”并与 `openjax_protocol::Event` 变体集合做 1:1 覆盖校验（测试期强制）。
3. 任何新增 core 事件变体，若未进入该集合并实现 envelope 投影，CI 必须失败。

## 7. P1 边界收口拆分方案

### 7.1 gateway 文件拆分

将 `openjax-gateway/src/state/events.rs` 拆为：

1. `turn_orchestrator.rs`：turn 执行编排。
2. `core_projection.rs`：core -> gateway envelope 投影。
3. `publish_pipeline.rs`：append 成功后发布的原子路径。

### 7.2 core 文件拆分

将 `openjax-core/src/agent/planner_tool_action.rs` 拆为：

1. `tool_guard.rs`：guard/policy 判定。
2. `tool_executor.rs`：工具执行。
3. `tool_projection.rs`：事件收敛与结果投影。

### 7.3 dead code 收口

`planner_tool_batch` 走一次性删除策略：

1. 删除 `planner_tool_batch.rs` dead path。
2. 同步更新 `openjax-core/src/agent/mod.rs`。
3. 同步更新 `openjax-core/src/agent/README.md` 与相关引用。

## 8. 测试迁移与门禁

本次重构必须同步测试迁移，不允许“实现切换但测试仍绑旧后端”。

### 8.1 新增/改造测试类型

1. 原子发布测试：
   - append 失败时 SSE 不发布。
   - 关键事件 append 失败时 turn 失败语义正确。
2. 映射门禁测试：
   - core 新事件未映射时测试 fail。
3. 一致性测试：
   - 同一 turn 下 SSE 与 timeline 的 `event_seq/type/payload` 一致。
   - 覆盖 `Last-Event-ID` 断线重连场景。
4. 存储后端切换测试：
   - 删除或改造依赖 SQLite `biz_events/biz_messages` 的断言。
   - 改为断言 segment + manifest + replay 结果。

### 8.2 最低验收命令

1. `zsh -lc "make gateway-fast"`
2. `zsh -lc "make core-feature-streaming"`
3. `zsh -lc "make core-feature-tools"`

### 8.3 恢复一致性校验

1. 进程重启后，`manifest.last_event_seq` 必须与 active segment 尾记录 `event_seq` 一致。
2. 若不一致，恢复逻辑需执行修复（以 segment 尾记录为准回写 manifest）并输出告警。

## 9. 文档同步要求

README 必须与实现一致：

1. `openjax-gateway/README.md` 删除过时 `src/persistence/*` 结构描述，替换为 transcript-first 结构。
2. `openjax-core/src/agent/README.md` 同步移除 `planner_tool_batch` dead path 描述，更新新拆分模块职责。
3. 所有“timeline 来源”描述统一改为 transcript replay。

## 10. 非目标（本阶段不做）

1. 不做协议字段升级（`run_id/step_id` 等）。
2. 不做兼容性补丁路径（如旧事件双写、旧映射静默 fallback）。
3. 不做 gateway public API 语义变更。
4. 不引入 SQLite 与 JSONL 双写。

## 11. 风险与控制

1. **一次性切换风险**：需保证 replay/timeline 全链路测试先到位再替换主路径。
2. **文件一致性风险**：append 与 manifest 更新需定义明确顺序与恢复规则，避免 seq 回退或跳号。
3. **超大会话风险**：通过 segment 轮转与 30 天 GC 控制。
4. **边界回退风险**：通过 mapper 门禁测试防止新增事件再次被静默忽略。

## 12. 里程碑（仅 P0 + P1）

1. M1（P0）：
   - 引入 TranscriptStore + append-then-publish 主链路。
   - 建立映射门禁与一致性测试。
2. M2（P1）：
   - 完成 gateway/core 文件拆分。
   - 删除 planner_tool_batch dead path。
   - README 与实现对齐。
