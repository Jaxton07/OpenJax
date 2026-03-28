# Policy Level Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 统一 policy level 字符串为 "allow"/"ask"/"deny"，新增 GET 接口，并在 Web UI Composer 输入框底部加入 policy level 切换按钮。

**Architecture:** 修改 `PolicyLevel::as_str()`/`from_str()` 对齐 `DecisionKind` 字符串（消除映射层）；Gateway 新增 GET endpoint 读取当前层级；TUI 移除 display 翻译层；Web 新增 `PolicyLevelButton` 组件，通过 `session-actions.ts` 的两个 action 与后端交互。

**Tech Stack:** Rust (openjax-core, openjax-gateway, ui/tui), React + TypeScript (ui/web/src), CSS (composer.css)

---

## File Map

| 文件 | 操作 | 职责 |
|------|------|------|
| `openjax-core/src/agent/policy_level.rs` | edit | 改字符串表示，更新单测 |
| `openjax-gateway/src/handlers/session.rs` | edit | 加 GET handler + DTO，更新错误提示 |
| `openjax-gateway/src/lib.rs` | edit | 路由加 `.get()` |
| `openjax-gateway/tests/gateway_api.rs` | edit | 测试值 permissive→allow |
| `ui/tui/src/runtime.rs` | edit | picker 数组改为 allow/ask/deny |
| `ui/tui/src/app/render_model.rs` | edit | 简化 policy_level_display |
| `ui/web/src/types/gateway.ts` | edit | 加 GetPolicyLevelResponse 类型 |
| `ui/web/src/lib/gatewayClient.ts` | edit | 加 getPolicyLevel 方法 |
| `ui/web/src/types/chat.ts` | edit | ChatSession 加 policyLevel? 字段 |
| `ui/web/src/hooks/chatApp/session-actions.ts` | edit | 加 fetchPolicyLevel / changePolicyLevel |
| `ui/web/src/hooks/useChatApp.ts` | edit | useEffect + sendPolicyLevel callback |
| `ui/web/src/App.tsx` | edit | 向 Composer 传 policyLevel / onPolicyLevelChange |
| `ui/web/src/components/composer/index.tsx` | edit | 透传 props 给 ComposerInput |
| `ui/web/src/components/composer/ComposerInput.tsx` | edit | 渲染 PolicyLevelButton |
| `ui/web/src/components/composer/PolicyLevelButton.tsx` | create | 按钮 + popover 组件 |
| `ui/web/src/pic/icon/index.tsx` | edit | 加 UpDownIcon |
| `ui/web/src/components/composer/composer.css` | edit | 加 policy button + popover 样式 |

---

### Task 1: 统一 PolicyLevel 字符串（Core）

**Files:**
- Modify: `openjax-core/src/agent/policy_level.rs`

- [ ] **Step 1: 修改 `as_str()` 和 `from_str()`**

将以下内容整体替换：

```rust
// openjax-core/src/agent/policy_level.rs
use openjax_policy::schema::DecisionKind;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PolicyLevel {
    Permissive,
    Standard,
    Strict,
}

impl PolicyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyLevel::Permissive => "allow",
            PolicyLevel::Standard => "ask",
            PolicyLevel::Strict => "deny",
        }
    }

    pub fn to_decision_kind(self) -> DecisionKind {
        match self {
            PolicyLevel::Permissive => DecisionKind::Allow,
            PolicyLevel::Standard => DecisionKind::Ask,
            PolicyLevel::Strict => DecisionKind::Deny,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "allow" => Some(PolicyLevel::Permissive),
            "ask" => Some(PolicyLevel::Standard),
            "deny" => Some(PolicyLevel::Strict),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_round_trips() {
        for level in [PolicyLevel::Permissive, PolicyLevel::Standard, PolicyLevel::Strict] {
            assert_eq!(PolicyLevel::from_str(level.as_str()), Some(level));
        }
    }

    #[test]
    fn from_str_returns_none_for_invalid() {
        assert!(PolicyLevel::from_str("unknown").is_none());
        assert!(PolicyLevel::from_str("").is_none());
        assert!(PolicyLevel::from_str("permissive").is_none());
    }

    #[test]
    fn to_decision_kind_maps_correctly() {
        assert_eq!(PolicyLevel::Permissive.to_decision_kind(), DecisionKind::Allow);
        assert_eq!(PolicyLevel::Standard.to_decision_kind(), DecisionKind::Ask);
        assert_eq!(PolicyLevel::Strict.to_decision_kind(), DecisionKind::Deny);
    }
}
```

- [ ] **Step 2: 运行 core 单测验证**

```bash
zsh -lc "cargo test -p openjax-core"
```

期望：全部通过，`from_str_round_trips` 和 `from_str_returns_none_for_invalid` 均 PASS。

- [ ] **Step 3: Commit**

```bash
git add openjax-core/src/agent/policy_level.rs
git commit -m "refactor(core): align PolicyLevel strings with DecisionKind (allow/ask/deny)"
```

---

### Task 2: Gateway 新增 GET endpoint

**Files:**
- Modify: `openjax-gateway/src/handlers/session.rs`
- Modify: `openjax-gateway/src/lib.rs`

- [ ] **Step 1: 在 `session.rs` 的 policy level 区块末尾追加 GET DTO 和 handler**

在 `set_policy_level` 函数结束（`}`）后追加：

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
    let level = agent_arc.lock().await.policy_default_decision_name().to_string();
    Ok(Json(GetPolicyLevelResponse { session_id, level }))
}
```

同时将 `set_policy_level` 的错误提示改为：

```rust
"'{}' is not a valid policy level; use allow, ask, or deny",
```

- [ ] **Step 2: 在 `lib.rs` 路由上追加 `.get()`**

找到：
```rust
"/api/v1/sessions/:session_id/policy",
put(handlers::set_policy_level),
```

改为：
```rust
"/api/v1/sessions/:session_id/policy",
get(handlers::get_policy_level).put(handlers::set_policy_level),
```

- [ ] **Step 3: 构建验证**

```bash
zsh -lc "cargo build -p openjax-gateway"
```

期望：编译通过，无 warning。

- [ ] **Step 4: 更新集成测试**

在 `openjax-gateway/tests/gateway_api.rs` 中找到：
```rust
.body(Body::from(r#"{"level":"permissive"}"#))
```
改为：
```rust
.body(Body::from(r#"{"level":"allow"}"#))
```

以及：
```rust
assert_eq!(body["level"], "permissive");
```
改为：
```rust
assert_eq!(body["level"], "allow");
```

- [ ] **Step 5: 运行 gateway 测试**

```bash
zsh -lc "cargo test -p openjax-gateway"
```

期望：全部通过。

- [ ] **Step 6: Commit**

```bash
git add openjax-gateway/src/handlers/session.rs openjax-gateway/src/lib.rs openjax-gateway/tests/gateway_api.rs
git commit -m "feat(gateway): GET /sessions/:id/policy + align level strings to allow/ask/deny"
```

---

### Task 3: TUI 对齐

**Files:**
- Modify: `ui/tui/src/runtime.rs`
- Modify: `ui/tui/src/app/render_model.rs`

**注意：必须在 Task 1 编译通过后执行，因为 TUI 依赖 `openjax-core` 的 `PolicyLevel::from_str`。**

- [ ] **Step 1: 修改 `runtime.rs` picker 数组**

找到（约 line 78）：
```rust
let levels = ["permissive", "standard", "strict"];
```
改为：
```rust
let levels = ["allow", "ask", "deny"];
```

- [ ] **Step 2: 简化 `render_model.rs` 的 `policy_level_display`**

找到：
```rust
fn policy_level_display(level: Option<&str>) -> (&'static str, Color) {
    match level {
        Some("permissive") | Some("allow") => ("permissive", Color::Cyan),
        Some("strict") | Some("deny") => ("strict", Color::Yellow),
        _ => ("standard", Color::White),
    }
}
```
替换为：
```rust
fn policy_level_display(level: Option<&str>) -> (&'static str, Color) {
    match level {
        Some("allow") => ("allow", Color::Cyan),
        Some("deny")  => ("deny", Color::Yellow),
        _             => ("ask", Color::White),
    }
}
```

- [ ] **Step 3: 运行 TUI 测试**

```bash
zsh -lc "cargo test -p tui_next"
```

期望：全部通过，特别是 `m23_policy_picker` 相关测试。

- [ ] **Step 4: Commit**

```bash
git add ui/tui/src/runtime.rs ui/tui/src/app/render_model.rs
git commit -m "refactor(tui): align policy level display to allow/ask/deny, remove translation layer"
```

---

### Task 4: Web 类型与客户端

**Files:**
- Modify: `ui/web/src/types/gateway.ts`
- Modify: `ui/web/src/lib/gatewayClient.ts`
- Modify: `ui/web/src/types/chat.ts`

- [ ] **Step 1: 在 `gateway.ts` 末尾追加类型**

```ts
export interface GetPolicyLevelResponse {
  session_id: string;
  level: "allow" | "ask" | "deny";
}
```

- [ ] **Step 2: 在 `gatewayClient.ts` import 列表加类型，并在 class 末尾（`healthCheck` 之前或之后）添加两个方法**

import 行加 `GetPolicyLevelResponse`：
```ts
import type {
  // ...existing imports...
  GetPolicyLevelResponse,
  // ...
} from "../types/gateway";
```

方法（两个一起加，Task 5 会用到 `setPolicyLevel`）：
```ts
async getPolicyLevel(sessionId: string): Promise<GetPolicyLevelResponse> {
  return this.request(`/api/v1/sessions/${sessionId}/policy`, { method: "GET" });
}

async setPolicyLevel(sessionId: string, level: string): Promise<{ level: string }> {
  return this.request(`/api/v1/sessions/${sessionId}/policy`, {
    method: "PUT",
    body: JSON.stringify({ level })
  });
}
```

- [ ] **Step 3: 在 `chat.ts` 的 `ChatSession` 接口加字段**

在 `streaming?: SessionStreamingState;` 之后追加：
```ts
policyLevel?: "allow" | "ask" | "deny";
```

- [ ] **Step 4: 运行 web 测试验证类型无误**

```bash
zsh -lc "cd ui/web && pnpm test"
```

期望：全部通过。

- [ ] **Step 5: Commit**

```bash
git add ui/web/src/types/gateway.ts ui/web/src/lib/gatewayClient.ts ui/web/src/types/chat.ts
git commit -m "feat(web): add GetPolicyLevelResponse type, getPolicyLevel client method, policyLevel field"
```

---

### Task 5: Web session-actions

**Files:**
- Modify: `ui/web/src/hooks/chatApp/session-actions.ts`

- [ ] **Step 1: 在文件末尾追加两个 action**

```ts
// ---------------------------------------------------------------------------
// Policy level
// ---------------------------------------------------------------------------

interface FetchPolicyLevelParams {
  client: GatewayClient;
  sessionId: string;
  updateSession: (sessionId: string, updater: (session: ChatSession) => ChatSession) => void;
}

export async function fetchPolicyLevelAction(params: FetchPolicyLevelParams): Promise<void> {
  try {
    const response = await params.client.getPolicyLevel(params.sessionId);
    params.updateSession(params.sessionId, (session) => ({
      ...session,
      policyLevel: response.level
    }));
  } catch {
    // 静默 fallback：404（session 未在后端创建）或网络错误，不写 globalError
  }
}

interface ChangePolicyLevelParams {
  client: GatewayClient;
  sessionId: string;
  level: "allow" | "ask" | "deny";
  updateSession: (sessionId: string, updater: (session: ChatSession) => ChatSession) => void;
  clearAuthState: (message: string) => void;
  setState: SetState;
}

export async function changePolicyLevelAction(params: ChangePolicyLevelParams): Promise<void> {
  try {
    await params.client.setPolicyLevel(params.sessionId, params.level);
    params.updateSession(params.sessionId, (session) => ({
      ...session,
      policyLevel: params.level
    }));
  } catch (error) {
    if (isAuthenticationError(error)) {
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    params.setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
  }
}
```

- [ ] **Step 2: 运行 web 测试**

```bash
zsh -lc "cd ui/web && pnpm test"
```

期望：全部通过。

- [ ] **Step 3: Commit**

```bash
git add ui/web/src/hooks/chatApp/session-actions.ts ui/web/src/lib/gatewayClient.ts
git commit -m "feat(web): add fetchPolicyLevelAction / changePolicyLevelAction"
```

---

### Task 6: useChatApp 接入

**Files:**
- Modify: `ui/web/src/hooks/useChatApp.ts`

- [ ] **Step 1: 导入新 actions**

在 `useChatApp.ts` 顶部 import session-actions 的行，追加：
```ts
import {
  // ...existing imports...
  fetchPolicyLevelAction,
  changePolicyLevelAction,
} from "./chatApp/session-actions";
```

- [ ] **Step 2: 在 SSE `useEffect`（line 356 附近）之后、`hydrateSessionsFromGateway` 之前，新增 policy fetch effect**

```ts
// 切换会话时从后端同步 policy level
useEffect(() => {
  const sessionId = activeSession?.id;
  if (!sessionId || !state.auth.authenticated || !state.auth.accessToken) {
    return;
  }
  void fetchPolicyLevelAction({ client, sessionId, updateSession });
}, [activeSession?.id, state.auth.authenticated, state.auth.accessToken, client, updateSession]);
```

- [ ] **Step 3: 在 `compactConversation` callback 之后新增 `sendPolicyLevel`**

```ts
const sendPolicyLevel = useCallback(
  async (sessionId: string, level: "allow" | "ask" | "deny") => {
    await changePolicyLevelAction({
      client,
      sessionId,
      level,
      updateSession,
      clearAuthState,
      setState
    });
  },
  [clearAuthState, client, updateSession]
);
```

- [ ] **Step 4: 在 return 对象里暴露 `sendPolicyLevel`**

```ts
return {
  // ...existing...
  sendPolicyLevel,
  dismissGlobalError,
  dismissToast
};
```

- [ ] **Step 5: 运行测试**

```bash
zsh -lc "cd ui/web && pnpm test"
```

期望：全部通过。

- [ ] **Step 6: Commit**

```bash
git add ui/web/src/hooks/useChatApp.ts
git commit -m "feat(web): fetch policy level on session switch, expose sendPolicyLevel"
```

---

### Task 7: App.tsx → Composer props 链路

**Files:**
- Modify: `ui/web/src/App.tsx`
- Modify: `ui/web/src/components/composer/index.tsx`
- Modify: `ui/web/src/components/composer/ComposerInput.tsx`

- [ ] **Step 1: `App.tsx` 解构 `sendPolicyLevel`，传给 Composer**

找到 `useChatApp()` 的解构，追加 `sendPolicyLevel`。

找到 `<Composer` 块，追加两个 props：
```tsx
policyLevel={activeSession?.policyLevel ?? "ask"}
onPolicyLevelChange={(level) => void sendPolicyLevel(activeSession!.id, level)}
```

- [ ] **Step 2: `composer/index.tsx` 扩展 props 接口并透传**

在 `ComposerProps` interface 追加：
```ts
policyLevel?: "allow" | "ask" | "deny";
onPolicyLevelChange?: (level: "allow" | "ask" | "deny") => void;
```

解构参数加 `policyLevel` / `onPolicyLevelChange`，透传给 `<ComposerInput>`。

- [ ] **Step 3: `ComposerInput.tsx` 扩展 props 接口，渲染 PolicyLevelButton**

在 `ComposerInputProps` 追加：
```ts
policyLevel?: "allow" | "ask" | "deny";
onPolicyLevelChange?: (level: "allow" | "ask" | "deny") => void;
```

在 `<ContextUsageRing contextUsage={contextUsage} />` 之后插入：
```tsx
{onPolicyLevelChange && (
  <PolicyLevelButton
    level={policyLevel ?? "ask"}
    onChange={onPolicyLevelChange}
  />
)}
```

import 行加：
```ts
import PolicyLevelButton from "./PolicyLevelButton";
```

- [ ] **Step 4: 类型检查（跳过 PolicyLevelButton，Task 8 完成后再做完整 build）**

```bash
zsh -lc "cd ui/web && pnpm tsc --noEmit"
```

期望：仅报 `PolicyLevelButton` 模块找不到的错误（正常，Task 8 会创建该文件），其余无类型错误。

---

### Task 8: PolicyLevelButton 组件 + 图标 + 样式

**Files:**
- Create: `ui/web/src/components/composer/PolicyLevelButton.tsx`
- Modify: `ui/web/src/pic/icon/index.tsx`
- Modify: `ui/web/src/components/composer/composer.css`

- [ ] **Step 1: 在 `pic/icon/index.tsx` 末尾追加 `UpDownIcon`**

```tsx
export function UpDownIcon(props: IconProps) {
  return (
    <svg viewBox="0 0 1463 1024" xmlns="http://www.w3.org/2000/svg" {...props}>
      <path
        d="M428.324571 353.572571 695.003429 92.306286c20.772571-20.187429 54.272-20.187429 75.044571 0l266.678857 261.266286c20.772571 20.333714 20.772571 53.248 0 73.435429-20.626286 20.333714-54.272 20.333714-75.044571 0L732.452571 202.605714 503.369143 427.008c-20.772571 20.333714-54.272 20.333714-75.044571 0C407.698286 406.820571 407.698286 373.906286 428.324571 353.572571z"
        fill="currentColor"
      />
      <path
        d="M1036.580571 669.110857 770.048 930.377143c-20.772571 20.187429-54.272 20.187429-75.044571 0L428.324571 669.110857c-20.772571-20.333714-20.772571-53.248 0-73.435429 20.626286-20.333714 54.272-20.333714 75.044571 0l229.083429 224.548571 229.083429-224.548571c20.772571-20.333714 54.272-20.333714 75.044571 0C1057.353143 616.009143 1057.353143 648.777143 1036.580571 669.110857z"
        fill="currentColor"
      />
    </svg>
  );
}
```

- [ ] **Step 2: 新建 `PolicyLevelButton.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";
import { UpDownIcon } from "../../pic/icon";

type PolicyLevel = "allow" | "ask" | "deny";

interface PolicyLevelButtonProps {
  level: PolicyLevel;
  onChange: (level: PolicyLevel) => void;
}

const LEVELS: { value: PolicyLevel; label: string; summary: string }[] = [
  { value: "allow", label: "allow", summary: "Allow all tools without asking" },
  { value: "ask",   label: "ask",   summary: "Ask before risky operations" },
  { value: "deny",  label: "deny",  summary: "Deny all risky operations" },
];

export default function PolicyLevelButton({ level, onChange }: PolicyLevelButtonProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handleMouseDown = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [open]);

  const handleSelect = (selected: PolicyLevel) => {
    setOpen(false);
    if (selected !== level) {
      onChange(selected);
    }
  };

  return (
    <div className="policy-level-wrap" ref={containerRef}>
      {open && (
        <div className="policy-level-popover" role="listbox">
          {LEVELS.map((item) => (
            <button
              key={item.value}
              type="button"
              role="option"
              aria-selected={item.value === level}
              className={`policy-level-option${item.value === level ? " active" : ""}`}
              onClick={() => handleSelect(item.value)}
            >
              <span className="policy-level-option-label">{item.label}</span>
              <span className="policy-level-option-summary">{item.summary}</span>
            </button>
          ))}
        </div>
      )}
      <button
        type="button"
        className="policy-level-btn"
        onClick={() => setOpen((v) => !v)}
        title="切换权限层级"
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <span className="policy-level-btn-text">{level}</span>
        <span className="policy-level-btn-icon" aria-hidden="true">
          <UpDownIcon />
        </span>
      </button>
    </div>
  );
}
```

- [ ] **Step 3: 在 `composer.css` 末尾追加样式**

```css
/* Policy Level Button */
.policy-level-wrap {
  position: absolute;
  left: 42px;
  bottom: 10px;
  display: inline-flex;
  align-items: center;
}

.policy-level-btn {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 24px;
  padding: 0 7px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: transparent;
  font-size: 11px;
  font-weight: 500;
  color: var(--ink-3);
  cursor: pointer;
  transition: color 0.14s ease, border-color 0.14s ease;
  white-space: nowrap;
}

.policy-level-btn:hover {
  color: var(--ink);
  border-color: var(--ink-4);
}

.policy-level-btn-icon {
  display: inline-grid;
  place-items: center;
  width: 10px;
  height: 10px;
}

.policy-level-btn-icon svg {
  width: 10px;
  height: 10px;
  fill: currentColor;
}

.policy-level-popover {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  min-width: 240px;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 8px 20px rgba(15, 23, 42, 0.12);
  overflow: hidden;
  z-index: 50;
}

[data-theme="dark"] .policy-level-popover {
  background: var(--bg-sub);
  box-shadow: 0 8px 20px rgba(2, 6, 23, 0.28);
}

.policy-level-option {
  display: flex;
  align-items: baseline;
  gap: 8px;
  width: 100%;
  padding: 8px 12px;
  border: none;
  background: transparent;
  cursor: pointer;
  text-align: left;
  transition: background 0.1s ease;
}

.policy-level-option:hover,
.policy-level-option.active {
  background: var(--bg-hover);
}

.policy-level-option-label {
  flex: 0 0 auto;
  font-size: 12px;
  font-weight: 600;
  font-family: monospace;
  color: var(--ink);
  min-width: 40px;
}

.policy-level-option.active .policy-level-option-label {
  color: var(--accent, #3b82f6);
}

.policy-level-option-summary {
  flex: 1 1 auto;
  font-size: 11px;
  color: var(--ink-4);
  white-space: nowrap;
}
```

- [ ] **Step 4: 完整构建**

```bash
zsh -lc "cd ui/web && pnpm build"
```

期望：编译通过，无类型错误。

- [ ] **Step 5: 运行全量测试**

```bash
zsh -lc "cd ui/web && pnpm test"
```

期望：全部通过。

- [ ] **Step 6: Commit**

```bash
git add ui/web/src/components/composer/PolicyLevelButton.tsx \
        ui/web/src/pic/icon/index.tsx \
        ui/web/src/components/composer/composer.css \
        ui/web/src/components/composer/ComposerInput.tsx \
        ui/web/src/components/composer/index.tsx \
        ui/web/src/App.tsx
git commit -m "feat(web): add PolicyLevelButton in composer — allow/ask/deny picker"
```

---

### Task 9: 全量验证

- [ ] **Step 1: 运行完整 Rust 测试**

```bash
zsh -lc "cargo test --workspace"
```

期望：全部通过。

- [ ] **Step 2: 运行 web 测试**

```bash
zsh -lc "cd ui/web && pnpm test"
```

期望：全部通过。

- [ ] **Step 3: 本地启动验证 UI**

```bash
zsh -lc "make run-web-dev"
```

打开 `http://127.0.0.1:5173`，登录后：
1. 输入框底部左侧可见 `ask` 文字 + 上下箭头图标
2. 点击按钮，弹出 popover 显示三个选项
3. 选择 `allow`，按钮文字变为 `allow`
4. 切换到另一个会话再切回，按钮正确显示上次设置的层级

- [ ] **Step 4: 最终 commit（若有遗漏文件）**

```bash
git add -p  # 逐块确认
git commit -m "chore: final cleanup after policy level picker integration"
```
