# Policy Level Display & Switching in TUI

**Date**: 2026-03-25
**Status**: Approved
**Scope**: openjax-core → ui/tui → openjax-gateway

---

## 1. 背景与目标

当前 `AppState` 已存储 `policy_default: Option<String>`，`set_runtime_info` 也从 `agent.policy_default_decision_name()` 读取该值，但 banner 和 footer 均未展示它。用户无法感知当前会话的策略层级，也无法在会话内快速切换。

**目标**：
1. 在 footer 常驻显示当前 policy 层级，随切换实时刷新
2. 通过 `/policy` slash 命令 + 两步式 picker 切换策略层级
3. 在 gateway 侧暴露 API 口子，供后续 web 接入

---

## 2. 策略层级定义

三档命名层级，映射到 `PolicyStore::default_decision`（`DecisionKind`）：

| 层级 | DecisionKind | 语义 |
|------|-------------|------|
| `permissive` | `Allow` | 大多数操作自动通过，仅 `destructive` 命中内置规则触发 Escalation 审批 |
| `standard` | `Ask` | 大多数操作需要审批（默认值）|
| `strict` | `Deny` | 拒绝所有未被规则显式 Allow 的操作（`system:destructive_escalate` 仍有效，将 destructive 命令提升为 Escalate 而非 Deny） |

**约束**：`system:destructive_escalate`（priority=1000）不受层级影响，任何层级下均有效。层级变更只在当前进程内有效，不持久化。

---

## 3. Footer 显示设计

### 3.1 格式

在现有 hint 文字末尾追加竖杠分隔的 policy 指示器：

```
Enter submit | / commands | Esc clear | Ctrl-C quit | policy: standard
```

不同模式下 footer 内容：

| `FooterMode` | 左侧 hint | 右侧 policy |
|---|---|---|
| `Idle` | `Enter submit | / commands | Esc clear | Ctrl-C quit` | `| policy: <level>` |
| `SlashActive` | `Tab/Enter complete | Esc dismiss` | `| policy: <level>` |
| `ApprovalActive` | `↑↓ select | Enter confirm | Esc later` | `| policy: <level>` |
| `PolicyPickerActive`（新） | `↑↓ select | Enter confirm | Esc cancel` | `| policy: <level>` |

### 3.2 颜色

`footer_text()` 返回类型从 `String` 改为 `Line<'static>`，policy 部分单独着色：

| 层级 | 颜色 |
|------|------|
| `permissive` | `Color::Cyan` |
| `standard` | `Color::White` |
| `strict` | `Color::Yellow` |

未知/None 时显示 `DarkGray`。

### 3.3 数据来源

footer 直接读 `AppState.policy_default: Option<String>`，切换后该字段更新，下一帧自动重绘，无需额外信号。

---

## 4. 切换交互设计

### 4.1 两步式交互

**第一步**：用户输入 `/policy`，slash palette 正常展示该命令作为候选，Enter 触发命令执行。`submit_slash_command_if_exact`（在 `slash_palette.rs` 中）扩展以处理 `SlashCommandKind::LocalPicker`：当命中 `LocalPicker` 类型命令时，调用 `self.open_policy_picker()` 并返回 `true`（消费输入），不进入后续 `find_exact` 分支。

**注意**：`/policy` 命令在存在 `state.pending_approval` 时不可激活（`open_policy_picker` 在 `pending_approval.is_some()` 时直接返回，不弹出 picker），避免与审批面板冲突。`policy_picker` 与 `pending_approval` 不同时激活。

**第二步**：触发后弹出 `PolicyPicker` overlay（新 `TransientKind::PolicyPicker`）：

```
Select policy level:

› permissive   宽松 - 自动通过大多数操作
  standard     标准 - 操作需审批（当前）
  strict       严格 - 拒绝未显式允许的操作
```

- 默认高亮当前生效层级（由 `AppState.policy_default` 确定）
- `↑↓` 移动选中项，循环
- `Enter` 确认：调 `agent.set_policy_level(level)` → 调 `app.apply_policy_pick(level.as_str())` → footer 指示器刷新
- `Esc` 取消：关闭 picker，不变更

### 4.2 Picker 状态

`PolicyPickerState` 定义在 `ui/tui/src/state/app_state.rs`（与 `PendingApproval`、`SlashPaletteState` 等保持一致）：

```rust
pub struct PolicyPickerState {
    pub selected_index: usize, // 0=permissive, 1=standard, 2=strict
}
```

`AppState` 新增字段 `policy_picker: Option<PolicyPickerState>`，`Default` 实现（显式 impl，非 derive）中加入 `policy_picker: None`。
`state/mod.rs` 导出 `PolicyPickerState`（与同文件其他状态类型一致）。

---

## 5. 模块改动说明

### 5.1 openjax-core

**新增 `policy_level.rs`**（`openjax-core/src/agent/policy_level.rs`，不放在 `runtime_policy.rs` 中）：

```rust
pub enum PolicyLevel { Permissive, Standard, Strict }

impl PolicyLevel {
    pub fn as_str(&self) -> &'static str { ... }
    pub fn to_decision_kind(&self) -> openjax_policy::DecisionKind { ... }
    pub fn from_str(s: &str) -> Option<Self> { ... }
}
```

`openjax_policy::DecisionKind` 已由 `openjax-policy` crate 导出，`openjax-core` 已依赖该 crate，无新依赖引入。

**`Agent::set_policy_level(&mut self, level: PolicyLevel)`**（`agent/bootstrap.rs` 或 `agent/mod.rs`）：

- 此方法**不返回 `Result`，是 infallible 的**。
- 无论 `self.policy_runtime` 是 `None` 还是 `Some`，均调用 `PolicyStore::new(level.to_decision_kind())` 创建新的 `PolicyStore`（`PolicyStore::new` 自动注入 `system:destructive_escalate` 等内置规则）。
- **有意不保留现有 store 中的非内置用户规则**（简化实现，session overlay 由 `PolicyRuntime::publish` 自动保留——`publish` 内部 clone overlays）。
- 最终调用 `self.set_policy_runtime(Some(new_runtime))`。
- `policy_default_decision_name()` 已存在，在 `set_policy_level` 后能反映新值。

**`SlashCommandKind` 新增 `LocalPicker` 变体**（`openjax-core/src/slash_commands/kinds.rs` 或等效文件）：

```rust
pub enum SlashCommandKind {
    // 已有...
    LocalPicker, // 新增：触发 TUI 本地 picker overlay，不经过 agent 或 gateway
}
```

新增辅助方法 `local_picker_name() -> Option<&'static str>`（参照现有 `session_action_name()` 模式）。

**slash command 注册**：注册 `/policy` 命令，`kind = SlashCommandKind::LocalPicker`，description 说明其用途。

### 5.2 ui/tui

**`state/app_state.rs`**：
- 新增 `PolicyPickerState { selected_index: usize }`
- `AppState` 新增 `policy_picker: Option<PolicyPickerState>`，显式 `Default` impl 中加 `policy_picker: None`

**`state/mod.rs`**：
- 导出 `PolicyPickerState`

**`slash_palette.rs`（`app/slash_palette.rs`）**：
- `submit_slash_command_if_exact` 扩展：在现有 `Builtin` 分支之后，增加 `LocalPicker` 分支 → 调 `self.open_policy_picker()` → 返回 `true`

**`app/mod.rs`**：
- 新增 `open_policy_picker(&mut self)` — 若 `state.pending_approval.is_some()` 则直接返回（不冲突）；否则根据 `state.policy_default` 计算 `selected_index`（0/1/2），写入 `state.policy_picker`；同时 dismiss slash palette
- 新增 `move_policy_selection(&mut self, delta: i8)` — 循环更新 `state.policy_picker.selected_index`（rem_euclid(3)）
- 新增 `dismiss_policy_picker(&mut self)` — 清除 `state.policy_picker`
- 新增 `apply_policy_pick(&mut self, level_str: &str)` — 同步更新 `state.policy_default`，清除 `state.policy_picker`（不访问 agent，纯状态更新，由 `runtime.rs` 在调 agent 成功后调用）

**`app/render_model.rs`**：
- `footer_text()` 重命名为 `footer_line()`，返回 `Line<'static>`：左侧 hint 保持 `DarkGray Bold`，末尾追加 `| policy: <level>` span，policy 部分按层级着色
- 新增 `policy_picker_lines() -> Option<Vec<Line<'static>>>`，构建选项行，高亮 `selected_index` 对应行
- 新增 `policy_picker_height() -> u16`

**`app/layout_metrics.rs`**：
- `TransientKind` 新增 `PolicyPicker` 变体
- `FooterMode` 新增 `PolicyPickerActive` 变体
- `bottom_layout()` 在 `state.policy_picker.is_some()` 时返回 `PolicyPickerActive` footer mode 和 `PolicyPicker` transient kind，高度由 `policy_picker_height()` 决定

**`tui.rs`**：
- `DrawRequest.footer_text: String` → `footer_line: Line<'static>`
- 渲染处直接使用 `Line`（移除旧的 `Span::styled(footer_text, ...)` 包装）

**`runtime_loop.rs`**（唯一构造 `DrawRequest` 的调用处）：
- `render_once` 中将 `footer_text: app.footer_text()` 替换为 `footer_line: app.footer_line()`

**`runtime.rs`**：
- `InputAction::MoveUp` / `MoveDown`：**先**判断 `app.state.policy_picker.is_some()` → 调 `app.move_policy_selection`；其次才判断 `pending_approval`（两者互斥，不会同时激活，但顺序保持防御性）
- `InputAction::Submit`：`policy_picker` 激活时，读取 `selected_index` → 映射到 `PolicyLevel` → `agent.lock().await.set_policy_level(level)`（infallible）→ 调 `app.apply_policy_pick(level.as_str())`
- `InputAction::DismissOverlay`：在现有 slash palette / approval 分支之前，先判断 `policy_picker` 激活时调 `app.dismiss_policy_picker()`

### 5.3 openjax-gateway

**新增路由**：`PUT /api/v1/sessions/:session_id/policy`（在 `lib.rs` 路由注册处注册）

**作用域说明**：此接口仅修改指定会话 agent 的本地 `PolicyRuntime`（session-local，via `agent.set_policy_level()`），不影响全局策略状态和其他会话，与现有全局规则路由及 overlay 路由完全独立。

**请求体**：
```json
{ "level": "permissive" | "standard" | "strict" }
```

**响应**：
- `200 OK`：`{"level": "<effective_level>"}`
- `400 Bad Request`：level 非法，`{"code": "invalid_policy_level", "message": "..."}`

**Handler 实现**（`handlers/session.rs`）：
- 解析 `level` → `PolicyLevel::from_str` → 非法返回 400
- 从 session map 取 `Arc<Mutex<Agent>>`（先 lock session map，取出 Arc 后释放 session map lock，再 lock agent），避免与其他 handler 的 lock 顺序冲突
- 调 `agent.set_policy_level(level)` → 返回 200

---

## 6. 测试要求

### 6.1 openjax-core 单元测试（`#[cfg(test)]` 块，`agent/policy_level.rs` 内）

- `PolicyLevel::from_str` 对 `"permissive"/"standard"/"strict"` 返回正确变体，非法值返回 `None`
- `PolicyLevel::to_decision_kind()` 三种变体映射正确
- `Agent::set_policy_level` 在 `policy_runtime = None` 时创建新 runtime，`policy_default_decision_name()` 反映新值
- `Agent::set_policy_level` 在已有 `policy_runtime` 时更新 default_decision，`policy_default_decision_name()` 反映新值
- **关键**：`set_policy_level(Strict)` 后对 destructive 命令决策仍为 `Escalate`（`system:destructive_escalate` 内置规则被 `PolicyStore::new` 注入，不受 Deny default 覆盖）

### 6.2 ui/tui 集成测试

**新增 `tests/m23_policy_picker_behavior.rs`**，覆盖：
- `open_policy_picker()` 在 `pending_approval = None` 时写入 `state.policy_picker`，`selected_index` 对应当前层级
- `open_policy_picker()` 在 `pending_approval = Some(...)` 时不改变 `state.policy_picker`（互斥保护）
- 方向键导航正确更新 `selected_index`（含循环边界：从 0 上移回到 2，从 2 下移回到 0）
- `apply_policy_pick("permissive")` 后 `state.policy_default == Some("permissive")`，`state.policy_picker == None`
- `dismiss_policy_picker()` 后 `state.policy_picker == None`，`state.policy_default` 不变
- `footer_line()` 在三种层级下末尾 span 包含正确文本（`policy: permissive` / `policy: standard` / `policy: strict`）
- `footer_line()` 在三种层级下 policy span 颜色正确（Cyan/White/Yellow）
- `policy_picker_lines()` 在 picker 激活时高亮正确选中项
- `footer_mode` 在 picker 激活时为 `PolicyPickerActive`，非激活时为 `Idle`

**更新 `tests/m10_approval_panel_navigation.rs`**：
- 确认 approval panel 的 `MoveUp`/`MoveDown` 逻辑在 `policy_picker = None` 时不受影响（回归）

**更新 `tests/m7_startup_banner_once.rs`**：
- 确认 `set_runtime_info` + banner 流程在新 `policy_picker: None` 默认值下仍正常（回归）

### 6.3 openjax-gateway 集成测试（`tests/gateway_api.rs`）

- `PUT /api/v1/sessions/:id/policy` 合法请求（permissive/standard/strict）返回 200，响应体 level 正确
- 非法 level 值返回 400

---

## 7. 不在本次范围内

- policy 变更持久化到配置文件
- web UI 侧的 policy 切换控件（gateway API 口子留好，UI 后续再做）
- session overlay 规则管理（更精细的规则 CRUD）
- banner 加 policy 行（footer 已足够，不重复）
- 全局策略影响多会话的场景（本次仅 session-local）

---

## 8. 实现顺序

1. **openjax-core**：`policy_level.rs`（`PolicyLevel` 枚举）→ `Agent::set_policy_level()` → `SlashCommandKind::LocalPicker` 变体及 `local_picker_name()` → slash command 注册 → 单元测试
2. **ui/tui**：`state/app_state.rs`（`PolicyPickerState`、`AppState` 字段）→ `state/mod.rs`（导出）→ `slash_palette.rs`（`LocalPicker` 分支）→ `app/mod.rs`（picker 逻辑）→ `layout_metrics.rs`（新 kind/mode）→ `render_model.rs`（`footer_line`、picker 渲染）→ `tui.rs` + `runtime_loop.rs`（`DrawRequest` 类型变更）→ `runtime.rs`（输入分发）→ 集成测试
3. **openjax-gateway**：新路由 + handler + 集成测试
