# 设计文档：草稿态 Policy Level（方案 C）

**日期：** 2026-03-31
**范围：** `ui/web` 前端，不涉及后端 API 变更
**状态：** 已批准，待实施

---

## 问题

WebUI 已改为"点击新建对话不立即创建远端 session"，草稿态下 `activeSession = null`。
此时点击 PolicyLevelButton 触发 `sendPolicyLevel(activeSession!.id, level)`，因 `activeSession` 为 null 导致：

```
App.tsx:224 Uncaught TypeError: Cannot read properties of null (reading 'id')
```

结果：用户无法在首条消息发送前调整 policy，默认始终以 `ask` 执行，工具调用频繁卡审批。

---

## 目标

1. 草稿态可切换 policy（allow/ask/deny），不报错。
2. 保持"点击新建不落盘、不创建空 session 目录"。
3. 发送首条消息时：先创建 session，再应用所选 policy，再发消息。
4. policy 状态按会话隔离，草稿态与各 session 互不干扰。

---

## 不采用的方案

**方案 B（点击新建即创建 session）：** 引入"空 session 定义/复用/清理/并发竞争"复杂度，架构语义不干净。放弃。

---

## 设计（方案 C：草稿 policy + 首发前落库）

### 1. 状态模型

`draftPolicyLevel` 作为独立 `useState` 存在于 `useChatApp.ts`，**不**进入 `ChatState`（草稿态纯前端，无需持久化）：

```ts
const [draftPolicyLevel, setDraftPolicyLevel] = useState<"allow" | "ask" | "deny">("ask");
const draftPolicyLevelRef = useRef(draftPolicyLevel);
useEffect(() => { draftPolicyLevelRef.current = draftPolicyLevel; }, [draftPolicyLevel]);
```

- `activeSession == null`：Composer 展示 `draftPolicyLevel`
- `activeSession != null`：Composer 展示 `activeSession.policyLevel ?? "ask"`（原有逻辑不变）
- `newChat()` 后：调用 `setDraftPolicyLevel("ask")` 重置（每次草稿从 ask 起步）

### 2. 统一 `onPolicyLevelChange` handler

在 `useChatApp.ts` 新增 callback，对外暴露，替代 `App.tsx` 中内联的 `activeSession!.id` 调用：

```ts
const onPolicyLevelChange = useCallback((level: "allow" | "ask" | "deny") => {
  if (activeSession != null) {
    void sendPolicyLevel(activeSession.id, level); // 有 session：走远端 API
  } else {
    setDraftPolicyLevel(level);                    // 草稿态：只更新本地
  }
}, [activeSession, sendPolicyLevel]);
```

`App.tsx` 改为：

```tsx
policyLevel={activeSession?.policyLevel ?? draftPolicyLevel}
onPolicyLevelChange={onPolicyLevelChange}
```

（`draftPolicyLevel` 和 `onPolicyLevelChange` 从 `useChatApp` return 出来）

### 3. 首发前应用 draft policy（`sendMessageAction`）

**新增参数（`SendMessageParams`）：**

```ts
isDraftSend?: boolean;
getDraftPolicyLevel?: () => "allow" | "ask" | "deny";
```

**插入位置：`ensureSession()` 之后，`tryBeginSubmit` 之前**

逻辑：

```
if isDraftSend && getDraftPolicyLevel() !== "ask":
  try:
    withAuthRetry(() => client.setPolicyLevel(sessionId, level))
    updateSession(sessionId, s => ({ ...s, policyLevel: level }))
  catch auth error:
    clearAuthState("登录态已失效，请重新登录。")
    return  ← 中止，不继续 submitTurn
  catch other error:
    setState(prev => ({ ...prev, globalError: humanizeError(error) }))
    return  ← 中止，不继续 submitTurn
```

`useChatApp.ts` 的 `sendMessage` callback 传入：

```ts
isDraftSend: !targetSessionId,
getDraftPolicyLevel: () => draftPolicyLevelRef.current,
```

`targetSessionId` 在 `sendMessage` 入口处捕获（`activeSessionIdRef.current`），准确反映"发送时是否为草稿"。

**`setPolicyLevel` 失败时的 session 状态说明（已知 tradeoff）：**

`ensureSession()` 在 `setPolicyLevel` 之前执行，因此失败时远端 session 已落库并出现在 sidebar。此时：
- 发送已中止，`globalError` 已展示，用户可感知 —— 不属于静默降级
- 该 session 无消息，`policyLevel` 为 `undefined`（展示为 `ask`）
- 不做额外回滚：删除 session 需要额外网络调用，引入"失败中的失败"复杂度
- 用户重试路径：点「新建对话」进入草稿态（`draftPolicyLevel` 保留），重新发送即可

此 tradeoff 可接受；若未来需要改善，可在失败时将 `activeSessionId` 置为 `null` 使用户回到草稿态。

---

## 改动文件

| 文件 | 改动 |
|------|------|
| `ui/web/src/App.tsx` | 去掉 `activeSession!.id` 非空断言；`policyLevel` 改用 `draftPolicyLevel`；`onPolicyLevelChange` 改用 hook 返回值 |
| `ui/web/src/hooks/useChatApp.ts` | 增加 `draftPolicyLevel` state + ref；新增 `onPolicyLevelChange` callback；`newChat` 后 reset；`sendMessage` 传 `isDraftSend` + `getDraftPolicyLevel`；return 新增两个字段 |
| `ui/web/src/hooks/chatApp/session-actions.ts` | `SendMessageParams` 增加两个可选字段；`sendMessageAction` 插入 policy 应用逻辑 |

---

## 测试要求

新增/更新测试（主要在 `session-actions.test.ts` 和 `useChatApp.new-chat.test.ts`）：

1. **草稿切 policy 不调 API：** `onPolicyLevelChange('allow')` when `activeSession == null` → 仅更新 `draftPolicyLevel`，`setPolicyLevel` 未被调用。
2. **草稿首发调用顺序：** `startSession → setPolicyLevel → submitTurn`，且 policy 写入 session 状态。
3. **`setPolicyLevel` 失败时中止：** `submitTurn` 未被调用，`globalError` 被设置。
4. **`setPolicyLevel` 为 `ask` 时跳过：** `isDraftSend=true` 但 `draftPolicyLevel=="ask"` → 不调用 `setPolicyLevel`，直接 `submitTurn`。
5. **已有 session 切 policy：** `onPolicyLevelChange` 调用远端 API，不影响 `draftPolicyLevel`。
6. **会话隔离：** 切换到已有 session 展示其 `policyLevel`；返回草稿展示 `draftPolicyLevel`。
7. **`newChat` 重置：** `newChat()` 后 `draftPolicyLevel` 回到 `"ask"`。
8. **保持旧行为：** 新建对话不创建远端 session 目录（现有测试不回归）。

---

## 验收标准

1. 草稿态点击 PolicyLevelButton 无报错，展示选中值。
2. 点击新建对话后，不产生新 session 目录。
3. 首条消息发送后才创建 session；且按选定 policy 生效（allow 不再被 ask 卡住）。
4. 切换会话后 policy 展示与生效正确、互不串扰。
5. `setPolicyLevel` 失败时发送被中止，不以默认 ask 静默降级。
