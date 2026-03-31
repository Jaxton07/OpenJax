# 2026-03-31 WebUI Busy-Turn Send Gate Design

## 1. 背景与问题

当前 WebUI 在同一会话 `turn` 仍处于 `submitting/streaming` 时，用户仍可通过输入框 `Enter` 继续发送消息。  
这会导致两类问题：

1. **前端顺序混乱**：`sendMessageAction` 会先做本地乐观插入 user message，再请求 gateway；当后端返回 `409 CONFLICT` 时，这条本地消息可能成为“悬空消息”，与后续真实事件序列不一致。
2. **提示语义错误**：当前 `CONFLICT` 被映射为登录异常文案，无法准确表达“当前回合正在进行中”。

虽然 gateway 已有同会话并发保护，不会让同一会话并发执行多个 turn，但前端体验和本地状态一致性仍然有缺陷。

## 2. 目标与非目标

### 2.1 目标

1. 在会话忙碌（`submitting/streaming`）期间，允许继续输入文本，但禁止发送。
2. 用户尝试发送时给出简短、明确、英文提示：`Please wait for the current response to finish.`
3. 避免提示刷屏：同文案短时间去重，仅弹一次。
4. 在业务层增加硬门控，确保被拦截发送不会触发乐观插入和 API 请求。
5. 修正 `CONFLICT` 文案语义，与真实错误对齐。

### 2.2 非目标

1. 不引入消息队列和“自动排队发送”能力。
2. 不修改 gateway 并发控制/turn 编排逻辑。
3. 不改动现有 `stop`（abort）主流程语义。

## 3. 方案对比与选择

### 方案 A：仅 UI 层门控

- 在 `ComposerInput` 阻止 `Enter` 和发送按钮点击。
- 风险：其他调用路径若直接触发 `sendMessage` 仍可绕过。

### 方案 B：UI + 业务层双门控（选用）

- UI 层负责交互体验（禁发送、保输入、提示）。
- 业务层（`sendMessageAction`）负责硬约束（即使绕过 UI 也不发送）。
- 优点：稳健、可维护、回归风险低。

### 方案 C：发送排队

- 会话忙碌时请求入队，turn 完成后自动发送。
- 复杂度高，超出当前需求。

**最终选择：方案 B。**

## 4. 详细设计

### 4.1 忙碌态定义

在 active session 上统一定义：

- `isBusyTurn = turnPhase === "submitting" || turnPhase === "streaming"`

该定义用于 UI 交互门控与业务层 guard 判定，避免多处条件漂移。

### 4.2 UI 层门控（Composer）

#### 4.2.1 输入与提交行为

当 `isBusyTurn=true`：

1. `textarea` 保持可编辑（不禁用）。
2. 发送按钮不可触发发送（可表现为 disabled 或拦截点击）。
3. `Enter` 不触发发送。
4. `Stop` 按钮保持可用（用于终止当前回合）。

#### 4.2.2 用户提示

当用户在忙碌态触发发送动作（按钮或 `Enter`）时：

- 显示一次 toast：`Please wait for the current response to finish.`
- 短时间内去重（建议 1500ms，允许后续可配置）。

实现建议：

- 由上层 `useChatApp` 提供 `notifyBusyTurnBlockedSend()`，内部更新 `infoToast`。
- 在 `Composer` 内部仅触发回调，不直接拼接文案，保持文案中心化。

### 4.3 业务层硬门控（sendMessageAction）

在 `sendMessageAction` 乐观插入前新增 guard：

1. 读取当前 `session.turnPhase`（通过参数注入当前 session 快照或查询函数）。
2. 若为 `submitting/streaming`：
   - 直接返回；
   - 仅设置 `infoToast` 为目标文案（可复用去重逻辑）；
   - **不得**执行 `updateSession(...messages.push(user...))`；
   - **不得**调用 `client.submitTurn(...)`。

该 guard 是防绕过安全网，不依赖 UI 层行为正确性。

### 4.4 CONFLICT 文案修正

在 `ui/web/src/lib/errors.ts` 中更新 `CONFLICT` 映射：

- 旧：登录异常语义
- 新：`Please wait for the current response to finish.`

这样即使触发后端兜底 `409`，用户也看到正确提示。

### 4.5 去重策略

增加轻量去重规则：

1. 仅针对相同 toast 文案去重；
2. 去重窗口建议 `1500ms`；
3. 去重窗口内重复触发不再次刷新 toast；
4. 窗口外再次触发可正常提示。

可放置于 `useChatApp`（集中处理 UI 反馈），避免分散在组件和 action。

## 5. 影响范围

## 5.1 前端文件（预期）

1. `ui/web/src/App.tsx`
2. `ui/web/src/hooks/useChatApp.ts`
3. `ui/web/src/hooks/chatApp/session-actions.ts`
4. `ui/web/src/components/composer/index.tsx`
5. `ui/web/src/components/composer/ComposerInput.tsx`
6. `ui/web/src/lib/errors.ts`

### 5.2 后端文件

无修改（gateway 保持不变）。

## 6. 测试设计

### 6.1 Composer 组件测试

新增/更新测试覆盖：

1. `isBusyTurn=true` 时 `Enter` 不调用 `onSend`。
2. `isBusyTurn=true` 时发送按钮不可发送，输入框仍可输入。
3. 忙碌态触发发送仅出现一次提示（去重窗口内重复触发不新增）。

### 6.2 sendMessageAction 单元测试

新增测试覆盖：

1. busy guard 生效：
   - 不调用 `submitTurn`
   - 不追加本地 user message
   - 仅触发 infoToast
2. 非 busy 时现有流程不回归：
   - 乐观插入 + submitTurn 保持原行为。

### 6.3 错误文案映射测试

更新 `errors` 测试：

1. `CONFLICT` -> `Please wait for the current response to finish.`
2. `UNAUTHENTICATED/FORBIDDEN` 映射不变。

## 7. 验收标准（DoD）

1. 会话处于 `submitting/streaming` 时，用户可输入但无法发送。
2. 忙碌态发送尝试仅弹出一条短提示，且短时间内不刷屏。
3. 被拦截发送不会触发乐观消息插入，不会调用 `submitTurn`。
4. `CONFLICT` 显示准确英文提示，不再出现登录异常语义。
5. 现有 stop、非忙碌发送主流程回归通过。

## 8. 风险与回滚点

### 8.1 风险

1. UI 与业务层条件不一致导致边界行为分裂。
2. 去重窗口过长造成用户感知“无反馈”。

### 8.2 缓解

1. 统一 `isBusyTurn` 判定，复用同一条件来源。
2. 去重仅对同文案生效，窗口保持短（约 1.5s）。

### 8.3 回滚点

若出现异常，可先保留业务层 hard guard，仅临时关闭 UI 端去重逻辑，确保正确性优先。

