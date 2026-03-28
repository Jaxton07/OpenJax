# Policy Level Display & Switching in TUI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Also remember:** You can use the `rustrover-index` MCP server for symbol lookups, references, and type hierarchy queries in Rust files.

**Goal:** 在 TUI footer 常驻显示当前 policy 层级（permissive/standard/strict），并通过 `/policy` slash 命令 + picker overlay 切换层级；gateway 侧暴露 API 口子。

**Architecture:**
- openjax-core：新增 `PolicyLevel` 枚举和 `Agent::set_policy_level()`，新增 `SlashCommandKind::LocalPicker` 变体；`/policy` 注册为 `LocalPicker` 类型命令
- ui/tui：`PolicyPickerState` 加入 `AppState`；`submit_slash_command_if_exact` 处理 `LocalPicker`；`footer_text()` → `footer_line()` 返回 `Line<'static>` 含 policy 着色；picker overlay 复用 approval panel 的 transient 模式
- openjax-gateway：`PUT /api/v1/sessions/:id/policy` session-local 层级切换接口

**Tech Stack:** Rust 2024 edition, ratatui, tokio, openjax-policy, axum

---

## File Map

| 文件 | 操作 | 职责 |
|------|------|------|
| `openjax-core/src/agent/policy_level.rs` | 新建 | `PolicyLevel` 枚举 + 转换方法 |
| `openjax-core/src/agent/bootstrap.rs` | 修改 | `Agent::set_policy_level()` |
| `openjax-core/src/agent/mod.rs` | 修改 | 导出 `policy_level` 模块 |
| `openjax-core/src/slash_commands/kinds.rs` | 修改 | 新增 `LocalPicker` 变体 + `local_picker_name()` |
| `openjax-core/src/slash_commands/registry.rs` | 修改 | 注册 `/policy` 命令 |
| `ui/tui/src/state/app_state.rs` | 修改 | 新增 `PolicyPickerState`，`AppState.policy_picker` |
| `ui/tui/src/state/mod.rs` | 修改 | 导出 `PolicyPickerState` |
| `ui/tui/src/app/slash_palette.rs` | 修改 | `submit_slash_command_if_exact` 处理 `LocalPicker` |
| `ui/tui/src/app/mod.rs` | 修改 | picker 开关/导航/确认/取消逻辑 |
| `ui/tui/src/app/layout_metrics.rs` | 修改 | `TransientKind::PolicyPicker`, `FooterMode::PolicyPickerActive` |
| `ui/tui/src/app/render_model.rs` | 修改 | `footer_line()`, `policy_picker_lines()` |
| `ui/tui/src/tui.rs` | 修改 | `DrawRequest.footer_line: Line<'static>` |
| `ui/tui/src/runtime_loop.rs` | 修改 | 调用 `footer_line()` |
| `ui/tui/src/runtime.rs` | 修改 | MoveUp/Down/Submit/DismissOverlay 分发 |
| `ui/tui/tests/m23_policy_picker_behavior.rs` | 新建 | picker 集成测试 |
| `openjax-gateway/src/handlers/session.rs` | 修改 | 新增 policy 层级切换 handler |
| `openjax-gateway/src/lib.rs` | 修改 | 注册新路由 |
| `openjax-gateway/tests/gateway_api.rs` | 修改 | gateway 集成测试 |

---

## Task 1: `PolicyLevel` 枚举（openjax-core）

**Files:**
- Create: `openjax-core/src/agent/policy_level.rs`
- Modify: `openjax-core/src/agent/mod.rs`

- [ ] **Step 1: 写 policy_level.rs**

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
            PolicyLevel::Permissive => "permissive",
            PolicyLevel::Standard => "standard",
            PolicyLevel::Strict => "strict",
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
            "permissive" => Some(PolicyLevel::Permissive),
            "standard" => Some(PolicyLevel::Standard),
            "strict" => Some(PolicyLevel::Strict),
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
        assert!(PolicyLevel::from_str("ask").is_none());
    }

    #[test]
    fn to_decision_kind_maps_correctly() {
        use openjax_policy::schema::DecisionKind;
        assert_eq!(PolicyLevel::Permissive.to_decision_kind(), DecisionKind::Allow);
        assert_eq!(PolicyLevel::Standard.to_decision_kind(), DecisionKind::Ask);
        assert_eq!(PolicyLevel::Strict.to_decision_kind(), DecisionKind::Deny);
    }
}
```

- [ ] **Step 2: 在 `agent/mod.rs` 中导出 policy_level 模块**

在 `openjax-core/src/agent/mod.rs` 文件的 mod 声明区添加：

```rust
pub mod policy_level;
pub use policy_level::PolicyLevel;
```

- [ ] **Step 3: 运行单元测试确认通过**

```bash
zsh -lc "cargo test -p openjax-core agent::policy_level"
```

Expected: 3 tests pass

- [ ] **Step 4: 提交**

```bash
git add openjax-core/src/agent/policy_level.rs openjax-core/src/agent/mod.rs
git commit -m "feat(core): 新增 PolicyLevel 枚举（permissive/standard/strict）"
```

---

## Task 2: `Agent::set_policy_level()`（openjax-core）

**Files:**
- Modify: `openjax-core/src/agent/bootstrap.rs`

- [ ] **Step 1: 在 `bootstrap.rs` 中实现 `set_policy_level`**

在已有 `set_policy_runtime` 方法之后添加：

```rust
/// 切换当前会话的策略层级。
/// - 无论是否已有 policy_runtime，均以新 default_decision 构造 PolicyStore（含内置 system:destructive_escalate 规则）
/// - 已有 runtime 时通过 publish 保留 session overlay；无 runtime 时新建
/// - 此方法 infallible
pub fn set_policy_level(&mut self, level: crate::agent::PolicyLevel) {
    use openjax_policy::{runtime::PolicyRuntime, store::PolicyStore};
    let kind = level.to_decision_kind();
    let store = PolicyStore::new(kind, vec![]);
    match self.policy_runtime.as_ref() {
        Some(runtime) => {
            runtime.publish(store);
        }
        None => {
            self.policy_runtime = Some(PolicyRuntime::new(store));
        }
    }
}
```

- [ ] **Step 2: 写单元测试（`bootstrap.rs` 的 `#[cfg(test)]` 块，或新建 `tests/policy_level_suite.rs`）**

推荐内联到 `bootstrap.rs` 底部：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::PolicyLevel;
    use openjax_policy::schema::DecisionKind;

    #[test]
    fn set_policy_level_from_none_creates_runtime() {
        let mut agent = Agent::with_config(crate::Config::default());
        assert_eq!(agent.policy_default_decision_name(), "ask"); // fallback
        agent.set_policy_level(PolicyLevel::Permissive);
        assert_eq!(agent.policy_default_decision_name(), "allow");
    }

    #[test]
    fn set_policy_level_from_existing_runtime_updates_default() {
        use openjax_policy::{runtime::PolicyRuntime, store::PolicyStore};
        let mut agent = Agent::with_config(crate::Config::default());
        agent.set_policy_runtime(Some(PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]))));
        agent.set_policy_level(PolicyLevel::Strict);
        assert_eq!(agent.policy_default_decision_name(), "deny");
    }

    #[test]
    fn set_policy_level_strict_still_escalates_destructive() {
        use openjax_policy::{schema::PolicyInput, store::PolicyStore, runtime::PolicyRuntime};
        let mut agent = Agent::with_config(crate::Config::default());
        agent.set_policy_level(PolicyLevel::Strict);
        let runtime = agent.policy_runtime.as_ref().unwrap();
        let input = PolicyInput {
            tool_name: "shell".to_string(),
            action: "exec".to_string(),
            session_id: None,
            actor: None,
            resource: None,
            capabilities: vec![],
            risk_tags: vec!["destructive".to_string()],
            policy_version: 0,
        };
        let decision = runtime.handle().decide(&input);
        // system:destructive_escalate (priority=1000) beats Deny default
        assert_eq!(decision.kind, openjax_policy::schema::DecisionKind::Escalate);
    }
}
```

- [ ] **Step 3: 运行测试**

```bash
zsh -lc "cargo test -p openjax-core -- set_policy_level"
```

Expected: 3 tests pass

- [ ] **Step 4: 提交**

```bash
git add openjax-core/src/agent/bootstrap.rs
git commit -m "feat(core): Agent::set_policy_level() — 切换 policy 层级，保留 overlay，内置规则不受影响"
```

---

## Task 3: `SlashCommandKind::LocalPicker` + `/policy` 注册（openjax-core）

**Files:**
- Modify: `openjax-core/src/slash_commands/kinds.rs`
- Modify: `openjax-core/src/slash_commands/registry.rs`
- Modify: `openjax-gateway/src/handlers/slash_commands.rs`（gateway 的 match arm 需覆盖新变体）

- [ ] **Step 1: 在 `kinds.rs` 添加 `LocalPicker` 变体及方法**

在 `SlashCommandKind` 枚举末尾添加变体：

```rust
/// TUI 本地 picker overlay，不经过 agent 或 gateway；由 TUI 的 submit_slash_command_if_exact 处理
LocalPicker {
    name: &'static str,
},
```

在 `impl SlashCommandKind` 中补充：
- `execute()` match arm（`LocalPicker` 返回 `SlashResult::Pending`）
- `needs_agent()` match arm（`LocalPicker` 返回 `false`）
- `session_action_name()` match arm（`LocalPicker` 返回 `None`）
- `replaces_input()` match arm（`LocalPicker` 返回 `false`）
- 新方法：

```rust
pub fn local_picker_name(&self) -> Option<&'static str> {
    match self {
        SlashCommandKind::LocalPicker { name } => Some(name),
        _ => None,
    }
}
```

- [ ] **Step 2: 在 `registry.rs` 注册 `/policy` 命令**

在 `builtin_commands()` 的 vec 末尾添加：

```rust
SlashCommand {
    name: "policy",
    aliases: &[],
    description: "Switch policy level (permissive / standard / strict)",
    usage_hint: "/policy",
    kind: SlashCommandKind::LocalPicker { name: "policy" },
},
```

同时在 `conflicts_with_builtin_or_alias` 的逻辑中，`builtin_commands()` 已包含 `/policy`，自动防止 skill 注册同名命令。

- [ ] **Step 3: 修复 gateway 的 slash_commands.rs 的穷举 match**

`openjax-gateway/src/handlers/slash_commands.rs` 中有多处 `match kind` 对 `SlashCommandKind` 的穷举，需加 `LocalPicker` 的 arm（返回 `"local_picker"` 字符串或跳过），避免编译报错。

- [ ] **Step 4: 构建验证无编译错误**

```bash
zsh -lc "cargo build -p openjax-core -p openjax-gateway"
```

Expected: 编译通过，无 warnings

- [ ] **Step 5: 运行 slash command 测试**

```bash
zsh -lc "cargo test -p openjax-core -- slash_commands"
```

Expected: 所有已有测试通过，`/policy` 可被 `find("policy")` 找到

- [ ] **Step 6: 提交**

```bash
git add openjax-core/src/slash_commands/kinds.rs \
        openjax-core/src/slash_commands/registry.rs \
        openjax-gateway/src/handlers/slash_commands.rs
git commit -m "feat(core): SlashCommandKind::LocalPicker + /policy 命令注册"
```

---

## Task 4: TUI state — `PolicyPickerState` 和 `AppState` 扩展

**Files:**
- Modify: `ui/tui/src/state/app_state.rs`
- Modify: `ui/tui/src/state/mod.rs`

- [ ] **Step 1: 在 `app_state.rs` 添加 `PolicyPickerState` 和 `AppState` 字段**

在 `PendingApproval` 结构体之后添加：

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyPickerState {
    pub selected_index: usize, // 0=permissive, 1=standard, 2=strict
}
```

在 `AppState` 结构体中，在 `approval_selection` 字段之后添加：

```rust
pub policy_picker: Option<PolicyPickerState>,
```

在 `Default` impl 的 `Self { ... }` 块中，在 `approval_selection` 初始值之后添加：

```rust
policy_picker: None,
```

- [ ] **Step 2: 在 `state/mod.rs` 导出 `PolicyPickerState`**

在已有 `pub use app_state::{...}` 行中，加入 `PolicyPickerState`。

- [ ] **Step 3: 构建验证**

```bash
zsh -lc "cargo build -p tui_next"
```

Expected: 编译通过

- [ ] **Step 4: 提交**

```bash
git add ui/tui/src/state/app_state.rs ui/tui/src/state/mod.rs
git commit -m "feat(tui): 新增 PolicyPickerState 和 AppState.policy_picker 字段"
```

---

## Task 5: TUI — `submit_slash_command_if_exact` 处理 `LocalPicker`

**Files:**
- Modify: `ui/tui/src/app/slash_palette.rs`
- Modify: `ui/tui/src/app/mod.rs`（新增 `open_policy_picker` 等方法）

- [ ] **Step 1: 在 `app/mod.rs` 新增 picker 操作方法**

在已有 `dismiss_slash_palette` 等方法之后添加以下方法（均为同步）：

```rust
/// 打开 policy picker。若 pending_approval 存在则忽略（互斥）。
pub fn open_policy_picker(&mut self) {
    if self.state.pending_approval.is_some() {
        return;
    }
    let current = self.state.policy_default.as_deref().unwrap_or("standard");
    let selected_index = match current {
        "permissive" | "allow" => 0,
        "strict" | "deny" => 2,
        _ => 1, // standard / ask / 其他均默认 standard
    };
    self.state.policy_picker = Some(crate::state::PolicyPickerState { selected_index });
    self.dismiss_slash_palette();
    self.state.input.clear();
    self.state.input_cursor = 0;
}

/// 循环移动 policy picker 选中项（delta = ±1）
pub fn move_policy_selection(&mut self, delta: i8) {
    let Some(picker) = self.state.policy_picker.as_mut() else { return };
    let next = (picker.selected_index as i8 + delta).rem_euclid(3) as usize;
    picker.selected_index = next;
}

/// 确认 picker 选择（仅更新本地状态；agent 调用在 runtime.rs 侧）
pub fn apply_policy_pick(&mut self, level_str: &str) {
    self.state.policy_default = Some(level_str.to_string());
    self.state.policy_picker = None;
}

/// 取消 picker，不变更 policy
pub fn dismiss_policy_picker(&mut self) {
    self.state.policy_picker = None;
}
```

- [ ] **Step 2: 在 `slash_palette.rs` 的 `submit_slash_command_if_exact` 中添加 `LocalPicker` 分支**

在已有 match arm `SlashCommandKind::SessionAction { .. } | SlashCommandKind::Skill { .. } => { false }` 之前，插入：

```rust
SlashCommandKind::LocalPicker { .. } => {
    self.open_policy_picker();
    true
}
```

- [ ] **Step 3: 构建验证**

```bash
zsh -lc "cargo build -p tui_next"
```

Expected: 编译通过

- [ ] **Step 4: 手动验证路径逻辑（运行已有 slash palette 测试）**

```bash
zsh -lc "cargo test -p tui_next -- slash_palette"
```

Expected: 所有已有测试通过

- [ ] **Step 5: 提交**

```bash
git add ui/tui/src/app/mod.rs ui/tui/src/app/slash_palette.rs
git commit -m "feat(tui): open_policy_picker + submit_slash_command_if_exact 处理 LocalPicker"
```

---

## Task 6: TUI layout — `TransientKind::PolicyPicker` 和 `FooterMode::PolicyPickerActive`

**Files:**
- Modify: `ui/tui/src/app/layout_metrics.rs`

- [ ] **Step 1: 在 `TransientKind` 和 `FooterMode` 枚举添加变体**

```rust
pub enum TransientKind {
    None,
    Slash,
    Approval,
    PolicyPicker, // 新增
}

pub enum FooterMode {
    Idle,
    SlashActive,
    ApprovalActive,
    PolicyPickerActive, // 新增
}
```

- [ ] **Step 2: 更新 `bottom_layout()` 逻辑**

在方法开头，在 `approval_rows` 和 `slash_rows` 计算之后，加 picker_rows 计算：

```rust
let picker_rows = if approval_rows > 0 || slash_rows > 0 {
    0
} else {
    // picker height: 1 header + 1 blank + 3 options = 5 行
    if self.state.policy_picker.is_some() { 5 } else { 0 }
};
```

在 `(transient_kind, transient_rows, footer_mode)` 的 if/else 链中，在 `approval_rows` 分支之后、`slash_rows` 分支之前，插入：

```rust
} else if picker_rows > 0 {
    (TransientKind::PolicyPicker, picker_rows, FooterMode::PolicyPickerActive)
```

- [ ] **Step 3: 构建验证**

```bash
zsh -lc "cargo build -p tui_next"
```

Expected: 编译通过，穷举 match 对 `TransientKind`/`FooterMode` 的地方可能需要补 arm（编译器会提示）

- [ ] **Step 4: 提交**

```bash
git add ui/tui/src/app/layout_metrics.rs
git commit -m "feat(tui): TransientKind::PolicyPicker + FooterMode::PolicyPickerActive"
```

---

## Task 7: TUI render — `footer_line()` 和 `policy_picker_lines()`

**Files:**
- Modify: `ui/tui/src/app/render_model.rs`

- [ ] **Step 1: 重命名 `footer_text()` → `footer_line()`，返回 `Line<'static>`，追加 policy span**

删除现有 `footer_text()` 方法，替换为：

```rust
pub fn footer_line(&self) -> Line<'static> {
    let hint = match self.bottom_layout(0).footer_mode {
        FooterMode::Idle => "Enter submit | / commands | Esc clear | Ctrl-C quit",
        FooterMode::SlashActive => "Tab/Enter complete | Esc dismiss",
        FooterMode::ApprovalActive => "↑↓ select | Enter confirm | Esc later",
        FooterMode::PolicyPickerActive => "↑↓ select | Enter confirm | Esc cancel",
    };
    let (policy_label, policy_color) = policy_level_display(
        self.state.policy_default.as_deref(),
    );
    Line::from(vec![
        Span::styled(hint, Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" | policy: {}", policy_label),
            Style::default().fg(policy_color).add_modifier(Modifier::BOLD),
        ),
    ])
}
```

在文件末尾（模块级）添加辅助函数：

```rust
fn policy_level_display(level: Option<&str>) -> (&'static str, Color) {
    match level {
        Some("permissive") | Some("allow") => ("permissive", Color::Cyan),
        Some("strict") | Some("deny") => ("strict", Color::Yellow),
        _ => ("standard", Color::White), // ask / standard / None 均显示 standard
    }
}
```

- [ ] **Step 2: 新增 `policy_picker_lines()` 和 `policy_picker_height()`**

```rust
pub fn policy_picker_lines(&self) -> Option<Vec<Line<'static>>> {
    let picker = self.state.policy_picker.as_ref()?;
    let options: [(&str, &str); 3] = [
        ("permissive", "宽松 - 自动通过大多数操作"),
        ("standard",   "标准 - 操作需审批（当前默认）"),
        ("strict",     "严格 - 拒绝未显式允许的操作"),
    ];
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "Select policy level:",
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::default());
    for (idx, (name, desc)) in options.iter().enumerate() {
        let selected = idx == picker.selected_index;
        let marker = if selected { "› " } else { "  " };
        let style = if selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(format!("{:<12}", name), style),
            Span::styled(desc.to_string(), Style::default().fg(Color::DarkGray)),
        ]));
    }
    Some(lines)
}

pub fn policy_picker_height(&self) -> u16 {
    self.policy_picker_lines()
        .map(|lines| lines.len() as u16)
        .unwrap_or(0)
}
```

- [ ] **Step 3: 在 `transient_panel()` 添加 `PolicyPicker` arm**

在已有 `TransientKind::Approval` / `TransientKind::Slash` / `TransientKind::None` arm 之前加：

```rust
TransientKind::PolicyPicker => self.policy_picker_lines().map(|lines| {
    TransientPanel {
        kind: TransientKind::PolicyPicker,
        lines,
        selected_index: self.state.policy_picker.as_ref().map(|p| p.selected_index + 2), // +2 offset for header lines
    }
}),
```

注：`selected_index` 是 picker 高亮行在 `lines` vec 中的实际索引（header=0, blank=1, options=2/3/4）。

- [ ] **Step 4: 更新 layout_metrics.rs 的 `bottom_layout()` 使用 `policy_picker_height()`**

将 Task 6 中硬编码的 `5` 改为：

```rust
if self.state.policy_picker.is_some() { self.policy_picker_height() } else { 0 }
```

- [ ] **Step 5: 构建验证**

```bash
zsh -lc "cargo build -p tui_next"
```

Expected: 编译通过

- [ ] **Step 6: 提交**

```bash
git add ui/tui/src/app/render_model.rs ui/tui/src/app/layout_metrics.rs
git commit -m "feat(tui): footer_line() + policy_picker_lines() 渲染"
```

---

## Task 8: TUI — `DrawRequest` 类型变更

**Files:**
- Modify: `ui/tui/src/tui.rs`
- Modify: `ui/tui/src/runtime_loop.rs`

- [ ] **Step 1: 修改 `tui.rs` 的 `DrawRequest` 字段**

将：
```rust
pub footer_text: String,
```
改为：
```rust
pub footer_line: Line<'static>,
```

- [ ] **Step 2: 修改 `tui.rs` 的 `draw()` 渲染逻辑**

在 `draw()` 方法的 destructuring 中，将 `footer_text` 改为 `footer_line`。

将渲染 footer 的代码从：
```rust
let footer = Paragraph::new(Line::from(vec![Span::styled(
    footer_text,
    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
)]));
```
改为：
```rust
let footer = Paragraph::new(footer_line);
```

- [ ] **Step 3: 修改 `runtime_loop.rs` 的 `render_once()`**

将：
```rust
footer_text: app.footer_text(),
```
改为：
```rust
footer_line: app.footer_line(),
```

- [ ] **Step 4: 构建验证**

```bash
zsh -lc "cargo build -p tui_next"
```

Expected: 编译通过，无 `footer_text` 相关错误

- [ ] **Step 5: 提交**

```bash
git add ui/tui/src/tui.rs ui/tui/src/runtime_loop.rs
git commit -m "refactor(tui): DrawRequest.footer_text → footer_line: Line<'static>"
```

---

## Task 9: TUI runtime — 输入分发

**Files:**
- Modify: `ui/tui/src/runtime.rs`

- [ ] **Step 1: 在 `InputAction::MoveUp` / `MoveDown` 分支最前面加 policy picker 优先判断**

当前代码（`runtime.rs` 约 105-121 行）：
```rust
InputAction::MoveUp => {
    if app.state.pending_approval.is_some() {
        app.move_approval_selection(-1);
    } else if app.is_slash_palette_active() {
        app.move_slash_selection(-1);
    } else {
        app.history_prev();
    }
}
```

改为：
```rust
InputAction::MoveUp => {
    if app.state.policy_picker.is_some() {
        app.move_policy_selection(-1);
    } else if app.state.pending_approval.is_some() {
        app.move_approval_selection(-1);
    } else if app.is_slash_palette_active() {
        app.move_slash_selection(-1);
    } else {
        app.history_prev();
    }
}
```

同样处理 `MoveDown`（delta = 1）。

- [ ] **Step 2: 在 `InputAction::Submit` 分支中处理 policy picker 确认**

在 `handle_submit_action` 调用之前（或在 `submit_input()` 调用之前），添加：

```rust
// policy picker 确认（需 agent lock）
if app.state.policy_picker.is_some() {
    let idx = app.state.policy_picker.as_ref().unwrap().selected_index;
    let levels = ["permissive", "standard", "strict"];
    if let Some(level_str) = levels.get(idx) {
        if let Some(level) = openjax_core::PolicyLevel::from_str(level_str) {
            agent.lock().await.set_policy_level(level);
        }
        app.apply_policy_pick(level_str);
    }
    continue;
}
```

将此块放在现有 `if turn_task.is_some() && app.state.pending_approval.is_none()` 检查之前，确保 picker 确认不受 busy 状态阻断。

- [ ] **Step 3: 在 `InputAction::DismissOverlay` 分支最前面加 policy picker 取消**

```rust
InputAction::DismissOverlay => {
    if app.state.policy_picker.is_some() {
        app.dismiss_policy_picker();
    } else if app.is_slash_palette_active() {
        app.dismiss_slash_palette();
    } else if app.state.pending_approval.is_none() {
        app.clear();
    }
}
```

- [ ] **Step 4: 构建验证**

```bash
zsh -lc "cargo build -p tui_next"
```

Expected: 编译通过

- [ ] **Step 5: 提交**

```bash
git add ui/tui/src/runtime.rs
git commit -m "feat(tui): runtime 分发 policy picker 的 MoveUp/Down、Submit、DismissOverlay"
```

---

## Task 10: TUI 集成测试

**Files:**
- Create: `ui/tui/tests/m23_policy_picker_behavior.rs`
- Modify: `ui/tui/tests/m10_approval_panel_navigation.rs`（可能需要回归验证）
- Modify: `ui/tui/tests/m7_startup_banner_once.rs`（可能需要回归验证）

- [ ] **Step 1: 新建 `tests/m23_policy_picker_behavior.rs`**

```rust
use tui_next::app::App;
use tui_next::state::{PendingApproval, PolicyPickerState};
use std::time::Instant;

fn make_pending_approval() -> PendingApproval {
    PendingApproval {
        request_id: "r1".to_string(),
        target: "some tool".to_string(),
        reason: "needs approval".to_string(),
        tool_name: None,
        command_preview: None,
        risk_tags: vec![],
        sandbox_backend: None,
        degrade_reason: None,
        requested_at: Instant::now(),
        timeout_ms: 300_000,
    }
}

#[test]
fn open_policy_picker_with_no_pending_approval() {
    let mut app = App::default();
    app.state.policy_default = Some("standard".to_string());
    app.open_policy_picker();
    let picker = app.state.policy_picker.as_ref().expect("picker should open");
    assert_eq!(picker.selected_index, 1, "standard maps to index 1");
}

#[test]
fn open_policy_picker_blocked_by_pending_approval() {
    let mut app = App::default();
    app.state.pending_approval = Some(make_pending_approval());
    app.open_policy_picker();
    assert!(app.state.policy_picker.is_none(), "picker must not open during approval");
}

#[test]
fn move_policy_selection_wraps() {
    let mut app = App::default();
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 0 });
    app.move_policy_selection(-1);
    assert_eq!(app.state.policy_picker.as_ref().unwrap().selected_index, 2, "0 - 1 wraps to 2");
    app.move_policy_selection(1);
    assert_eq!(app.state.policy_picker.as_ref().unwrap().selected_index, 0, "2 + 1 wraps to 0");
}

#[test]
fn apply_policy_pick_updates_policy_default_and_clears_picker() {
    let mut app = App::default();
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 0 });
    app.apply_policy_pick("permissive");
    assert_eq!(app.state.policy_default, Some("permissive".to_string()));
    assert!(app.state.policy_picker.is_none());
}

#[test]
fn dismiss_policy_picker_clears_picker_without_changing_policy() {
    let mut app = App::default();
    app.state.policy_default = Some("standard".to_string());
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 2 });
    app.dismiss_policy_picker();
    assert!(app.state.policy_picker.is_none());
    assert_eq!(app.state.policy_default, Some("standard".to_string()));
}

#[test]
fn footer_line_contains_correct_policy_label() {
    let mut app = App::default();
    for (input, expected) in [
        ("permissive", "permissive"),
        ("allow",      "permissive"),
        ("standard",   "standard"),
        ("ask",        "standard"),
        ("strict",     "strict"),
        ("deny",       "strict"),
    ] {
        app.state.policy_default = Some(input.to_string());
        let line = app.footer_line();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains(expected), "footer for '{input}' should contain '{expected}', got: {text}");
    }
}

#[test]
fn policy_picker_lines_highlights_correct_index() {
    let mut app = App::default();
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 2 }); // strict
    let lines = app.policy_picker_lines().expect("picker lines should exist");
    // 2 header lines + 3 options = 5 lines; option at index 2 is lines[4]
    let strict_line_text: String = lines[4].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(strict_line_text.contains("strict"), "last option line should contain 'strict'");
    // leading marker should be '› ' for selected
    assert!(lines[4].spans[0].content.contains('›'), "selected option should have '›' marker");
}

#[test]
fn footer_mode_is_policy_picker_active_when_picker_open() {
    use tui_next::app::FooterMode;
    let mut app = App::default();
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 1 });
    let layout = app.bottom_layout(80);
    assert_eq!(layout.footer_mode, FooterMode::PolicyPickerActive);
}
```

- [ ] **Step 2: 运行新测试**

```bash
zsh -lc "cargo test -p tui_next --test m23_policy_picker_behavior"
```

Expected: 所有 8 个测试通过

- [ ] **Step 3: 运行回归测试**

```bash
zsh -lc "cargo test -p tui_next --test m10_approval_panel_navigation && cargo test -p tui_next --test m7_startup_banner_once"
```

Expected: 两个套件均通过

- [ ] **Step 4: 运行全量 tui 测试**

```bash
zsh -lc "cargo test -p tui_next"
```

Expected: 全部通过

- [ ] **Step 5: 提交**

```bash
git add ui/tui/tests/m23_policy_picker_behavior.rs
git commit -m "test(tui): m23 policy picker 集成测试"
```

---

## Task 11: Gateway — policy 层级切换 API

**Files:**
- Modify: `openjax-gateway/src/handlers/session.rs`
- Modify: `openjax-gateway/src/lib.rs`

- [ ] **Step 1: 在 `session.rs` 添加请求体类型和 handler**

在文件末尾添加：

```rust
#[derive(serde::Deserialize)]
pub struct SetPolicyLevelRequest {
    pub level: String,
}

#[derive(serde::Serialize)]
pub struct SetPolicyLevelResponse {
    pub level: String,
}

pub async fn set_policy_level(
    axum::extract::Path(session_id): axum::extract::Path<String>,
    axum::Extension(state): axum::Extension<crate::state::AppState>,
    axum::Json(body): axum::Json<SetPolicyLevelRequest>,
) -> Result<axum::Json<SetPolicyLevelResponse>, crate::error::ApiError> {
    let level = openjax_core::PolicyLevel::from_str(&body.level)
        .ok_or_else(|| crate::error::ApiError::bad_request(
            "invalid_policy_level",
            format!("'{}' is not a valid policy level; use permissive, standard, or strict", body.level),
        ))?;

    // 取出 session 对应的 agent Arc（先释放 session map lock，再 lock agent）
    let agent_arc = {
        let sessions = state.sessions.read().await; // 或对应的 read 方法
        sessions
            .get(&session_id)
            .map(|s| s.agent.clone())
            .ok_or_else(|| crate::error::ApiError::not_found("session not found"))?
    };
    agent_arc.lock().await.set_policy_level(level);

    Ok(axum::Json(SetPolicyLevelResponse {
        level: level.as_str().to_string(),
    }))
}
```

注：`state.sessions`、`agent_arc` 的确切访问方式依 `AppState` / `SessionRuntime` 的实际结构而定，请在实现时参考 `session.rs` 中已有的 handler（如 `resolve_approval`）的模式。

- [ ] **Step 2: 在 `lib.rs` 注册路由**

在 session 路由注册处（已有 `POST .../approvals/*` 等路由附近）添加：

```rust
.route(
    "/api/v1/sessions/:session_id/policy",
    axum::routing::put(crate::handlers::session::set_policy_level),
)
```

- [ ] **Step 3: 构建验证**

```bash
zsh -lc "cargo build -p openjax-gateway"
```

Expected: 编译通过

- [ ] **Step 4: 提交**

```bash
git add openjax-gateway/src/handlers/session.rs openjax-gateway/src/lib.rs
git commit -m "feat(gateway): PUT /api/v1/sessions/:id/policy — session-local policy level 切换"
```

---

## Task 12: Gateway 集成测试

**Files:**
- Modify: `openjax-gateway/tests/gateway_api.rs`

- [ ] **Step 1: 添加 policy level 接口测试**

参考 `gateway_api.rs` 已有的测试（如审批接口测试）的 helper/fixture 模式，添加：

```rust
// --- policy level 切换 ---

#[tokio::test]
async fn put_policy_valid_level_returns_200() {
    let (app, session_id) = setup_session().await; // 使用已有 helper
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{session_id}/policy"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", test_token()))
                .body(Body::from(r#"{"level":"permissive"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = parse_json_body(resp).await;
    assert_eq!(body["level"], "permissive");
}

#[tokio::test]
async fn put_policy_invalid_level_returns_400() {
    let (app, session_id) = setup_session().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{session_id}/policy"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", test_token()))
                .body(Body::from(r#"{"level":"ultra"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 2: 运行 gateway 集成测试**

```bash
zsh -lc "cargo test -p openjax-gateway"
```

Expected: 全部通过，含新增两个测试

- [ ] **Step 3: 提交**

```bash
git add openjax-gateway/tests/gateway_api.rs
git commit -m "test(gateway): PUT /sessions/:id/policy 的 200/400 集成测试"
```

---

## Task 13: 全量验证

- [ ] **Step 1: 运行全量测试**

```bash
zsh -lc "cargo test --workspace"
```

Expected: 全部通过，无 regression

- [ ] **Step 2: 手动运行 TUI 验证**

```bash
zsh -lc "cargo run -q -p tui_next"
```

验证项：
1. footer 底部显示 `| policy: standard`（白色）
2. 输入 `/policy` + Enter → picker overlay 弹出，显示三个选项
3. `↑↓` 移动选中项（高亮变化）
4. Enter 确认 permissive → footer 变为 `| policy: permissive`（Cyan）
5. 再次 `/policy` → picker 默认高亮 permissive（index 0）
6. Esc 取消 → picker 关闭，policy 不变
7. 有 pending approval 时输入 `/policy` → picker 不弹出
