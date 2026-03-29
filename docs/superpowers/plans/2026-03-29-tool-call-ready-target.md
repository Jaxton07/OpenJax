# ToolCallReady 携带 target 字段 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让流式路径的 ToolCallReady 事件携带 target 字段（文件路径或命令），前端通过 merge 更新 ToolStep 显示。

**Architecture:** 后端在 planner_stream_flow 中累积 ToolArgsDelta，在 ToolUseEnd 时解析完整 args 提取 target；非流式路径 tool_lifecycle 直接复用已有 target；前端在收到 tool_call_ready 事件后 merge 到已有 ToolStep 更新显示。

**Tech Stack:** Rust (openjax-protocol, openjax-core, openjax-gateway), TypeScript (React frontend)

---

### Task 1: Protocol 层 — ToolCallReady 增加 target 字段

**Files:**
- Modify: `openjax-protocol/src/lib.rs:117-123`

- [ ] **Step 1: 在 ToolCallReady 变体中增加 target 字段**

将 `openjax-protocol/src/lib.rs:117-123` 的 ToolCallReady 变体从：

```rust
ToolCallReady {
    turn_id: u64,
    tool_call_id: String,
    tool_name: String,
    #[serde(default)]
    display_name: Option<String>,
},
```

改为：

```rust
ToolCallReady {
    turn_id: u64,
    tool_call_id: String,
    tool_name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    target: Option<String>,
},
```

- [ ] **Step 2: 编译验证**

Run: `zsh -lc "cargo build -p openjax-protocol"`
Expected: 编译成功（可能显示下游 crate 警告，暂不影响）

- [ ] **Step 3: 修复所有下游编译错误**

运行 `zsh -lc "cargo build -p openjax-core 2>&1 | head -40"` 和 `zsh -lc "cargo build -p openjax-gateway 2>&1 | head -40"`，找到所有因新增 `target` 字段导致的 pattern 匹配错误。每个位置都需要补上 `target: None` 或对应值。

预期需要修改的位置：
1. `openjax-core/src/agent/planner_stream_flow.rs:110-115` — ToolUseEnd 分支的 ToolCallReady
2. `openjax-core/src/agent/tool_lifecycle.rs:43-50` — emit_tool_call_started_sequence 中的 ToolCallReady
3. `openjax-gateway/src/event_mapper/tool.rs:73-82` — map 函数中的 ToolCallReady 模式匹配

在 Task 2 和 Task 3 中会正确填充 target 值，这里先用 `target: None` 让编译通过。

- [ ] **Step 4: 确认 workspace 编译通过**

Run: `zsh -lc "cargo build -p openjax-core && cargo build -p openjax-gateway"`
Expected: 编译成功

---

### Task 2: Core — planner_stream_flow 累积 args 并提取 target

**Files:**
- Modify: `openjax-core/src/agent/planner_stream_flow.rs:44-117`

- [ ] **Step 1: 在 process_stream_deltas 闭包中新增 args_accum**

在 `openjax-core/src/agent/planner_stream_flow.rs` 中，`request_planner_model_output` 函数体内已有 `let mut tool_names: HashMap<String, String> = HashMap::new();`（第 51 行）。在其后新增一行：

```rust
let mut tool_names: HashMap<String, String> = HashMap::new();
let mut args_accum: HashMap<String, String> = HashMap::new();  // 新增
```

- [ ] **Step 2: ToolArgsDelta 分支追加 delta 到 args_accum**

将第 91-103 行的 `StreamDelta::ToolArgsDelta { id, delta }` 分支改为：

```rust
StreamDelta::ToolArgsDelta { id, delta } => {
    args_accum.entry(id.clone()).or_default().push_str(&delta);
    let tool_name = tool_names.get(&id).cloned().unwrap_or_default();
    let display_name = self.tools.display_name_for(&tool_name);
    self.push_event(
        events,
        Event::ToolCallArgsDelta {
            turn_id,
            tool_call_id: id,
            tool_name,
            args_delta: delta,
            display_name,
        },
    );
}
```

- [ ] **Step 3: ToolUseEnd 分支提取 target 并传入 ToolCallReady**

将第 105-117 行的 `StreamDelta::ToolUseEnd { id }` 分支改为：

```rust
StreamDelta::ToolUseEnd { id } => {
    let tool_name = tool_names.get(&id).cloned().unwrap_or_default();
    let display_name = self.tools.display_name_for(&tool_name);
    let target = args_accum
        .remove(&id)
        .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok())
        .and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect::<HashMap<String, String>>()
            })
        })
        .as_ref()
        .and_then(|args| crate::agent::planner_utils::extract_tool_target_hint(&tool_name, args));
    self.push_event(
        events,
        Event::ToolCallReady {
            turn_id,
            tool_call_id: id,
            tool_name,
            display_name,
            target,
        },
    );
}
```

- [ ] **Step 4: 编译验证**

Run: `zsh -lc "cargo build -p openjax-core"`
Expected: 编译成功

---

### Task 3: Core — tool_lifecycle 的 ToolCallReady 带 target

**Files:**
- Modify: `openjax-core/src/agent/tool_lifecycle.rs:43-50`

- [ ] **Step 1: 将已有 target 变量传入 ToolCallReady**

在 `openjax-core/src/agent/tool_lifecycle.rs` 中，`emit_tool_call_started_sequence` 函数第 27 行已有 `target: extract_tool_target_hint(tool_name, args)`。需要把这个 target 保存到变量并复用。

将第 20-50 行从：

```rust
let display_name = self.tools.display_name_for(tool_name);
self.push_event(
    events,
    Event::ToolCallStarted {
        turn_id,
        tool_call_id: tool_call_id.to_string(),
        tool_name: tool_name.to_string(),
        target: extract_tool_target_hint(tool_name, args),
        display_name: display_name.clone(),
    },
);
if let Some(args_delta) = tool_args_delta_payload(args) {
    self.push_event(
        events,
        Event::ToolCallArgsDelta {
            turn_id,
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            args_delta,
            display_name: display_name.clone(),
        },
    );
}
self.push_event(
    events,
    Event::ToolCallReady {
        turn_id,
        tool_call_id: tool_call_id.to_string(),
        tool_name: tool_name.to_string(),
        display_name: display_name.clone(),
    },
);
```

改为：

```rust
let display_name = self.tools.display_name_for(tool_name);
let target = extract_tool_target_hint(tool_name, args);
self.push_event(
    events,
    Event::ToolCallStarted {
        turn_id,
        tool_call_id: tool_call_id.to_string(),
        tool_name: tool_name.to_string(),
        target: target.clone(),
        display_name: display_name.clone(),
    },
);
if let Some(args_delta) = tool_args_delta_payload(args) {
    self.push_event(
        events,
        Event::ToolCallArgsDelta {
            turn_id,
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            args_delta,
            display_name: display_name.clone(),
        },
    );
}
self.push_event(
    events,
    Event::ToolCallReady {
        turn_id,
        tool_call_id: tool_call_id.to_string(),
        tool_name: tool_name.to_string(),
        display_name: display_name.clone(),
        target,
    },
);
```

- [ ] **Step 2: 编译验证**

Run: `zsh -lc "cargo build -p openjax-core"`
Expected: 编译成功

---

### Task 4: Gateway — 映射 ToolCallReady 的 target 字段

**Files:**
- Modify: `openjax-gateway/src/event_mapper/tool.rs:73-83`

- [ ] **Step 1: 更新 ToolCallReady 的模式匹配和 payload**

将第 73-83 行从：

```rust
Event::ToolCallReady {
    turn_id,
    tool_call_id,
    tool_name,
    display_name,
} => Some(CoreEventMapping {
    core_turn_id: Some(*turn_id),
    event_type: "tool_call_ready",
    payload: json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "display_name": display_name }),
    stream_source: None,
}),
```

改为：

```rust
Event::ToolCallReady {
    turn_id,
    tool_call_id,
    tool_name,
    display_name,
    target,
} => Some(CoreEventMapping {
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

- [ ] **Step 2: 编译验证**

Run: `zsh -lc "cargo build -p openjax-gateway"`
Expected: 编译成功

---

### Task 5: 后端全量验证

- [ ] **Step 1: 运行 core 测试**

Run: `zsh -lc "cargo test -p openjax-core 2>&1 | tail -20"`
Expected: 全部测试通过

- [ ] **Step 2: 运行 gateway 测试**

Run: `zsh -lc "cargo test -p openjax-gateway 2>&1 | tail -20"`
Expected: 全部测试通过

- [ ] **Step 3: 提交后端改动**

```bash
git add openjax-protocol/src/lib.rs openjax-core/src/agent/planner_stream_flow.rs openjax-core/src/agent/tool_lifecycle.rs openjax-gateway/src/event_mapper/tool.rs
git commit -m "feat(core,gateway): ToolCallReady 事件携带 target 字段

流式路径累积 ToolArgsDelta，在 ToolUseEnd 时解析 args 提取 target；
非流式路径复用已有 extract_tool_target_hint 结果。"
```

---

### Task 6: Frontend — 类型定义与事件处理

**Files:**
- Modify: `ui/web/src/types/gateway.ts:239-260`
- Modify: `ui/web/src/lib/session-events/tools.ts:9-17,135-150`

- [ ] **Step 1: StreamEvent type 增加 tool_call_ready**

在 `ui/web/src/types/gateway.ts` 的 `StreamEvent` 接口中，`type` 联合类型的 `"tool_call_started"` 之后新增 `"tool_call_ready"`：

将第 251 行：
```
    | "tool_call_started"
```
改为：
```
    | "tool_call_started"
    | "tool_call_ready"
```

- [ ] **Step 2: isToolStepEvent 增加 tool_call_ready 判断**

在 `ui/web/src/lib/session-events/tools.ts` 中，将 `isToolStepEvent` 函数（第 9-17 行）改为：

```typescript
export function isToolStepEvent(event: StreamEvent): boolean {
  return (
    event.type === "tool_call_started" ||
    event.type === "tool_call_ready" ||
    event.type === "tool_call_completed" ||
    event.type === "approval_requested" ||
    event.type === "approval_resolved" ||
    event.type === "error"
  );
}
```

- [ ] **Step 3: createStepFromEvent 增加 tool_call_ready 分支**

在 `ui/web/src/lib/session-events/tools.ts` 的 `createStepFromEvent` 函数中，在 `tool_call_started` 分支（第 136-149 行）之后、`tool_call_completed` 分支（第 152 行）之前插入新分支：

```typescript
if (event.type === "tool_call_ready") {
    const payload = event.payload as { tool_call_id?: string; target?: string };
    const target = payload.target ?? "";
    return {
        id: toolCallIdFromPayload(event) || `tool_call_ready:${event.turn_id ?? "unknown"}:${event.event_seq}`,
        type: "tool" as const,
        target,
        toolCallId: toolCallIdFromPayload(event),
    } as ToolStep;
}
```

注意：这里创建的是一个轻量 ToolStep，只含 id、type、target、toolCallId。mergeToolStep 中 `{...previous, ...next}` 展开只会用 next 中存在的 key 覆盖 previous，不会丢失 previous 的 title、status 等字段。

- [ ] **Step 4: 前端构建验证**

Run: `zsh -lc "cd ui/web && pnpm build"`
Expected: 构建成功

- [ ] **Step 5: 提交前端改动**

```bash
git add ui/web/src/types/gateway.ts ui/web/src/lib/session-events/tools.ts
git commit -m "feat(ui): 前端处理 tool_call_ready 事件更新 ToolStep target"
```

---

### Task 7: 全量集成验证

- [ ] **Step 1: Clippy 检查**

Run: `zsh -lc "cargo clippy -p openjax-core -p openjax-gateway -- -D warnings 2>&1 | tail -20"`
Expected: 无 warning

- [ ] **Step 2: 全量测试**

Run: `zsh -lc "cargo test -p openjax-core -p openjax-gateway 2>&1 | tail -30"`
Expected: 全部通过
