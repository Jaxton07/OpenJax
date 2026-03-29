# 暂时下线 apply_patch 模型暴露层 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 通过新增 `ApplyPatchToolType::Disabled`，将 `apply_patch` 从默认工具面和 prompt 中移除，同时保留源码和测试文件。

**Architecture:** 沿用 `ShellToolType::Disabled` 的现有模式，在 `ToolsConfig`、`spec.rs`、`tool_builder.rs` 中统一加入 `Disabled` 判断，并清理 agent prompt、planner utils、decision、tool guard 中的 `apply_patch` 引用。

**Tech Stack:** Rust 2024, cargo test, tokio

---

## 文件变更总览

- `openjax-core/src/tools/spec.rs` — 给 `ApplyPatchToolType` 增加 `Disabled` 变体，修改 `ToolsConfig::default()` 与 `build_all_specs`
- `openjax-core/src/tools/tool_builder.rs` — 条件注册 `ApplyPatchHandler`，更新单元测试
- `openjax-core/src/agent/prompt.rs` — 删除 system prompt / planner prompt 中对 `apply_patch` 的提及
- `openjax-core/src/agent/planner_utils.rs` — 从 `extract_tool_target_hint` 和 `is_mutating_tool` 中移除 `apply_patch`
- `openjax-core/src/agent/decision.rs` — 从 `canonical_tool_name` 中移除 `apply_patch`
- `openjax-core/src/agent/tool_guard.rs` — 删除 `ApplyPatchReadGuard`
- `openjax-core/src/agent/planner.rs` — 移除 `ToolActionContext` 中的 `apply_patch_read_guard` 及使用点
- `openjax-core/tests/tools_sandbox_suite.rs` — 注释掉 `apply_patch_m4`
- `openjax-core/tests/tools_sandbox/m4_apply_patch.rs` — 给整个模块加 `#[ignore]`
- `openjax-core/src/tools/README.md` 与 `docs/tools-list.md` — 更新当前支持工具列表

---

### Task 1: 配置层 — 在 `spec.rs` 支持 `ApplyPatchToolType::Disabled`

**Files:**
- Modify: `openjax-core/src/tools/spec.rs:29-40`

- [ ] **Step 1: 修改 `ApplyPatchToolType` 枚举**

在 `openjax-core/src/tools/spec.rs` 中：

```rust
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum ApplyPatchToolType {
    Default,
    Freeform,
    Disabled,
}
```

- [ ] **Step 2: 修改 `ToolsConfig::default()`**

```rust
impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            shell_type: ShellToolType::Default,
            apply_patch_tool_type: Some(ApplyPatchToolType::Disabled),
        }
    }
}
```

- [ ] **Step 3: 修改 `build_all_specs` 中的 `apply_patch` 加入逻辑**

把这段：

```rust
    specs.push(match config.apply_patch_tool_type {
        Some(ApplyPatchToolType::Freeform) => create_apply_patch_freeform_spec(),
        _ => create_apply_patch_spec(),
    });
```

改为：

```rust
    if !matches!(
        config.apply_patch_tool_type,
        Some(ApplyPatchToolType::Disabled)
    ) {
        specs.push(match config.apply_patch_tool_type {
            Some(ApplyPatchToolType::Freeform) => create_apply_patch_freeform_spec(),
            _ => create_apply_patch_spec(),
        });
    }
```

- [ ] **Step 4: 更新 `spec.rs` 的已有单元测试**

找到 `build_all_specs_exposes_read_edit_contract_names` 测试（或任何断言默认 specs 包含 `apply_patch` 的测试），将其中的 `assert!(names.contains(&"apply_patch".to_string()));` 删除或改为断言不包含。

```rust
    #[test]
    fn build_all_specs_exposes_read_edit_contract_names() {
        let names: Vec<String> = build_all_specs(&ToolsConfig::default())
            .into_iter()
            .map(|spec| spec.name)
            .collect();
        let legacy_read = format!("{}_{}", "read", "file");
        let legacy_edit = format!("{}_{}_{}", "edit", "file", "range");
        assert!(names.contains(&"Read".to_string()));
        assert!(names.contains(&"Edit".to_string()));
        assert!(!names.contains(&legacy_read));
        assert!(!names.contains(&legacy_edit));
        assert!(!names.contains(&"apply_patch".to_string()));
    }
```

如果测试中没有对 `apply_patch` 的断言，则新增一个 `build_all_specs_hides_apply_patch_when_disabled`：

```rust
    #[test]
    fn build_all_specs_hides_apply_patch_when_disabled() {
        let names: Vec<String> = build_all_specs(&ToolsConfig::default())
            .into_iter()
            .map(|spec| spec.name)
            .collect();
        assert!(!names.contains(&"apply_patch".to_string()));
    }
```

- [ ] **Step 5: 运行测试验证**

Run: `zsh -lc "cargo test -p openjax-core build_all_specs -- --nocapture"`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add openjax-core/src/tools/spec.rs
git commit -m "$(cat <<'EOF'
feat(core,tools): ApplyPatchToolType 增加 Disabled 变体并设为默认值

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: 注册层 — 条件注册 `ApplyPatchHandler`

**Files:**
- Modify: `openjax-core/src/tools/tool_builder.rs:55-101`

- [ ] **Step 1: 修改 `build_tool_registry_with_config`**

在注册 `apply_patch` 之前加入条件判断：

```rust
    if !matches!(
        config.apply_patch_tool_type,
        Some(crate::tools::spec::ApplyPatchToolType::Disabled)
    ) {
        let patch_handler = Arc::new(ApplyPatchHandler);
        builder.register_handler("apply_patch", patch_handler);
    }
```

把原来直接注册的两行包进去即可。确保外部 `import` 不变。

- [ ] **Step 2: 更新 `tool_builder.rs` 的单元测试**

修改 `default_registry_includes_system_tools` 测试，把对 `apply_patch` 存在的断言改为不存在：

```rust
        assert!(registry.handler("apply_patch").is_none());
```

同时把 `names.contains(&"apply_patch".to_string())` 改为 `assert!(!names.contains(&"apply_patch".to_string()));`。

修改 `registry_build_respects_apply_patch_tool_type` 测试：
把 `freeform` 配置的 `apply_patch_tool_type` 显式写为 `Some(ApplyPatchToolType::Freeform)`。在该配置下仍断言 `apply_patch` spec 包含 `FREEFORM` 且 handler 存在。
并增加 `disabled` 分支的断言：

```rust
        let disabled = ToolsConfig {
            shell_type: crate::tools::spec::ShellToolType::Default,
            apply_patch_tool_type: Some(crate::tools::spec::ApplyPatchToolType::Disabled),
        };
        let (registry, specs) = build_tool_registry_with_config(&disabled);
        assert!(registry.handler("apply_patch").is_none());
        assert!(!specs.iter().any(|spec| spec.name == "apply_patch"));
```

- [ ] **Step 3: 运行测试验证**

Run: `zsh -lc "cargo test -p openjax-core tool_builder -- --nocapture"`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add openjax-core/src/tools/tool_builder.rs
git commit -m "$(cat <<'EOF'
feat(core,tools): 默认注册表不再注册 apply_patch handler

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Prompt 层 — 删除 system prompt 和 planner prompt 中的 `apply_patch`

**Files:**
- Modify: `openjax-core/src/agent/prompt.rs`

- [ ] **Step 1: 修改 `build_system_prompt`**

找到这段：

```rust
- For multi-file edits or file operations (add/delete/move/rename), use apply_patch.
```

删除该行。再把下面这行也删掉：

```rust
- For apply_patch, follow the format contract in the apply_patch tool description.
```

改为：

```rust
Tool selection policy:
- Modify existing files only after calling `Read`.
- Use `Edit` for single-file existing-text edits.
- If `Edit` fails, call `Read` before retrying.
- For multi-file edits or file operations (add/delete/move/rename), use shell.
- Prefer process_snapshot/system_load/disk_usage for process/host metrics over shell ps/top/df.
- When modifying existing files, preserve the source file's formatting and style.
- For shell, prefer workspace-relative commands; avoid absolute-path `cd` unless required.
- Skill markers like /skill-name are not shell executables; convert selected skills into concrete tool steps.
```

- [ ] **Step 2: 修改 `build_planner_input`**

删除以下引用：

```rust
- For apply_patch, follow the format contract in the apply_patch tool description.
```

把工具列表中的 `apply_patch` 也删除：

```rust
1) Tool call: {{"action":"tool","tool":"Read|list_dir|grep_files|process_snapshot|system_load|disk_usage|shell|Edit","args":{{...}}}}
```

把规则中的：

```rust
  - Prefer apply_patch for multi-file edits or file operations (add/delete/move/rename).
  - If apply_patch fails with context mismatch (e.g., hunk context not found), call `Read` before any further edits.
```

改为：

```rust
  - For multi-file edits or file operations (add/delete/move/rename), use shell.
```

- [ ] **Step 3: 修改 `build_json_repair_prompt`**

同样把工具列表中的 `apply_patch` 删除，即改为：

```rust
1) {{"action":"tool","tool":"Read|list_dir|grep_files|process_snapshot|system_load|disk_usage|shell|Edit","args":{{...}}}}
```

- [ ] **Step 4: 运行相关测试**

Run: `zsh -lc "cargo test -p openjax-core prompt_and_policy -- --nocapture"`
如果该测试文件对 prompt 内容做了精确字符串匹配（比如断言包含 `apply_patch`），同步修正断言为新的 prompt 文本或不包含 `apply_patch`。

- [ ] **Step 5: Commit**

```bash
git add openjax-core/src/agent/prompt.rs
git commit -m "$(cat <<'EOF'
feat(core,agent): 从 system prompt 和 planner prompt 中移除 apply_patch

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Planner Utils — 移除 `apply_patch` 相关辅助函数引用

**Files:**
- Modify: `openjax-core/src/agent/planner_utils.rs`

- [ ] **Step 1: 修改 `extract_tool_target_hint`**

把：

```rust
    let keys: &[&str] = match tool_name {
        "Read" | "Edit" | "apply_patch" | "write_file" => {
            &["file_path", "path", "filepath"]
        }
```

改为：

```rust
    let keys: &[&str] = match tool_name {
        "Read" | "Edit" | "write_file" => {
            &["file_path", "path", "filepath"]
        }
```

- [ ] **Step 2: 修改 `is_mutating_tool`**

把：

```rust
pub(super) fn is_mutating_tool(tool_name: &str) -> bool {
    matches!(tool_name, "Edit" | "apply_patch" | "shell" | "exec_command")
}
```

改为：

```rust
pub(super) fn is_mutating_tool(tool_name: &str) -> bool {
    matches!(tool_name, "Edit" | "shell" | "exec_command")
}
```

- [ ] **Step 3: 运行编译检查**

Run: `zsh -lc "cargo check -p openjax-core"`
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add openjax-core/src/agent/planner_utils.rs
git commit -m "$(cat <<'EOF'
feat(core,agent): planner_utils 中移除 apply_patch 引用

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Decision 层 — 从 `canonical_tool_name` 中移除 `apply_patch`

**Files:**
- Modify: `openjax-core/src/agent/decision.rs:365-378`

- [ ] **Step 1: 修改 `canonical_tool_name`**

把 `"apply_patch" => Some("apply_patch"),` 这一整行删除。

```rust
fn canonical_tool_name(name: &str) -> Option<&'static str> {
    match name.trim().to_ascii_lowercase().as_str() {
        "read" => Some("Read"),
        "list_dir" => Some("list_dir"),
        "grep_files" => Some("grep_files"),
        "process_snapshot" => Some("process_snapshot"),
        "system_load" => Some("system_load"),
        "disk_usage" => Some("disk_usage"),
        "shell" => Some("shell"),
        "edit" => Some("Edit"),
        _ => None,
    }
}
```

- [ ] **Step 2: 运行 decision 模块测试**

Run: `zsh -lc "cargo test -p openjax-core decision -- --nocapture"`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add openjax-core/src/agent/decision.rs
git commit -m "$(cat <<'EOF'
feat(core,agent): 从 canonical_tool_name 中移除 apply_patch

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: 删除 `ApplyPatchReadGuard`

**Files:**
- Delete contents: `openjax-core/src/agent/tool_guard.rs`
- Modify: `openjax-core/src/agent/planner.rs`

- [ ] **Step 1: 清空 `tool_guard.rs`**

将整个文件替换为空内容（或者保留 `#![allow(dead_code)]` 的空文件）。为了最小变更，直接删掉全部内容写成一个空文件即可。注意：其他文件对 `ApplyPatchReadGuard` 的 import 会在下一步清理。

- [ ] **Step 2: 修改 `planner.rs` 移除 `ApplyPatchReadGuard` 引用**

删除 import：

```rust
use crate::agent::tool_guard::ApplyPatchReadGuard;
```

把 `ToolActionContext` 结构体中的这行删除：

```rust
    pub apply_patch_read_guard: &'a mut ApplyPatchReadGuard,
```

在 `execute_natural_language_turn` 函数中：
1. 删除 `let mut apply_patch_read_guard = ApplyPatchReadGuard::default();`
2. 删除构造 `ctx` 时传入的 `apply_patch_read_guard: &mut apply_patch_read_guard,`

- [ ] **Step 3: 运行编译检查**

Run: `zsh -lc "cargo check -p openjax-core"`
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add openjax-core/src/agent/tool_guard.rs openjax-core/src/agent/planner.rs
git commit -m "$(cat <<'EOF'
feat(core,agent): 删除 ApplyPatchReadGuard 及相关引用

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: 跳过 `apply_patch` 集成测试

**Files:**
- Modify: `openjax-core/tests/tools_sandbox_suite.rs`
- Modify: `openjax-core/tests/tools_sandbox/m4_apply_patch.rs`

- [ ] **Step 1: 在 suite 中注释掉 `apply_patch` 模块**

```rust
//! Aggregated integration suite for tool execution, sandbox policy, and file mutation flows.

// Temporarily hidden: #[path = "tools_sandbox/m4_apply_patch.rs"]
// mod apply_patch_m4;
#[path = "tools_sandbox/m5_edit.rs"]
mod edit_m5;
...
```

- [ ] **Step 2: 给 `m4_apply_patch.rs` 整个模块加 `#[ignore]`**

在 `openjax-core/tests/tools_sandbox/m4_apply_patch.rs` 第一行加上：

```rust
#![allow(dead_code)]
```

然后在第二个非空行（`use async_trait...` 之前）加上：

不对，集成测试文件本身就是直接写 `#[tokio::test]` 的，无法给整个文件加 `#[ignore]`。最简单的方式是给每个 `#[tokio::test]` 前面加 `#[ignore = "apply_patch temporarily hidden from model"]`。如果测试数量较多，用正则批量替换也行。

批量把文件中所有的 `#[tokio::test]` 替换为：

```rust
#[tokio::test]
#[ignore = "apply_patch temporarily hidden from model"]
```

- [ ] **Step 3: 运行 suite 测试确认跳过**

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite -- --ignored --nocapture"`
Expected: 应该会显示 `apply_patch` 相关测试被忽略

再跑不忽略的版本确认没有失败：
Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite -- --nocapture"`
Expected: PASS（apply_patch 测试不再运行）

- [ ] **Step 4: Commit**

```bash
git add openjax-core/tests/tools_sandbox_suite.rs openjax-core/tests/tools_sandbox/m4_apply_patch.rs
git commit -m "$(cat <<'EOF'
test(core): 暂时跳过 apply_patch 集成测试

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: 更新文档中的工具列表

**Files:**
- Modify: `openjax-core/src/tools/README.md`
- Modify: `openjax-core/src/tools/docs/tools-list.md`

- [ ] **Step 1: 更新 `README.md`**

找到这句：

```markdown
- 文件/代码工具：`Read`、`list_dir`、`grep_files`、`glob_files`、`write_file`、`Edit`、`apply_patch`
```

改为：

```markdown
- 文件/代码工具：`Read`、`list_dir`、`grep_files`、`glob_files`、`write_file`、`Edit`
```

- [ ] **Step 2: 更新 `tools-list.md`**

在工具对比表格（Tool comparison table）中，把 `apply_patch` 所在行删除。如果表格是：

```markdown
| 工具 | 变异操作 | 超时 | 沙箱支持 | 主要用途 |
```

删除 `| apply_patch | ... |` 这一行即可。

- [ ] **Step 3: Commit**

```bash
git add openjax-core/src/tools/README.md openjax-core/src/tools/docs/tools-list.md
git commit -m "$(cat <<'EOF'
docs(core): 更新文档中当前支持的工具列表，移除 apply_patch

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: 全量测试回归

- [ ] **Step 1: 运行 `openjax-core` 分层测试**

Run: `zsh -lc "make core-smoke"`
Expected: PASS

Run: `zsh -lc "make core-feature-tools"`
Expected: PASS

Run: `zsh -lc "make core-feature-approval"`
Expected: PASS

- [ ] **Step 2: 确认无旧名残留**

Run: `zsh -lc "grep -R 'apply_patch' openjax-core/src/agent/ openjax-core/src/tools/tool_builder.rs openjax-core/src/tools/spec.rs openjax-core/src/tools/handlers/ || true"`
Expected: 仅可能在 handler 源码、`apply_patch/` 子目录、`apply_patch_interceptor.rs` 中还有残留，agent/、spec.rs、tool_builder.rs 中应没有默认注册/暴露的引用。

- [ ] **Step 3: 最终提交（可选）**

如果全部通过，可 push 或保持本地 commit 等待下一步。
