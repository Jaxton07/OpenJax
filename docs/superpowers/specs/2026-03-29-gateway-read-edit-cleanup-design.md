# Gateway Read / Edit Cleanup Design

## Background

`openjax-core` 的公开文件工具已经迁到 `Read` / `Edit`，但仓库内仍有两类旧公开名残留：

- `openjax-gateway` 的活动代码与测试仍引用 `read_file`
- `openjax-core/src/tools/read_file.rs` 仍保留旧实现文件

这轮只做收口清理，不扩展 `Read` / `Edit` 契约，也不处理 `apply_patch`。

## Goals

- 将 `openjax-gateway` 范围内剩余的 `read_file` / `edit_file_range` 公开名引用迁到 `Read` / `Edit`
- 删除 `openjax-core` 中已无调用方的 legacy `read_file.rs`
- 再做一轮 scoped cleanup，确保 `openjax-core` 与 `openjax-gateway` 的活动代码和测试不再暴露旧公开名

## Non-Goals

- 不修改 `Read` / `Edit` 的参数结构或返回语义
- 不处理 `apply_patch` 的稳定性与行为
- 不做额外兼容层或双轨名称暴露
- 不扩展到 `openjax-core` 与 `openjax-gateway` 之外的模块范围

## Selected Approach

采用最短路径收口：

- 在 `openjax-gateway` 里直接将旧公开工具名测试样例、事件摘要样例和策略规则样例改为 `Read` / `Edit`
- 删除 `openjax-core/src/tools/read_file.rs`
- 对 `openjax-core` 与 `openjax-gateway` 做 scoped grep，确认活动代码/测试里不再有旧公开名残留

不保留旧名称兼容，也不添加新的桥接逻辑。原因是当前残留面已经很小，继续兼容只会制造新的长期尾巴。

## Architecture Impact

涉及改动面：

- `openjax-gateway/src/stdio/dispatch.rs`
  - 更新内部测试样例中的工具名摘要
- `openjax-gateway/tests/policy_api/m5_policy_effect.rs`
  - 更新策略规则与 turn 输入中的工具名
- `openjax-core/src/tools/`
  - 删除未使用的 `read_file.rs`
  - 如有 module/export 残留，同步清理

## Testing

需要覆盖：

- `openjax-gateway` 相关测试在 `Read` 名称下通过
- `openjax-core` 工具相关测试在删除 `read_file.rs` 后仍通过
- scoped grep 结果中，`openjax-core` 与 `openjax-gateway` 的活动代码/测试不再出现 `read_file` / `edit_file_range` 旧公开名

## Acceptance Criteria

- `openjax-gateway` 活动代码和测试不再使用 `read_file` / `edit_file_range` 作为公开工具名
- `openjax-core/src/tools/read_file.rs` 被删除且不影响构建测试
- `openjax-core` 与 `openjax-gateway` 范围内不再保留旧公开名尾巴
