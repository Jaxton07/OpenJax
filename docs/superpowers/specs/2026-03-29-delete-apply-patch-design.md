# 彻底删除 apply_patch 工具设计文档

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 从代码、配置、测试、文档所有层面彻底删除 `apply_patch` 工具，不留任何残留引用。

**Architecture:** 删除 `openjax-core/src/tools/apply_patch/` 整个目录及其 handler、interceptor，清理 `spec.rs` 和 `tool_builder.rs` 中的相关配置项和枚举，删除所有测试文件，清理所有活跃和历史文档中的引用。

**Tech Stack:** Rust

---

## 文件结构变更

### 待删除的文件/目录

| 路径 | 说明 |
|------|------|
| `openjax-core/src/tools/apply_patch/` | 整个目录包含 9 个文件 |
| `openjax-core/src/tools/handlers/apply_patch.rs` | Handler 实现 |
| `openjax-core/src/tools/apply_patch_interceptor.rs` | Interceptor |
| `openjax-core/tests/tools_sandbox/m4_apply_patch.rs` | 测试文件 |

### 待修改的文件

| 路径 | 修改内容 |
|------|---------|
| `openjax-core/src/tools/mod.rs` | 删除 `pub mod apply_patch`、`pub mod apply_patch_interceptor`、相关 `pub use` |
| `openjax-core/src/tools/handlers/mod.rs` | 删除 `pub mod apply_patch`、`pub use apply_patch::ApplyPatchHandler` |
| `openjax-core/src/tools/spec.rs` | 删除 `ApplyPatchToolType` 枚举、`APPLY_PATCH_FORMAT_DETAIL`、两个 `create_apply_patch*_spec()` 函数、相关测试 |
| `openjax-core/src/tools/tool_builder.rs` | 删除 `ApplyPatchHandler` 导入、条件注册块、`apply_patch` 相关单元测试 |
| `openjax-core/src/tools/router.rs` | 删除 `parse_tool_call` 测试中 3 个 `apply_patch` 用例 |
| `openjax-core/src/tools/router_impl.rs` | 检查并清理任何 `apply_patch` 引用 |
| `openjax-core/src/tools/orchestrator.rs` | 检查并清理任何 `apply_patch` 引用 |
| `openjax-core/tests/tools_sandbox_suite.rs` | 删除 `apply_patch` 相关的注释/引用 |
| `openjax-core/README.md` | 清理残留引用 |
| `openjax-core/src/tools/README.md` | 清理工具列表和目录说明 |
| `openjax-core/src/tools/docs/tools-list.md` | 清理 |
| `openjax-core/src/tools/docs/usage-guide.md` | 清理 |
| `openjax-core/src/tools/docs/architecture.md` | 清理 |
| `openjax-core/src/tools/docs/references.md` | 清理 |
| `openjax-core/src/tools/docs/core-components.md` | 清理 |
| `docs/superpowers/specs/2026-03-29-hide-apply-patch-design.md` | 删除（下线方案设计文档） |
| `docs/superpowers/plans/2026-03-29-hide-apply-patch.md` | 删除（下线方案实施计划） |
| `docs/superpowers/specs/2026-03-29-read-edit-tool-design.md` | 清理 `apply_patch` 引用 |
| `docs/superpowers/specs/2026-03-28-native-tool-calling-remaining-phases-design.md` | 清理 |
| `docs/plan/refactor/tools/native-tool-calling-plan.md` | 清理 |
| `docs/plan/refactor/tools/tool-optimization-plan.md` | 清理 |
| `docs/tools.md` | 清理 |
| `docs/security.md` | 清理 |
| `AGENTS.md` | 清理 |
| `openjax-policy/README.md` | 清理 |
| `docs/archive/core/codex-quick-reference.md` | 清理 |
| `docs/archive/core/codex-architecture-reference.md` | 清理 |
| `docs/archive/core/implementation-plan.md` | 清理 |
| `docs/archive/core/openjax-vs-codex-comparison.md` | 清理 |

## 验证清单

- [ ] `cargo check -p openjax-core` 无错误
- [ ] `cargo check --workspace` 无错误
- [ ] `cargo test -p openjax-core` 通过
- [ ] `cargo fmt` 无变更
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [ ] 全局搜索 `apply_patch` 零匹配
- [ ] 全局搜索 `apply.?patch`（不区分大小写）零匹配
