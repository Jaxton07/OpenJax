# Kimi Provider And Anthropic Thinking Design

**Goal:** 修复两个独立但相邻的运行时问题：一是 `openjax-gateway` 重启后会把 WebUI 中已保存的 Kimi built-in provider 回退到旧默认值；二是 `openjax-core` 在 `anthropic_messages` 多轮 tool-use 请求中丢失 assistant reasoning/thinking 历史，导致 Kimi 返回 400。

**Scope:** 仅覆盖以下两项修复：
- Kimi built-in provider 的持久化与启动行为
- Anthropic Messages / Kimi 的 reasoning 历史闭环

**Out of Scope:**
- `request_profile` 的 provider 持久化与 API 扩展
- 通过关闭 thinking、降低能力或添加兼容性分支来规避问题
- 任何与本次问题无关的 provider/catalog 重构

---

## 背景

当前仓库里，Kimi 的默认配置已经在 built-in catalog 中更新为：

- `base_url = https://api.kimi.com/coding`
- `protocol = anthropic_messages`
- `default_model = k2.5`

但 `openjax-gateway` 启动时仍存在一套旧的 Kimi 归一化逻辑，会把数据库中的 built-in Kimi provider 强制改回：

- `base_url = https://api.kimi.com/coding/v1`
- `model_name = kimi-for-coding`

这会导致用户在 WebUI 中配置好 Kimi 后，重启 gateway 又被回退。

另一个问题出现在 `anthropic_messages` 多轮 tool-use 流程中。当前 `ModelResponse` 虽然能承载 `reasoning`，但 planner 没有把它写回 assistant 历史消息，导致第二轮请求只有 assistant `tool_use`，没有对应的 reasoning/thinking 历史。Kimi 在开启 thinking 的情况下会校验这条历史消息，并返回：

```text
thinking is enabled but reasoning_content is missing in assistant tool call message
```

这两个问题的共同点是“运行时真值不一致”：

- provider 配置的真值在 catalog 和 DB 之间不一致
- assistant 历史消息的真值在 `ModelResponse` 和 `ConversationMessage` 之间不一致

本设计的目标是收敛这两处真值边界。

## 设计原则

### 1. 单一真值

- built-in catalog 只负责创建时的默认值
- provider 一旦落库，DB 就是运行时唯一真值
- assistant 一旦返回 reasoning，该 reasoning 必须进入会话历史，不能只停留在临时响应对象里

### 2. 不做补丁式规避

不采用以下方案：

- 在 Kimi 后续轮次关闭 thinking
- 把 reasoning 混入 text 或 raw 字段伪装传递
- 启动时继续“修正”用户已经保存的 provider 业务字段

这些方案都只是在掩盖根因，不符合本仓库当前的约束。

### 3. 保持模块边界清晰

- `openjax-core/src/builtin_catalog.rs` 负责 built-in 目录
- `openjax-gateway` 负责 provider CRUD、持久化与会话运行时组装
- `openjax-core` 的消息类型负责表达完整的 assistant 历史语义
- `anthropic_messages` 负责把完整历史消息序列化为 Kimi/Anthropic 可接受的 payload

---

## 问题一：Kimi Provider 在 Gateway 重启后回退

### 现状

当前 provider 创建路径与运行路径存在两套默认值：

- built-in catalog 使用新值：`https://api.kimi.com/coding` + `k2.5`
- gateway 启动归一化逻辑使用旧值：`https://api.kimi.com/coding/v1` + `kimi-for-coding`

结果是：

1. WebUI 通过 catalog 创建 built-in Kimi provider
2. provider 写入 DB
3. gateway 重启
4. 启动逻辑扫描 DB 中的 built-in Kimi provider
5. 旧归一化逻辑覆写 `base_url/model/context_window`

这破坏了用户在 WebUI 中明确保存的配置。

### 目标行为

修复后，provider 的真值边界应为：

- catalog 仅用于“创建前预填”
- DB 仅用于“创建后运行”
- gateway 启动期不能修改已存在 provider 的业务字段

也就是说，只要 provider 已经存在于 DB 中：

- `base_url`
- `model_name`
- `protocol`
- `context_window_size`

都必须被视为已有真值，除非用户显式调用 provider 更新接口。

### 设计决策

#### 决策 A：移除或收缩 Kimi 启动归一化逻辑

`normalize_builtin_provider_defaults` 这类逻辑不能继续承担“业务字段修正”职责。

允许保留的仅限：

- 非破坏性迁移入口
- 明确版本迁移且不会覆盖用户输入的场景

不允许保留的行为：

- 根据 provider_name/provider_type 自动改写 Kimi 的 base URL
- 自动把 model 名称从 `k2.5` 改回 `kimi-for-coding`
- 自动把 context window 改写成旧常量

#### 决策 B：Kimi built-in 默认值以 catalog 为准

当前 Kimi built-in 的默认源统一收敛到 `openjax-core/src/builtin_catalog.rs`。

这意味着 gateway 相关样例、测试、断言也要全部对齐：

- `https://api.kimi.com/coding`
- `k2.5`
- `anthropic_messages`

### 模块影响

受影响模块如下：

- `openjax-gateway/src/state/config.rs`
  - 删除或收缩旧的 Kimi 归一化逻辑
- `openjax-gateway/src/state/events.rs`
  - 启动链路不再调用会覆写 provider 真值的逻辑
- `openjax-gateway/src/handlers/provider.rs`
  - 更新测试样例和断言中的旧 Kimi 值

### 验收标准

满足以下条件即视为问题一修复完成：

1. WebUI 使用 built-in Kimi 创建 provider 后，DB 中保存新默认值
2. gateway 重启后，provider 不会被回退到旧值
3. active provider snapshot 不会被旧值覆盖
4. provider 相关测试不再引用 `kimi-for-coding` 或 `https://api.kimi.com/coding/v1` 作为当前 Kimi 默认值

---

## 问题二：Anthropic Messages 多轮 Tool-Use 丢失 Reasoning 历史

### 现状

当前链路如下：

1. `anthropic_messages` 从 Kimi/Anthropic 响应中解析出：
   - `reasoning`
   - `content`（`text`/`tool_use`）
2. `planner` 收到 `ModelResponse`
3. `planner` 仅把 `response.content` 写回 `ConversationMessage::Assistant`
4. 工具执行结束后，进入下一轮请求
5. `anthropic_messages.build_request` 只能从 assistant 历史里序列化 `text` 和 `tool_use`
6. 上一轮 assistant tool-use 前的 reasoning 丢失
7. Kimi 在 thinking 开启时校验失败并返回 400

根因是 assistant 历史语义不完整：`ModelResponse` 能表达 reasoning，但 `ConversationMessage::Assistant` 不能。

### 目标行为

修复后，一条 assistant 历史消息必须能够按顺序表达：

1. `reasoning/thinking`
2. `text`
3. `tool_use`

并且这三个部分都应属于同一条 assistant message 的有序 block 序列，而不是外挂字段。

### 设计决策

#### 决策 A：把 reasoning 提升为 assistant 历史的一等内容

在消息模型中，reasoning 不能再只是 `ModelResponse.reasoning` 的临时字段。它必须成为 assistant 历史消息的一部分，能被：

- planner 写回历史
- request builder 再次读取
- 后续测试直接断言

推荐做法是扩展 `AssistantContentBlock`，新增 reasoning/thinking block 类型，而不是单独加一个 assistant 级别的附加字段。原因有三点：

1. reasoning 在语义上和 text、tool_use 一样，都是 assistant content 的有序组成部分
2. 使用 block 可以自然保持顺序
3. request builder 不需要跨多个字段拼装同一条 message

#### 决策 B：planner 负责把完整响应转成完整历史

planner 当前只把 `response.content` 回填到 `messages`。修复后，planner 的职责应是：

- 将 `response.reasoning` 转为 reasoning block
- 与 `response.content` 按顺序合并
- 一次性写入 `ConversationMessage::Assistant`

这样 assistant 历史的构造责任集中在 planner，不分散到各个 model backend。

#### 决策 C：Anthropic Messages Builder 只做“忠实重放”

`anthropic_messages.build_request` 的职责不应是“推测之前是否有 thinking”，而应是“忠实序列化已经存在的历史消息”。

也就是说：

- 如果 assistant 历史里有 reasoning block，就输出 reasoning/thinking 内容
- 如果 assistant 历史里有 text block，就输出 text
- 如果 assistant 历史里有 tool_use block，就输出 tool_use

这个 builder 不负责修补 planner 丢失的数据，只负责把正确的历史消息发出去。

### 顺序语义

assistant 历史 block 的顺序必须保留，尤其是 tool-use 之前的 thinking。设计上要求：

- reasoning block 在历史消息中的位置必须可追踪
- text 与 tool_use 的原有顺序不变
- 多个 reasoning block 的情况仍要能表达

当前 `anthropic_messages` 的 streaming/non-streaming 解析已经能分别取出 `thinking` 和 `content`。本次改动的重点不是“重新解析协议”，而是“让解析结果进入历史模型并可被后续请求重放”。

### 模块影响

受影响模块如下：

- `openjax-core/src/model/types.rs`
  - 扩展 assistant message block，增加 reasoning/thinking 表达能力
- `openjax-core/src/agent/planner.rs`
  - 把 `ModelResponse.reasoning` 与 `content` 一起写回 assistant 历史
- `openjax-core/src/model/anthropic_messages.rs`
  - 从 assistant 历史消息中序列化 reasoning block
  - 保持对 text/tool_use 的现有支持

### 验收标准

满足以下条件即视为问题二修复完成：

1. 第一轮 Kimi/Anthropic 返回 `reasoning + tool_use` 时，planner 会把 reasoning 写入 assistant 历史
2. 第二轮请求序列化 assistant 历史时，会携带对应 reasoning/thinking 内容
3. 触发 `Read` 这类工具调用后，不再出现

```text
thinking is enabled but reasoning_content is missing
```

4. 不通过关闭 thinking 或回退协议能力来规避问题

---

## 数据流设计

修复后的完整数据流如下：

1. 模型响应阶段
   - `anthropic_messages.complete` / `complete_stream`
   - 解析出 `reasoning` 与 `content`
   - 形成 `ModelResponse`

2. 历史回填阶段
   - planner 将 `ModelResponse` 转为完整 `ConversationMessage::Assistant`
   - assistant 历史 block 序列包含 reasoning、text、tool_use

3. 工具执行阶段
   - tool result 仍以 `UserContentBlock::ToolResult` 的形式追加
   - 这部分行为保持现有设计不变

4. 下一轮请求阶段
   - `anthropic_messages.build_request` 重放完整 assistant 历史消息
   - Kimi/Anthropic 收到完整 tool-use 上下文，不再因缺 reasoning 失败

5. Provider 运行时阶段
   - gateway 从 DB 读取 provider
   - 不对 Kimi provider 做启动期业务字段覆写

---

## 测试设计

### Gateway 侧测试

测试目标：

- built-in Kimi provider 使用新 catalog 值时，重启后不回退
- active provider 快照与 DB 值保持一致
- provider handler 样例与断言全部使用新 Kimi 默认值

建议测试类型：

- `state/config.rs` 附近的单测
- `gateway_api_suite` 中 provider 相关测试

### Core 侧测试

测试目标：

- assistant 消息结构能表达 reasoning block
- planner 会把 `ModelResponse.reasoning` 回填进 assistant 历史
- `anthropic_messages` builder 会把 reasoning block 正确序列化进请求

建议测试类型：

- `model/types.rs` 单测
- `model/anthropic_messages.rs` 单测
- planner 相关测试，覆盖“第一轮 tool-use + 第二轮继续推理”的最小回归路径

### 手动联调验证

最终需要用 WebUI 做一次闭环验证：

1. 配置 Kimi API Key
2. 确认 provider 显示：
   - `https://api.kimi.com/coding`
   - `k2.5`
3. 重启 gateway
4. 再次确认 provider 未回退
5. 发起一个会触发 `Read` 的请求
6. 确认不再出现缺失 `reasoning_content` 的 400

---

## 风险与边界

### 风险 1：assistant message 结构变更会影响现有测试

这是可接受影响。本次修复本质上就是把 assistant 历史语义补完整，因此相关测试需要同步更新，而不是回避类型调整。

### 风险 2：reasoning block 的顺序处理不当

如果 reasoning 不作为 block 而是外挂字段，很容易在多 block 场景中失去顺序语义。因此本设计明确拒绝外挂字段方案。

### 风险 3：把 endpoint `/v1/messages` 误判为 bug

当前 `anthropic_messages` 会对 base URL 自动补全 `/v1/messages`。这不是本次根因，本设计不触碰该逻辑，避免引入无关变化。

---

## 结论

本次设计收敛为两个明确动作：

1. Provider 真值收敛
   - built-in catalog 只负责创建默认值
   - DB 是运行时唯一真值
   - gateway 启动时不再覆写 Kimi provider 业务字段

2. Reasoning 历史闭环
   - reasoning 成为 assistant 历史消息的一等内容
   - planner 负责把完整响应写回历史
   - `anthropic_messages` 负责忠实重放完整历史

这两个动作完成后，WebUI 的 Kimi provider 配置将具备稳定持久化行为，Kimi 的 Anthropic tool-use 多轮请求也将具备完整的 reasoning 历史闭环。
