# 暂时将 `apply_patch` 从模型可见工具面下线

## 背景

`apply_patch` 当前支持添加、更新、删除、移动和重命名等文件操作，内部在 `openjax-core/src/tools/apply_patch/` 下有约 9 个模块的实现，还包含多级模糊匹配算法。虽然功能完备，但在日常使用中显得过重。我们想先试验一种更简化的工作流：模型单文件编辑用 `Edit`，删除/移动/重命名等操作交给 `shell` 执行。

## 目标

- 将 `apply_patch` 从模型可见的工具列表中移除。
- 让策略、审批、沙箱等层面对 `apply_patch` "无感知"（调用即视为不存在）。
- 保留源码和原有测试文件在仓库中，方便后续快速回滚。
- 不动 `Edit`、`Read`、`shell` 及其他工具。

## 非目标

- 删除 `apply_patch` 的源码或其单元/集成测试。
- 引入新的替代工具。
- 修改 `apply_patch` 内部的行为逻辑。

## 选定方案：`ApplyPatchToolType::Disabled`

我们沿用 `ShellToolType::Disabled` 的已有模式：

1. 给 `ApplyPatchToolType` 枚举新增 `Disabled` 变体。
2. 将 `ToolsConfig::default()` 中的 `apply_patch_tool_type` 设为 `Some(Disabled)`。
3. 在 `build_all_specs` 中：当为 `Disabled` 时，不再将 `apply_patch` 的 spec 加入返回列表。
4. 在 `build_tool_registry_with_config` 中：当为 `Disabled` 时，跳过 `ApplyPatchHandler` 的注册。
5. 清理 prompt、planner 和 tool-guard 中对 `apply_patch` 的所有引用，让模型完全看不到该工具。
6. 暂时将 `apply_patch` 的集成测试从测试套件运行器中跳过。
7. 更新文档列表，反映当前实际对外暴露的工具面。

## 详细改动

### 配置与注册

涉及文件：
- `openjax-core/src/tools/spec.rs`
  - `ApplyPatchToolType` 增加 `Disabled` 变体。
  - `ToolsConfig::default()` 的 `apply_patch_tool_type` 改为 `Some(ApplyPatchToolType::Disabled)`。
  - `build_all_specs` 中仅在非 `Disabled` 时才加入 `apply_patch` spec。
- `openjax-core/src/tools/tool_builder.rs`
  - `build_tool_registry_with_config` 中仅在非 `Disabled` 时才注册 `ApplyPatchHandler`。
  - 更新现有单元测试，断言默认注册表中 **不存在** `apply_patch`。

### Prompt 与决策层

涉及文件：
- `openjax-core/src/agent/prompt.rs`
  - 从 system prompt 和 planner prompt 中删除所有 `apply_patch` 的提及（包括 "for multi-file edits or file operations, use apply_patch" 等）。
  - 改为引导模型：多文件编辑或文件操作（添加/删除/移动/重命名）使用 `shell` 或 `Edit`。
- `openjax-core/src/agent/planner_utils.rs`
  - `extract_tool_target_hint` 的匹配分支中移除 `"apply_patch"`。
  - `is_mutating_tool` 中移除 `"apply_patch"`。
- `openjax-core/src/agent/decision.rs`
  - `canonical_tool_name` 中移除 `"apply_patch"` 的映射，使 JSON planner 模式也视其为不支持的命令。
- `openjax-core/src/agent/tool_guard.rs`
  - 完全删除 `ApplyPatchReadGuard` 及对应枚举。
- `openjax-core/src/agent/planner.rs`
  - 从 `ToolActionContext` 中移除 `apply_patch_read_guard` 字段，并清理所有使用点。

### 拦截器（保留代码）

- `openjax-core/src/tools/apply_patch_interceptor.rs`
  - 代码保持不动。由于 `apply_patch` 已从工具面中注销，该拦截器仅在用户手动于 `shell` 中输入 `apply_patch` 时才会触发，这在实验期内是可接受的。

### 测试

- `openjax-core/tests/tools_sandbox_suite.rs`
  - 注释掉 `mod apply_patch_m4;` 这一行。
- `openjax-core/tests/tools_sandbox/m4_apply_patch.rs`
  - 将整个测试模块加上 `#[ignore = "apply_patch 暂时从模型工具面下线"]`，或注释掉模块内容，方便未来直接恢复。
- `openjax-core/tests/policy_center_suite.rs` 及相关套件
  - 更新任何默认假设 `apply_patch` 存在于工具列表中的断言。

### 文档

- `openjax-core/src/tools/README.md`
  - 从"当前受支持工具面"列表中移除 `apply_patch`。
- `openjax-core/src/tools/docs/tools-list.md`
  - 从支持工具表格中移除 `apply_patch`，但保留文档中关于 `apply_patch/` 子模块架构的详细说明（作为历史/实现参考）。

## 错误处理与回滚

如果模型因缓存上下文等原因仍然尝试调用 `apply_patch`，调用将失败，原因如下：
1. `canonical_tool_name` 不再映射它。
2. `ToolRegistry` 中没有对应的 handler。
返回的错误将是标准的不支持工具错误。

回滚时只需：
1. 把 `ToolsConfig::default().apply_patch_tool_type` 改回 `Some(ApplyPatchToolType::Freeform)` 或 `Some(ApplyPatchToolType::Default)`。
2. 恢复 prompt 中被删除的 `apply_patch` 相关文本。
3. 恢复 `canonical_tool_name` 和 `ApplyPatchReadGuard`。
4. 取消测试的 ignore/注释。

## 测试计划

- `cargo test -p openjax-core --test tools_sandbox_suite` 在不运行 `apply_patch` 测试的情况下通过。
- `cargo test -p openjax-core --test skills_suite` 通过。
- `cargo test -p openjax-core --test approval_suite` 通过。
- 在 `openjax-core/src/agent/` 和 `openjax-core/src/tools/` 范围内做 scoped grep，确认默认不再注册 `apply_patch` 的 spec/handler。

## 验收标准

- `build_default_tool_registry()` 返回的 specs 中不包含 `apply_patch`。
- `build_default_tool_registry()` 未注册 `ApplyPatchHandler`。
- Prompt 中不再将 `apply_patch` 列为可用工具。
- `ApplyPatchReadGuard` 被移除。
- `apply_patch` 集成测试被跳过/未编入活跃套件。
- 其余所有核心测试通过。
