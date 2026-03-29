# ToolCallReady 携带 target 字段

## 背景

WebUI 对话区域的 tool call 卡片需显示目标文件路径（Read/Edit 等工具）或执行的命令（shell 工具）。前端已实现 `step.target` 的渲染逻辑（`ToolStepCard.tsx`、`.step-target` CSS），但流式路径中 `ToolCallStarted` 发出时 args 尚未到达，`target` 为 `None`。

根因：`planner_stream_flow.rs:86` 硬编码 `target: None`。非流式路径 `tool_lifecycle.rs` 拥有完整 args，target 正确。

## 方案

利用已有的 `ToolCallReady` 事件（args 全部收齐后、工具执行前触发）携带 target。前端收到后 merge 到已有 ToolStep 中更新显示。

## 改动清单

### 1. Protocol — `openjax-protocol/src/lib.rs`

`ToolCallReady` 变体增加 `target: Option<String>` 字段，补充 `#[serde(default)]` 保持向后兼容。

```rust
ToolCallReady {
    turn_id: u64,
    tool_call_id: String,
    tool_name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    target: Option<String>,  // 新增
},
```

### 2. Core — `openjax-core/src/agent/planner_stream_flow.rs`

在 `process_stream_deltas` 函数中：

- 新增 `args_accum: HashMap<String, String>`，按 `tool_call_id` 累积 args delta 片段
- `ToolArgsDelta` 分支：追加 delta 到 `args_accum[id]`
- `ToolUseEnd` 分支：解析 `args_accum[id]` 为 JSON（使用 `serde_json::Value`，仅提取字符串字段构建 `HashMap<String, String>`），调用 `extract_tool_target_hint` 提取 target，传入 `ToolCallReady` 事件

ToolUseEnd 处理伪代码：

```rust
StreamDelta::ToolUseEnd { id } => {
    let tool_name = tool_names.get(&id).cloned().unwrap_or_default();
    let display_name = self.tools.display_name_for(&tool_name);
    let target = args_accum.remove(&id)
        .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok())
        .and_then(|v| v.as_object().map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect::<HashMap<String, String>>()
        }))
        .as_ref()
        .and_then(|args| extract_tool_target_hint(&tool_name, args));
    self.push_event(events, Event::ToolCallReady {
        turn_id,
        tool_call_id: id,
        tool_name,
        display_name,
        target,
    });
}
```

> **注意：** 解析为 `serde_json::Value` 再提取字符串字段，而非直接 `from_str::<HashMap<String, String>>`。因为工具参数可能包含非字符串值（如 `offset: 10`），直接反序列化为 `HashMap<String, String>` 会失败。target 提取只关心字符串字段（`file_path`、`cmd` 等），非字符串字段跳过不影响结果。

### 3. Core — `openjax-core/src/agent/tool_lifecycle.rs`

`emit_tool_call_started_sequence` 中 `ToolCallReady` 也带上 target（复用已提取的 `extract_tool_target_hint` 结果）。

该函数已有 `target` 变量（用于 `ToolCallStarted`），直接传入 `ToolCallReady`：

```rust
self.push_event(events, Event::ToolCallReady {
    turn_id,
    tool_call_id: tool_call_id.to_string(),
    tool_name: tool_name.to_string(),
    display_name: display_name.clone(),
    target,  // 复用已有变量
});
```

### 4. Gateway — `openjax-gateway/src/event_mapper/tool.rs`

`ToolCallReady` 映射增加 `"target": target` 字段。

```rust
Event::ToolCallReady { turn_id, tool_call_id, tool_name, display_name, target } => Some(CoreEventMapping {
    core_turn_id: Some(*turn_id),
    event_type: "tool_call_ready",
    payload: json!({
        "tool_call_id": tool_call_id,
        "tool_name": tool_name,
        "display_name": display_name,
        "target": target,
    }),
    stream_source: None,
}),
```

### 5. Frontend — 类型定义 `ui/web/src/types/gateway.ts`

新增 `ToolCallReadyPayload` 接口：

```typescript
export interface ToolCallReadyPayload extends Record<string, unknown> {
  tool_call_id?: string;
  tool_name?: string;
  display_name?: string;
  target?: string;
}
```

### 6. Frontend — 事件处理 `ui/web/src/lib/session-events/tools.ts`

- `isToolStepEvent` 增加 `"tool_call_ready"` 判断
- `createStepFromEvent` 新增 `tool_call_ready` 分支：创建轻量 ToolStep（仅含 id + target）
- `mergeToolStep` 中 tool+tool 合并路径：`{ ...previous, ...next }` 天然保证 next 中的 target 覆盖 previous 中的空值，无需额外改动

### 7. Frontend — CSS

`.step-target` 已有 `text-overflow: ellipsis; white-space: nowrap; overflow: hidden;`，无需额外改动。

## 复用的已有函数

- `extract_tool_target_hint(tool_name, args)` — `openjax-core/src/agent/planner_utils.rs`
- `tool_names` HashMap（已有，存 `tool_call_id → tool_name` 映射）— `planner_stream_flow.rs`

## 验证

1. `cargo build -p openjax-core && cargo build -p openjax-gateway` 编译通过
2. `cargo test -p openjax-core` 全部测试通过
3. `cd ui/web && pnpm build` 前端构建通过
4. `make run-web-dev` 启动后，在 WebUI 中发送包含工具调用的请求，确认：
   - tool call 卡片在"运行中"状态即显示 target（文件路径或命令）
   - 命令过长时以省略号截断
   - 非 shell 工具（如 Read、Edit）显示文件绝对路径
