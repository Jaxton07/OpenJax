# Web UI Policy Level Picker — Design Spec

**Date**: 2026-03-25
**Status**: Approved (v4, align on allow/ask/deny)

## 背景

Web UI 输入框底部需要一个权限层级切换按钮，让用户查看和切换当前会话的 policy level。TUI 已有该功能但存在不必要的字符串映射层，本次一并清理对齐。

## 核心简化原则

`DecisionKind::as_str()` 已经返回 `"allow"/"ask"/"deny"`，`policy_default_decision_name()` 也直接返回这些字符串。问题根源是 `PolicyLevel::as_str()` 用了不同名称（`"permissive"/"standard"/"strict"`），导致 TUI 和 gateway 都需要额外映射。

**解法：只改 `policy_level.rs` 的字符串表示，对齐 `DecisionKind`。不新增任何 Agent 方法，不改 Agent struct。**

## 三个层级定义（统一后）

| 对外字符串 | PolicyLevel 枚举 | DecisionKind | 摘要文字 |
|-----------|----------------|--------------|---------|
| `allow` | `Permissive` | `Allow` | Allow all tools without asking |
| `ask` | `Standard` | `Ask` | Ask before risky operations |
| `deny` | `Strict` | `Deny` | Deny all risky operations |

## 架构变更

### Core（openjax-core）— 1 个文件

**`src/agent/policy_level.rs`**

仅修改字符串表示，枚举结构不变：

```rust
impl PolicyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyLevel::Permissive => "allow",   // 原 "permissive"
            PolicyLevel::Standard   => "ask",     // 原 "standard"
            PolicyLevel::Strict     => "deny",    // 原 "strict"
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "allow" => Some(PolicyLevel::Permissive),  // 原 "permissive"
            "ask"   => Some(PolicyLevel::Standard),    // 原 "standard"
            "deny"  => Some(PolicyLevel::Strict),      // 原 "strict"
            _ => None,
        }
    }
}
```

`to_decision_kind()` 不变。`policy_default_decision_name()` 不变（已返回 "allow"/"ask"/"deny"）。
不需要新增 `get_policy_level()` 方法——gateway GET 直接调 `policy_default_decision_name()`。

同步更新文件内已有单测的期望值（从 "permissive"/"standard"/"strict" 改为 "allow"/"ask"/"deny"）。

### Gateway（openjax-gateway）— 3 个文件

**`src/handlers/session.rs`**

1. 新增 GET DTO 和 handler：

```rust
#[derive(Debug, Serialize)]
pub struct GetPolicyLevelResponse {
    pub session_id: String,
    pub level: String,
}

/// GET /api/v1/sessions/:session_id/policy
pub async fn get_policy_level(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<GetPolicyLevelResponse>, ApiError> {
    let agent_arc = {
        let session_runtime = state.get_session(&session_id).await?;
        let session = session_runtime.lock().await;
        session.agent.clone()
    };
    let level = agent_arc.lock().await.policy_default_decision_name();
    Ok(Json(GetPolicyLevelResponse {
        session_id,
        level: level.to_string(),
    }))
}
```

2. 更新 `set_policy_level` 错误提示：
```rust
"'{}' is not a valid policy level; use allow, ask, or deny"
```

**`src/lib.rs`**

```rust
.route(
    "/api/v1/sessions/:session_id/policy",
    get(handlers::get_policy_level).put(handlers::set_policy_level),
)
```

**`tests/gateway_api.rs`**

更新 policy 相关测试：`"permissive"` → `"allow"`（line 887、894）。

### TUI（ui/tui）— 2 个文件

**`src/runtime.rs`**

将 picker 数组（line 78）从：
```rust
let levels = ["permissive", "standard", "strict"];
```
改为：
```rust
let levels = ["allow", "ask", "deny"];
```
`policy_default_decision_name()` 调用（line 48）**无需修改**，已返回正确字符串。

**`src/app/render_model.rs`**

简化 `policy_level_display`，去掉翻译层，直接透传：

```rust
fn policy_level_display(level: Option<&str>) -> (&'static str, Color) {
    match level {
        Some("allow") => ("allow", Color::Cyan),
        Some("deny")  => ("deny", Color::Yellow),
        _             => ("ask", Color::White),
    }
}
```

### Web（ui/web）— 8 个文件 + 1 个新建

**`src/types/gateway.ts`**
```ts
export interface GetPolicyLevelResponse {
  session_id: string;
  level: "allow" | "ask" | "deny";
}
```

**`src/lib/gatewayClient.ts`**
```ts
async getPolicyLevel(sessionId: string): Promise<GetPolicyLevelResponse> {
  return this.request(`/api/v1/sessions/${sessionId}/policy`, { method: "GET" });
}
```

**`src/types/chat.ts`**

`ChatSession` 加可选字段，`undefined` 表示"尚未从后端获取"：
```ts
policyLevel?: "allow" | "ask" | "deny";
```
UI 层用 `policyLevel ?? "ask"` 处理 undefined（"ask" 是后端初始默认值）。

**`src/hooks/chatApp/session-actions.ts`**

新增两个 action（遵循现有委托模式）：

- `fetchPolicyLevel(client, sessionId, updateSession)` — 调 GET，更新对应 session 的 `policyLevel`；返回非 200 或网络错误时静默 fallback（不更新 state，UI 保持 `"ask"`），不写 `globalError`。
- `changePolicyLevel(client, sessionId, level, updateSession, onAuthError)` — 调 PUT：
  - 成功：同步更新 `session.policyLevel`
  - 鉴权错误（401 / UNAUTHENTICATED / FORBIDDEN）：调用 `onAuthError`
  - 其他错误：写入 `globalError`

**`src/hooks/useChatApp.ts`**

- 新增 `useEffect`，依赖数组为 `[activeSession?.id]`：若 session 存在且非 draft 时调用 `fetchPolicyLevel`。
- 暴露 `sendPolicyLevel(sessionId, level)` callback，内部调用 `changePolicyLevel`。

**`src/App.tsx`**
```tsx
policyLevel={activeSession?.policyLevel ?? "ask"}
onPolicyLevelChange={(level) => void sendPolicyLevel(activeSession!.id, level)}
```

**`src/components/composer/index.tsx`**

接收并透传 `policyLevel` / `onPolicyLevelChange` 给 `<ComposerInput>`。

**`src/components/composer/ComposerInput.tsx`**

在 `<ContextUsageRing>` 后插入：
```tsx
<PolicyLevelButton level={policyLevel} onChange={onPolicyLevelChange} />
```
（`.composer` 已有 `position: relative`，无需修改 CSS 容器。）

**新建 `src/components/composer/PolicyLevelButton.tsx`**
- `open: boolean` 状态控制 popover 显隐
- 渲染：pill 按钮，`position: absolute; left: 42px; bottom: 10px`，显示当前 level + `UpDownIcon`
- 点击展开向上 popover，3 行选项（当前层级高亮），点击调 `onChange(level)` 后关闭
- `useRef + useEffect` 监听 `mousedown` 实现点击外部关闭

**`src/pic/icon/index.tsx`**

新增 `UpDownIcon`，内联 `up_down.svg` 两条 path（viewBox `0 0 1463 1024`）。

**`src/components/composer/composer.css`**

新增：
- `.policy-level-btn` — pill 样式，绝对定位，与 `.context-usage-ring` 视觉一致
- `.policy-level-popover` — 向上弹出，`position: absolute; bottom: calc(100% + 4px); z-index: 30`
- `.policy-level-option` — 行选项，hover 高亮
- `.policy-level-option.active` — 当前层级高亮

## 数据流

```
activeSession?.id 变化（非 draft session）
  → useChatApp: fetchPolicyLevel(sessionId)
      → GET /api/v1/sessions/:id/policy
      → 200: 更新 session.policyLevel ("allow"/"ask"/"deny")
      → 非 200 / 网络错误: 静默，UI 保持 "ask"
  → App.tsx → Composer → ComposerInput → PolicyLevelButton 显示

用户点击层级选项
  → PolicyLevelButton.onChange(level)
  → useChatApp: changePolicyLevel(sessionId, level)
      → PUT /api/v1/sessions/:id/policy { level }
      → 成功: 更新 session.policyLevel
      → 401: onAuthError
      → 其他: globalError
  → PolicyLevelButton 重新渲染
```

## 边界处理

| 场景 | 处理方式 |
|------|---------|
| GET 时 session 是本地 draft（未在后端创建） | 不发 GET，UI 显示默认 "ask" |
| GET 返回 404 / 网络错误 | 静默 fallback，不写 globalError |
| PUT 401 | onAuthError → clearAuthState |
| PUT 其他错误 | globalError |
| session 切换竞态 | updateSession 回调内匹配 sessionId，乱序响应无副作用 |

## 文件变更清单

| 文件 | 操作 |
|------|------|
| `openjax-core/src/agent/policy_level.rs` | edit — `as_str`/`from_str` 改用 "allow"/"ask"/"deny"，更新单测 |
| `openjax-gateway/src/handlers/session.rs` | edit — 加 `GetPolicyLevelResponse` + `get_policy_level` handler，更新错误提示 |
| `openjax-gateway/src/lib.rs` | edit — 路由加 `.get()` |
| `openjax-gateway/tests/gateway_api.rs` | edit — 测试值 "permissive" → "allow" |
| `ui/tui/src/runtime.rs` | edit — picker 数组改为 "allow"/"ask"/"deny" |
| `ui/tui/src/app/render_model.rs` | edit — 简化 `policy_level_display` |
| `ui/web/src/types/gateway.ts` | edit — 加 `GetPolicyLevelResponse` |
| `ui/web/src/lib/gatewayClient.ts` | edit — 加 `getPolicyLevel` |
| `ui/web/src/types/chat.ts` | edit — `ChatSession` 加 `policyLevel?` |
| `ui/web/src/hooks/chatApp/session-actions.ts` | edit — 加 `fetchPolicyLevel` / `changePolicyLevel` |
| `ui/web/src/hooks/useChatApp.ts` | edit — useEffect + 暴露 `sendPolicyLevel` |
| `ui/web/src/App.tsx` | edit — 传 props |
| `ui/web/src/components/composer/index.tsx` | edit — 透传 props |
| `ui/web/src/components/composer/ComposerInput.tsx` | edit — 渲染 PolicyLevelButton |
| `ui/web/src/components/composer/PolicyLevelButton.tsx` | create |
| `ui/web/src/pic/icon/index.tsx` | edit — 加 UpDownIcon |
| `ui/web/src/components/composer/composer.css` | edit — 加样式 |

共 17 个文件，1 个新建。

## 测试计划

### openjax-core
- 更新 `policy_level.rs` 单测期望值（"permissive"→"allow" 等）
- 验证 `from_str("allow")` / `from_str("ask")` / `from_str("deny")` round-trip

### openjax-gateway
- `GET /api/v1/sessions/:id/policy` 返回 200 + `{ level: "ask" }`（初始态）
- PUT allow → GET 验证返回 `"allow"`
- 更新现有 "permissive" 测试为 "allow"

### ui/tui
- 现有 policy picker 集成测试（m23）继续通过

### ui/web
- `gatewayClient.test.ts`：mock GET /policy，验证 `getPolicyLevel` 解析正确
- `PolicyLevelButton` 组件测试：渲染当前 level；点击调 `onChange`；点击外部关闭
- `changePolicyLevel` 错误路径测试：PUT 成功 / 401 / 其他错误三条路径
- `pnpm test` 全量通过
