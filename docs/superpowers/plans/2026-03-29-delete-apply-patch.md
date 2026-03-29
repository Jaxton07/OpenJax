# 彻底删除 apply_patch 工具实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 从代码、配置、测试、文档所有层面彻底删除 `apply_patch` 工具，不留任何残留引用。

**Architecture:** 删除 `openjax-core/src/tools/apply_patch/` 整个目录及其 handler、interceptor，清理 `spec.rs` 和 `tool_builder.rs` 中的相关配置项和枚举，删除所有测试文件，清理所有活跃和历史文档中的引用。

**Tech Stack:** Rust

---

## Task 1: 删除 apply_patch 核心实现目录和文件

**Files:**
- Delete: `openjax-core/src/tools/apply_patch/` (整个目录)
- Delete: `openjax-core/src/tools/handlers/apply_patch.rs`
- Delete: `openjax-core/src/tools/apply_patch_interceptor.rs`
- Delete: `openjax-core/tests/tools_sandbox/m4_apply_patch.rs`

- [ ] **Step 1: 删除 apply_patch 目录**

```bash
rm -rf openjax-core/src/tools/apply_patch/
```

- [ ] **Step 2: 删除 handler 和 interceptor 文件**

```bash
rm openjax-core/src/tools/handlers/apply_patch.rs
rm openjax-core/src/tools/apply_patch_interceptor.rs
```

- [ ] **Step 3: 删除测试文件**

```bash
rm openjax-core/tests/tools_sandbox/m4_apply_patch.rs
```

- [ ] **Step 4: 验证文件已删除**

```bash
ls openjax-core/src/tools/apply_patch/ 2>&1 || echo "目录已删除"
ls openjax-core/src/tools/handlers/apply_patch.rs 2>&1 || echo "文件已删除"
ls openjax-core/src/tools/apply_patch_interceptor.rs 2>&1 || echo "文件已删除"
ls openjax-core/tests/tools_sandbox/m4_apply_patch.rs 2>&1 || echo "文件已删除"
```

Expected: 所有检查都显示 "已删除"

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "🗑️ refactor(core): 删除 apply_patch 核心实现文件

删除以下文件和目录:
- openjax-core/src/tools/apply_patch/ (整个目录)
- openjax-core/src/tools/handlers/apply_patch.rs
- openjax-core/src/tools/apply_patch_interceptor.rs
- openjax-core/tests/tools_sandbox/m4_apply_patch.rs

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 2: 清理 mod.rs 和 handlers/mod.rs

**Files:**
- Modify: `openjax-core/src/tools/mod.rs`
- Modify: `openjax-core/src/tools/handlers/mod.rs`

- [ ] **Step 1: 读取当前 mod.rs 内容**

```bash
cat openjax-core/src/tools/mod.rs
```

- [ ] **Step 2: 编辑 mod.rs 删除 apply_patch 相关行**

删除以下行:
- `pub mod apply_patch;`
- `pub mod apply_patch_interceptor;`
- `pub use apply_patch::{...}` 整行

文件应保留其他模块声明和导出。

- [ ] **Step 3: 读取当前 handlers/mod.rs 内容**

```bash
cat openjax-core/src/tools/handlers/mod.rs
```

- [ ] **Step 4: 编辑 handlers/mod.rs 删除 apply_patch 相关行**

删除以下行:
- `pub mod apply_patch;`
- `pub use apply_patch::ApplyPatchHandler;`

- [ ] **Step 5: 运行 cargo check 验证**

```bash
cargo check -p openjax-core 2>&1 | head -50
```

Expected: 无与 `apply_patch` 相关的编译错误（可能会有其他文件的错误，继续下一步修复）

- [ ] **Step 6: 提交**

```bash
git add openjax-core/src/tools/mod.rs openjax-core/src/tools/handlers/mod.rs
git commit -m "🗑️ refactor(core): 从 mod.rs 中移除 apply_patch 模块声明

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 3: 清理 spec.rs

**Files:**
- Modify: `openjax-core/src/tools/spec.rs`

- [ ] **Step 1: 读取 spec.rs 确定需要删除的内容**

```bash
cat openjax-core/src/tools/spec.rs
```

- [ ] **Step 2: 删除 `ApplyPatchToolType` 枚举**

删除:
```rust
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum ApplyPatchToolType {
    Default,
    Freeform,
    Disabled,
}
```

- [ ] **Step 3: 从 `ToolsConfig` 中删除 `apply_patch_tool_type` 字段**

将:
```rust
pub struct ToolsConfig {
    pub shell_type: ShellToolType,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
}
```
改为:
```rust
pub struct ToolsConfig {
    pub shell_type: ShellToolType,
}
```

- [ ] **Step 4: 从 `Default` impl 中删除 `apply_patch_tool_type`**

删除 `apply_patch_tool_type: Some(ApplyPatchToolType::Disabled),` 这一行。

- [ ] **Step 5: 删除 `APPLY_PATCH_FORMAT_DETAIL` 常量**

删除从 `const APPLY_PATCH_FORMAT_DETAIL: &str = r#"` 开始到 `"#;` 结束的整个常量定义。

- [ ] **Step 6: 删除 `create_apply_patch_spec` 函数**

删除整个函数（约 25 行）。

- [ ] **Step 7: 删除 `create_apply_patch_freeform_spec` 函数**

删除整个函数（约 40 行）。

- [ ] **Step 8: 从 `build_all_specs` 中删除 apply_patch 相关逻辑**

删除:
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

- [ ] **Step 9: 删除 spec.rs 中的测试**

删除 `#[cfg(test)]` 模块中与 `apply_patch` 相关的测试用例。

- [ ] **Step 10: 运行 cargo check 验证**

```bash
cargo check -p openjax-core 2>&1 | head -50
```

- [ ] **Step 11: 提交**

```bash
git add openjax-core/src/tools/spec.rs
git commit -m "🗑️ refactor(core): 从 spec.rs 移除 apply_patch 相关代码

- 删除 ApplyPatchToolType 枚举
- 从 ToolsConfig 移除 apply_patch_tool_type 字段
- 删除 APPLY_PATCH_FORMAT_DETAIL 常量
- 删除 create_apply_patch_spec 和 create_apply_patch_freeform_spec 函数
- 从 build_all_specs 移除条件注册逻辑
- 清理相关单元测试

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 4: 清理 tool_builder.rs

**Files:**
- Modify: `openjax-core/src/tools/tool_builder.rs`

- [ ] **Step 1: 读取 tool_builder.rs**

```bash
cat openjax-core/src/tools/tool_builder.rs
```

- [ ] **Step 2: 删除 `ApplyPatchHandler` 导入**

从 use 语句中删除 `ApplyPatchHandler`。

- [ ] **Step 3: 删除 `ApplyPatchToolType` 导入**

删除 `use crate::tools::spec::{ApplyPatchToolType, ...};` 中的 `ApplyPatchToolType`。

- [ ] **Step 4: 删除条件注册块**

删除:
```rust
if !matches!(
    config.apply_patch_tool_type,
    Some(crate::tools::spec::ApplyPatchToolType::Disabled)
) {
    let patch_handler = Arc::new(ApplyPatchHandler);
    builder.register_handler("apply_patch", patch_handler);
}
```

- [ ] **Step 5: 删除 `build_tool_registry_with_config` 中的 parallel 逻辑**

删除:
```rust
let parallel = !spec.name.eq("apply_patch");
builder.push_spec(spec, parallel);
```
改为:
```rust
builder.push_spec(spec, true);
```

- [ ] **Step 6: 删除单元测试中与 apply_patch 相关的部分**

删除测试 `default_registry_includes_system_tools` 中的:
```rust
assert!(registry.handler("apply_patch").is_none());
...
assert!(!names.contains(&"apply_patch".to_string()));
```

删除整个测试 `registry_build_respects_apply_patch_tool_type`。

- [ ] **Step 7: 运行 cargo check 验证**

```bash
cargo check -p openjax-core 2>&1 | head -50
```

- [ ] **Step 8: 提交**

```bash
git add openjax-core/src/tools/tool_builder.rs
git commit -m "🗑️ refactor(core): 从 tool_builder.rs 移除 apply_patch 相关代码

- 删除 ApplyPatchHandler 导入
- 删除条件注册 apply_patch 的代码块
- 简化 push_spec 调用（移除 parallel 特殊处理）
- 清理相关单元测试

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 5: 清理 router.rs 中的测试

**Files:**
- Modify: `openjax-core/src/tools/router.rs`

- [ ] **Step 1: 读取 router.rs 测试部分**

```bash
cat openjax-core/src/tools/router.rs | tail -60
```

- [ ] **Step 2: 删除 apply_patch 相关的测试用例**

删除以下测试:
- `parse_tool_call_preserves_quoted_apply_patch`
- `parse_tool_call_preserves_quoted_apply_patch_update`
- `parse_tool_call_rejects_unclosed_quote`（如果它只测试 apply_patch）

保留 `parse_tool_call_preserves_quoted_shell_command` 和 `parse_tool_call_rejects_unclosed_quote`（如果有其他用途）。

- [ ] **Step 3: 运行 cargo check 验证**

```bash
cargo check -p openjax-core 2>&1 | head -50
```

- [ ] **Step 4: 提交**

```bash
git add openjax-core/src/tools/router.rs
git commit -m "🗑️ refactor(core): 从 router.rs 测试移除 apply_patch 用例

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 6: 检查并清理 router_impl.rs 和 orchestrator.rs

**Files:**
- Check/Modify: `openjax-core/src/tools/router_impl.rs`
- Check/Modify: `openjax-core/src/tools/orchestrator.rs`

- [ ] **Step 1: 搜索 router_impl.rs 中的 apply_patch 引用**

```bash
grep -n "apply_patch" openjax-core/src/tools/router_impl.rs || echo "无匹配"
```

- [ ] **Step 2: 如有引用，删除并验证**

根据 grep 结果删除相关代码，然后:
```bash
cargo check -p openjax-core 2>&1 | head -30
```

- [ ] **Step 3: 搜索 orchestrator.rs 中的 apply_patch 引用**

```bash
grep -n "apply_patch" openjax-core/src/tools/orchestrator.rs || echo "无匹配"
```

- [ ] **Step 4: 如有引用，删除并验证**

根据 grep 结果删除相关代码，然后:
```bash
cargo check -p openjax-core 2>&1 | head -30
```

- [ ] **Step 5: 提交（如有修改）**

```bash
git add -A
git commit -m "🗑️ refactor(core): 清理 router_impl.rs 和 orchestrator.rs 中的 apply_patch 引用

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 7: 清理测试套件引用

**Files:**
- Modify: `openjax-core/tests/tools_sandbox_suite.rs`

- [ ] **Step 1: 读取测试套件文件**

```bash
cat openjax-core/tests/tools_sandbox_suite.rs
```

- [ ] **Step 2: 删除 apply_patch 相关的注释或模块引用**

删除类似以下的内容:
```rust
// apply_patch 测试暂时跳过...
// #[path = "tools_sandbox/m4_apply_patch.rs"]
// mod apply_patch_m4;
```

如果有实际的 `mod apply_patch_m4;` 引用，也需要删除。

- [ ] **Step 3: 运行测试验证**

```bash
cargo test -p openjax-core --test tools_sandbox_suite 2>&1 | tail -20
```

Expected: 测试通过，无 apply_patch 相关测试

- [ ] **Step 4: 提交**

```bash
git add openjax-core/tests/tools_sandbox_suite.rs
git commit -m "🗑️ refactor(core): 从测试套件移除 apply_patch 引用

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 8: 清理活跃文档

**Files:**
- Modify: `openjax-core/README.md`
- Modify: `openjax-core/src/tools/README.md`
- Modify: `openjax-core/src/tools/docs/tools-list.md`
- Modify: `openjax-core/src/tools/docs/usage-guide.md`
- Modify: `openjax-core/src/tools/docs/architecture.md`
- Modify: `openjax-core/src/tools/docs/references.md`
- Modify: `openjax-core/src/tools/docs/core-components.md`

- [ ] **Step 1: 搜索所有活跃文档中的 apply_patch 引用**

```bash
grep -r "apply_patch" openjax-core/README.md openjax-core/src/tools/README.md openjax-core/src/tools/docs/ 2>/dev/null | head -30
```

- [ ] **Step 2: 逐个编辑文件删除引用**

对每个包含 `apply_patch` 的文件:
1. 使用 Read 工具读取文件
2. 使用 Edit 工具删除包含 `apply_patch` 的行或段落
3. 保持文档的连贯性和格式

- [ ] **Step 3: 验证清理完成**

```bash
grep -r "apply_patch" openjax-core/README.md openjax-core/src/tools/README.md openjax-core/src/tools/docs/ 2>/dev/null || echo "活跃文档清理完成"
```

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "📝 docs(core): 从活跃文档移除 apply_patch 引用

清理以下文件:
- openjax-core/README.md
- openjax-core/src/tools/README.md
- openjax-core/src/tools/docs/*.md

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 9: 删除下线方案相关文档

**Files:**
- Delete: `docs/superpowers/specs/2026-03-29-hide-apply-patch-design.md`
- Delete: `docs/superpowers/plans/2026-03-29-hide-apply-patch.md`

- [ ] **Step 1: 删除下线方案设计文档**

```bash
rm docs/superpowers/specs/2026-03-29-hide-apply-patch-design.md
```

- [ ] **Step 2: 删除下线方案实施计划**

```bash
rm docs/superpowers/plans/2026-03-29-hide-apply-patch.md
```

- [ ] **Step 3: 验证删除**

```bash
ls docs/superpowers/specs/2026-03-29-hide-apply-patch-design.md 2>&1 || echo "文件已删除"
ls docs/superpowers/plans/2026-03-29-hide-apply-patch.md 2>&1 || echo "文件已删除"
```

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "🗑️ docs: 删除 apply_patch 下线方案相关文档

删除:
- docs/superpowers/specs/2026-03-29-hide-apply-patch-design.md
- docs/superpowers/plans/2026-03-29-hide-apply-patch.md

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 10: 清理设计文档中的引用

**Files:**
- Modify: `docs/superpowers/specs/2026-03-29-read-edit-tool-design.md`
- Modify: `docs/superpowers/specs/2026-03-28-native-tool-calling-remaining-phases-design.md`

- [ ] **Step 1: 搜索设计文档中的 apply_patch 引用**

```bash
grep -n "apply_patch" docs/superpowers/specs/2026-03-29-read-edit-tool-design.md docs/superpowers/specs/2026-03-28-native-tool-calling-remaining-phases-design.md 2>/dev/null || echo "无匹配"
```

- [ ] **Step 2: 如有引用，清理并提交**

根据 grep 结果编辑文件，然后:
```bash
git add -A
git commit -m "📝 docs: 从设计文档移除 apply_patch 引用

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 11: 清理计划和历史文档

**Files:**
- Modify: `docs/plan/refactor/tools/native-tool-calling-plan.md`
- Modify: `docs/plan/refactor/tools/tool-optimization-plan.md`
- Modify: `docs/tools.md`
- Modify: `docs/security.md`
- Modify: `AGENTS.md`
- Modify: `openjax-policy/README.md`

- [ ] **Step 1: 搜索这些文件中的 apply_patch 引用**

```bash
grep -rn "apply_patch" docs/plan/ docs/tools.md docs/security.md AGENTS.md openjax-policy/README.md 2>/dev/null | head -30
```

- [ ] **Step 2: 逐个编辑文件删除引用**

对每个包含 `apply_patch` 的文件，删除相关段落或句子。

- [ ] **Step 3: 提交**

```bash
git add -A
git commit -m "📝 docs: 从计划和策略文档移除 apply_patch 引用

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 12: 清理 archive 文档

**Files:**
- Modify: `docs/archive/core/codex-quick-reference.md`
- Modify: `docs/archive/core/codex-architecture-reference.md`
- Modify: `docs/archive/core/implementation-plan.md`
- Modify: `docs/archive/core/openjax-vs-codex-comparison.md`

- [ ] **Step 1: 搜索 archive 中的 apply_patch 引用**

```bash
grep -rn "apply_patch" docs/archive/core/ 2>/dev/null | head -30
```

- [ ] **Step 2: 逐个编辑文件删除引用**

对每个包含 `apply_patch` 的文件，删除相关段落或句子。

- [ ] **Step 3: 提交**

```bash
git add -A
git commit -m "📝 docs: 从 archive 文档移除 apply_patch 引用

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 13: 最终验证

- [ ] **Step 1: 运行 cargo check**

```bash
cargo check --workspace 2>&1 | tail -20
```

Expected: 无错误

- [ ] **Step 2: 运行测试**

```bash
cargo test -p openjax-core 2>&1 | tail -30
```

Expected: 所有测试通过

- [ ] **Step 3: 运行 cargo fmt**

```bash
cargo fmt
```

- [ ] **Step 4: 运行 clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -30
```

Expected: 无警告

- [ ] **Step 5: 全局搜索确认零匹配**

```bash
grep -r "apply_patch" --include="*.rs" --include="*.md" . 2>/dev/null | grep -v "target/" | grep -v ".git/" | head -20
```

Expected: 无输出（零匹配）

```bash
grep -ri "apply.?patch" --include="*.rs" --include="*.md" . 2>/dev/null | grep -v "target/" | grep -v ".git/" | head -20
```

Expected: 无输出（零匹配）

- [ ] **Step 6: 如有格式化变更，提交**

```bash
git diff --stat
```

如有变更:
```bash
git add -A
git commit -m "🎨 style: cargo fmt 格式化

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## 完成总结

实施完成后，代码库应满足:
1. 所有 `apply_patch` 实现代码已删除
2. 所有 `apply_patch` 配置和枚举已删除
3. 所有 `apply_patch` 测试已删除
4. 所有 `apply_patch` 文档引用已清理
5. `cargo check`、`cargo test`、`cargo clippy` 全部通过
6. 全局搜索 `apply_patch` 返回零匹配
