# Kimi Provider And Anthropic Thinking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 Kimi built-in provider 在 gateway 重启后回退旧默认值的问题，并修复 Anthropic Messages 多轮 tool-use 场景中丢失 assistant reasoning 历史导致的 Kimi 400。

**Architecture:** 本计划分成两个独立实现面。第一部分收敛 provider 真值边界，确保 built-in catalog 仅负责创建默认值，DB 才是运行时唯一真值。第二部分补齐 assistant reasoning 的历史消息表达，让 `planner` 能把 reasoning 写回历史，`anthropic_messages` 能在下一轮请求中忠实重放。

**Tech Stack:** Rust 2024, `openjax-core`, `openjax-gateway`, Axum, SQLite store, Anthropic Messages protocol tests, cargo test

---

### Task 1: 修复 Gateway 启动时覆写 Kimi Provider

**Files:**
- Modify: `openjax-gateway/src/state/config.rs`
- Modify: `openjax-gateway/src/state/events.rs`
- Modify: `openjax-gateway/src/handlers/provider.rs`
- Test: `openjax-gateway/src/state/config.rs`
- Test: `openjax-gateway/tests/gateway_api/m6_provider.rs`

- [ ] **Step 1: 写失败测试，锁定 Kimi built-in 不应在启动时回退**

在 `openjax-gateway/src/state/config.rs` 新增或改写单测，构造以下数据：

```rust
let provider = <SqliteStore as ProviderRepository>::create_provider(
    &store,
    "Kimi Coding",
    "https://api.kimi.com/coding",
    "k2.5",
    "key",
    "built_in",
    "anthropic_messages",
    200000,
)?;
```

验证 gateway 初始化相关逻辑运行后，provider 仍保持：

```rust
assert_eq!(updated.base_url, "https://api.kimi.com/coding");
assert_eq!(updated.model_name, "k2.5");
assert_eq!(updated.protocol, "anthropic_messages");
```

- [ ] **Step 2: 运行测试确认当前失败**

Run:
```bash
zsh -lc "cargo test -p openjax-gateway --lib state::config"
```

Expected:
- FAIL
- 失败原因表现为 provider 被改回旧值，或现有测试断言仍要求旧 Kimi 默认值

- [ ] **Step 3: 删除或收缩旧的 Kimi 归一化逻辑**

在 `openjax-gateway/src/state/config.rs`：
- 删除 `KIMI_BASE_URL = https://api.kimi.com/coding/v1`
- 删除 `KIMI_MODEL = kimi-for-coding`
- 删除或重写 `normalize_builtin_provider_defaults`

目标是让启动逻辑不再改写：
- `base_url`
- `model_name`
- `context_window_size`

- [ ] **Step 4: 移除启动链路中的覆写入口**

在 `openjax-gateway/src/state/events.rs`：
- 删除 `normalize_builtin_provider_defaults(&store);`
- 如果需要保留迁移入口，只能替换成不会覆写用户业务字段的空操作或安全迁移函数

- [ ] **Step 5: 同步更新 provider handler 的样例与断言**

在 `openjax-gateway/src/handlers/provider.rs`：
- 把测试样例中的 Kimi 值改成：

```rust
base_url: "https://api.kimi.com/coding".to_string(),
model_name: "k2.5".to_string(),
protocol: "anthropic_messages".to_string(),
```

- 删除所有把 `kimi-for-coding` 或 `/coding/v1` 视为当前 Kimi 默认值的断言

- [ ] **Step 6: 运行 gateway provider 相关测试**

Run:
```bash
zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m6_provider -- --nocapture"
```

Expected:
- PASS
- 新增的“不回退”测试通过

- [ ] **Step 7: 运行 gateway 完整测试集确认无回归**

Run:
```bash
zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite"
```

Expected:
- PASS

- [ ] **Step 8: Commit**

```bash
git add openjax-gateway/src/state/config.rs openjax-gateway/src/state/events.rs openjax-gateway/src/handlers/provider.rs openjax-gateway/tests/gateway_api/m6_provider.rs
git commit -m "fix(gateway): stop resetting kimi built-in provider defaults"
```

### Task 2: 为 Assistant 历史补齐 Reasoning Block 表达

**Files:**
- Modify: `openjax-core/src/model/types.rs`
- Test: `openjax-core/src/model/types.rs`

- [ ] **Step 1: 写失败测试，锁定 assistant content 需要支持 reasoning**

在 `openjax-core/src/model/types.rs` 为 `AssistantContentBlock` 增加目标测试，覆盖序列化与反序列化：

```rust
let msg = ConversationMessage::Assistant(vec![
    AssistantContentBlock::Reasoning {
        text: "need to inspect file".to_string(),
    },
    AssistantContentBlock::ToolUse {
        id: "tool_1".to_string(),
        name: "Read".to_string(),
        input: serde_json::json!({"file_path": "test.md"}),
    },
]);
```

要求 roundtrip 后结构不变。

- [ ] **Step 2: 运行测试确认当前失败**

Run:
```bash
zsh -lc "cargo test -p openjax-core model::types -- --nocapture"
```

Expected:
- FAIL
- 失败原因是 `AssistantContentBlock` 还没有 `Reasoning` 变体

- [ ] **Step 3: 扩展 assistant 历史消息结构**

在 `openjax-core/src/model/types.rs`：
- 为 `AssistantContentBlock` 添加 reasoning 变体，例如：

```rust
Reasoning { text: String }
```

- 让 `ConversationMessage::Assistant` 可以承载 reasoning、text、tool_use 的有序 block 列表
- 补齐最小 serde/unit tests

- [ ] **Step 4: 运行类型测试确认通过**

Run:
```bash
zsh -lc "cargo test -p openjax-core model::types -- --nocapture"
```

Expected:
- PASS

- [ ] **Step 5: Commit**

```bash
git add openjax-core/src/model/types.rs
git commit -m "feat(core): add assistant reasoning content blocks"
```

### Task 3: 修复 Planner 历史回填，保留 Reasoning

**Files:**
- Modify: `openjax-core/src/agent/planner.rs`
- Test: `openjax-core/src/tests/prompt_and_policy.rs`
- Possibly Modify: `openjax-core/src/tests/support.rs`

- [ ] **Step 1: 写失败测试，锁定 planner 会把 reasoning 写回 assistant 历史**

新增一个最小测试：
- 第一轮模型响应返回 `reasoning + tool_use`
- planner 执行完工具后发起下一轮
- 断言第二轮 `ModelRequest.messages` 中对应 assistant 历史包含 reasoning block

建议复用 `openjax-core/src/tests/support.rs` 的脚本化模型模式，只增加最小必要 mock。

- [ ] **Step 2: 运行测试确认当前失败**

Run:
```bash
zsh -lc "cargo test -p openjax-core planner -- --nocapture"
```

Expected:
- FAIL
- 失败原因是 planner 仍只写入 `response.content`

- [ ] **Step 3: 修改 planner，将 reasoning 与 content 一起写回历史**

在 `openjax-core/src/agent/planner.rs`：
- 用一个集中 helper 把 `ModelResponse` 转成 `ConversationMessage::Assistant`
- 规则：
  - 若 `response.reasoning` 存在，先写 reasoning block
  - 再追加 `response.content`

示意：

```rust
let assistant_blocks = response_to_assistant_blocks(&response);
messages.push(ConversationMessage::Assistant(assistant_blocks));
```

- [ ] **Step 4: 运行 planner 相关测试确认通过**

Run:
```bash
zsh -lc "cargo test -p openjax-core planner -- --nocapture"
```

Expected:
- PASS

- [ ] **Step 5: Commit**

```bash
git add openjax-core/src/agent/planner.rs openjax-core/src/tests/prompt_and_policy.rs openjax-core/src/tests/support.rs
git commit -m "fix(core): preserve assistant reasoning in planner history"
```

### Task 4: 修复 Anthropic Messages Builder，重放 Assistant Reasoning

**Files:**
- Modify: `openjax-core/src/model/anthropic_messages.rs`
- Test: `openjax-core/src/model/anthropic_messages.rs`

- [ ] **Step 1: 写失败测试，锁定 builder 会序列化 reasoning block**

在 `openjax-core/src/model/anthropic_messages.rs` 增加最小测试：
- 构造 `ModelRequest.messages`
- assistant 历史包含：

```rust
ConversationMessage::Assistant(vec![
    AssistantContentBlock::Reasoning { text: "inspect file".to_string() },
    AssistantContentBlock::ToolUse {
        id: "tool_1".to_string(),
        name: "Read".to_string(),
        input: json!({"file_path": "test.md"}),
    },
])
```

- 调用 request builder
- 断言输出的 assistant message content 中包含 reasoning/thinking block 与 tool_use block

- [ ] **Step 2: 运行测试确认当前失败**

Run:
```bash
zsh -lc "cargo test -p openjax-core anthropic_messages -- --nocapture"
```

Expected:
- FAIL
- 失败原因是 builder 还不认识 reasoning block

- [ ] **Step 3: 更新 Anthropic Messages 请求映射**

在 `openjax-core/src/model/anthropic_messages.rs`：
- 为 assistant 历史增加 reasoning block 到 Anthropic/Kimi thinking block 的映射
- 保持现有 `text` 和 `tool_use` 的行为不变
- 不改变 `build_messages_endpoint` 的 `/v1/messages` 逻辑

- [ ] **Step 4: 增加一个多轮 tool-use 回归测试**

新增或扩展测试，验证：
- 第一轮有 reasoning + tool_use
- 工具结果进入下一轮
- 第二轮 builder 输出的 assistant 历史仍包含 reasoning

- [ ] **Step 5: 运行 anthropic messages 相关测试**

Run:
```bash
zsh -lc "cargo test -p openjax-core anthropic_messages -- --nocapture"
```

Expected:
- PASS

- [ ] **Step 6: 运行 openjax-core 全量测试**

Run:
```bash
zsh -lc "cargo test -p openjax-core --tests"
```

Expected:
- PASS

- [ ] **Step 7: Commit**

```bash
git add openjax-core/src/model/anthropic_messages.rs
git commit -m "fix(core): replay assistant reasoning for anthropic tool turns"
```

### Task 5: 端到端联调验证

**Files:**
- No code changes required
- Verification target: WebUI + gateway runtime

- [ ] **Step 1: 启动本地开发环境**

Run:
```bash
zsh -lc "make run-web-dev"
```

Expected:
- gateway 与 web 启动成功
- 使用 `127.0.0.1` 访问，不使用 `localhost`

- [ ] **Step 2: 手动验证 provider 不回退**

在 WebUI 中：
1. 配置 Kimi API Key
2. 检查 provider 显示：
   - `https://api.kimi.com/coding`
   - `k2.5`
3. 重启 gateway
4. 再次进入设置页确认值未变化

- [ ] **Step 3: 手动验证 tool-use 第二轮不再 400**

在 WebUI 中提交一个会触发 `Read` 的请求，例如：

```text
你看下这个文件有什么内容 test.md
```

Expected:
- 第一轮 planner 发出 `Read`
- 工具执行成功
- 第二轮 planner 正常继续
- gateway 日志中不再出现：

```text
thinking is enabled but reasoning_content is missing
```

- [ ] **Step 4: 记录验证证据**

记录以下信息：
- 测试命令
- 测试结果
- WebUI 手动验证结果
- 相关日志摘录

- [ ] **Step 5: Commit 验证文档或保持工作树干净**

如果有专门验证记录文件则提交；如果没有新增文件，确认工作树只包含代码修复改动。

---

## Execution Notes

- 先做 Task 1，再做 Task 2-4，最后做 Task 5。
- 不要把 provider 回退修复和 reasoning 历史修复揉成一个提交；分开更容易验证与回滚。
- 如果 `openjax-core/src/model/types.rs` 或 `openjax-core/src/model/anthropic_messages.rs` 接近过大，请在实施时评估是否拆 helper，但不要在本计划之外做无关重构。
