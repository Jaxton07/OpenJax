# Context Compression Design

**Date**: 2026-03-19
**Status**: Approved
**Branch**: feat/context-compression (pending)

## Background

OpenJax 的会话历史已通过 history 重构（commit a645e9ca）升级为轮次结构：`history: Vec<HistoryItem>`，其中 `HistoryItem` 有两个 variant：`Turn(TurnRecord)` 和 `Summary(String)`。截断配额为 `MAX_CONVERSATION_HISTORY_TURNS = 100`（硬上限兜底），只计 `Turn` variant，`Summary` 不占配额。

对于长任务场景，当前设计仍会导致：

1. 重要的早期上下文（用户目标、已完成步骤、关键决策）被静默丢弃
2. 模型失去对任务全貌的理解，可能重复工作或做出矛盾决策
3. 无法感知 token 用量，不知道何时该压缩

本设计在现有轮次结构的基础上，叠加一套 token 感知的上下文压缩机制，利用 `HistoryItem::Summary` variant 存储压缩摘要。

## Goals

- 自动压缩：基于模型 context window 用量占比，在达到阈值前自动触发压缩
- 手动压缩：前端可通过 `/compact` 接口随时触发
- 保留关键信息：压缩后的摘要包含用户目标、关键决策和执行链路概要
- 不破坏现有功能：压缩是 history 内部操作，不影响事件流、审批、工具调用等

## Non-Goals

- 不实现多级摘要（压缩后再压缩的递归场景留待后续）
- 不持久化摘要到数据库（摘要仅存在于内存 session，会话重建不恢复）；当前系统连普通历史都不持久化，单独为压缩摘要开口会产生不一致。完整的历史持久化（含会话快照）是独立的后续工作
- `MAX_CONVERSATION_HISTORY_TURNS = 100` 为硬上限兜底，正常流程由压缩接管，硬截断几乎不触发；`Summary` 不占配额，压缩不会导致摘要被逐出

## Design

### 1. 摘要格式（混合式）

压缩时，旧的 `Turn` 条目被替换为一个 `HistoryItem::Summary(String)`，摘要文本格式如下：

```
[CONTEXT SUMMARY - covers turns 1~N, 2026-03-19 14:30]

**Objective**: <用户期望达成的目标>

**Key Decisions**:
- <决策1>
- <决策2>

**Execution Steps**:
1. tool_name(args_summary) → result_summary ✓/✗
2. tool_name(args_summary) → result_summary ✓/✗

**Current State**: <当前所处阶段，已完成什么，待完成什么>
```

说明：
- `Key Decisions` 为可选区块，仅当历史中有明确技术决策时出现
- `Execution Steps` 每条格式：`工具名(核心参数摘要) → 结果状态`，末尾用 ✓/✗ 标记成功与否
- 整体控制在 400 token 以内（约 1600 字符），避免摘要本身过大

### 2. 流式 Usage 采集修复

**问题**：`chat_completions.rs` 的 `complete_stream` 方法在流式路径下返回 `usage: None`，尽管 GLM 等 provider 在最后一个 SSE chunk 中携带了 usage 数据。

**修复**：
- 在流式 frame 解析循环中，遇到含 `usage` 字段的 frame 时提取并缓存
- 在 `ChatCompletionRequest` 中新增 `stream_options` 字段，streaming 时设为 `{"include_usage": true}`（OpenAI 官方需要此参数）
- `complete_stream` 最终返回的 `ModelResponse.usage` 填入采集到的值；若 provider 不返回 usage，则 fallback 到字符数估算（`total_history_chars / 3.5`）

### 3. Agent 新增字段

在 `openjax-core/src/lib.rs` 的 `Agent` struct 中新增：

```rust
context_window_size: u32,      // 从 active provider DB 记录注入，默认 32768
last_input_tokens: Option<u64>, // 上次请求的实际 prompt tokens（来自 usage）
```

`bootstrap.rs` 在构建 Agent 时，从 active provider config 的 `context_window_size` 字段注入。

### 4. 自动压缩触发逻辑

位置：`planner.rs` 每轮 model response 处理后

```
触发条件：
  actual = last_input_tokens（若有）
  estimated = sum(history entry chars) / 3.5（fallback）
  ratio = actual_or_estimated / context_window_size
  if ratio >= 0.75 → 触发压缩
```

阈值 0.75 为默认值，预留 25% 空间给当前轮的 output tokens 和 reasoning tokens。

压缩完成后继续当前回合的 planner 循环（不中断任务）。

### 5. 新模块：context_compressor.rs

路径：`openjax-core/src/agent/context_compressor.rs`

职责：
- 接收当前 `history: &[HistoryItem]` 和 `model_client`
- 将 history 分为两部分，**以 `Turn` variant 计数为边界**：
  - **recent**：从 history 末尾往前数，保留最后 3 个 `Turn` variant（及其间穿插的任何 `Summary`）
  - **to_summarize**：3rd-from-last `Turn` 之前的所有条目（包含旧的 `Summary`）
- 若 `to_summarize` 中 `Turn` 数量为 0，或整个 history 中 `Turn` 总数 <= 4，直接返回原 history（不压缩）
- 构造 compression prompt（序列化 `to_summarize` 部分：`Turn` 展开为 user/tool/assistant，旧 `Summary` 直接插入），调用 model（非流式 `complete`）生成混合格式摘要
- **多次压缩策略**：采用**合并策略**——新摘要替代 `to_summarize` 中所有旧 `Summary` 条目，最终 history 中始终只有 0 或 1 个 `Summary`，避免链式堆积
- **失败降级**：若模型调用失败，记录 `warn!` 日志并返回原 history（不中断任务，跳过本次压缩）
- 返回：`[HistoryItem::Summary(text)] + recent_items`

compression prompt 示例：

```
You are a context compressor. Given the following conversation history,
produce a concise summary in this exact format:

[CONTEXT SUMMARY - covers turns 1~N, <timestamp>]

**Objective**: <one sentence>

**Key Decisions**:  (omit this section if none)
- <decision>

**Execution Steps**:
1. tool(args_summary) → result ✓/✗

**Current State**: <one sentence>

Keep the summary under 400 tokens. Be factual, preserve tool names and outcomes.

--- HISTORY TO SUMMARIZE ---
<history entries>
```

### 6. 协议层新增事件

在 `openjax-protocol` 中新增 `ContextCompacted` 事件：

```rust
ContextCompacted {
    compressed_turns: u32,   // 被压缩的历史条数
    retained_turns: u32,     // 保留的最近条数
    summary_preview: String, // 摘要前 120 字，供前端 Toast 展示
}
```

### 7. Gateway /compact 实现

`handlers.rs` 中，当 action 为 `compact` 或 input 为 `/compact` 时：
- 调用 `agent.compact()` 方法（新增到 Agent 上的公开方法，内部调用 context_compressor）
- 等待压缩完成
- 向 SSE 推送 `ContextCompacted` 事件
- 返回 turn completed 响应（与 `/clear` 类似的流程）

### 8. 数据流全链路

```
用户发送消息
    → planner 调用 model
    → model response (含 usage)
    → 更新 last_input_tokens
    → check_and_auto_compact()
        → ratio >= 0.75?
            → YES: context_compressor.compact(history, model_client)
                   → emit ContextCompacted event
                   → replace history
            → NO: continue
    → 下一轮 planner 循环

手动压缩
    → POST /api/v1/sessions/:id/turns { input: "/compact" }
    → handlers.rs 调用 agent.compact()
    → emit ContextCompacted event
    → return turn_completed
```

## Files Changed

| 文件 | 类型 | 说明 |
|------|------|------|
| `openjax-core/src/model/chat_completions.rs` | 修改 | 修复流式 usage 采集；新增 stream_options |
| `openjax-core/src/lib.rs` | 修改 | Agent struct 新增 context_window_size / last_input_tokens |
| `openjax-core/src/agent/bootstrap.rs` | 修改 | 注入 context_window_size |
| `openjax-core/src/agent/context_compressor.rs` | 新建 | 压缩逻辑主体 |
| `openjax-core/src/agent/mod.rs` | 修改 | 新增 `pub(crate) mod context_compressor` 声明 |
| `openjax-core/src/agent/planner.rs` | 修改 | 每轮 usage 更新 + 触发压缩检查 |
| `openjax-protocol/src/` | 修改 | 新增 ContextCompacted 事件 |
| `openjax-gateway/src/handlers.rs` | 修改 | 实现 /compact action |
| `openjax-gateway/src/event_mapper/` | 修改 | 映射 ContextCompacted 事件 |

## Testing

- 单元测试（`#[cfg(test)]`）：
  - `context_compressor`: 测试分割逻辑（Turn 总数 <= 4 时不压缩、正常分割、多次压缩合并旧 Summary）
  - `chat_completions`: 测试流式 usage 从最后一帧提取
- 集成测试（`tests/` 下新建 `m10_context_compression.rs`）：
  - 验证 agent history 达到阈值后自动触发压缩
  - 验证 /compact 手动触发后 ContextCompacted 事件被正确推送
  - 验证压缩后 history 结构符合预期（1 Summary + 最近 3 Turn）

## Open Questions

- `context_window_size` 为 0 时（用户自定义 provider 未填写）：跳过 token 感知压缩，仅保留条数截断
- Anthropic provider 的流式 usage 格式与 OpenAI 不同（`input_tokens` 字段名），`anthropic_messages.rs` 需单独验证，但不在本次实现范围内
