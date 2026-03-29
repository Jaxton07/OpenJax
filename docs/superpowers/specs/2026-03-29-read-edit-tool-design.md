# Read / Edit Tool Redesign

## Background

`openjax-core` 当前对文件编辑主要暴露 `read_file` 和 `edit_file_range`。

现状问题：

- `edit_file_range` 依赖行号定位，模型在多次编辑后容易因行号漂移改错位置。
- system prompt 里对“先读再改”只是偏好提示，不是明确约束。
- TUI / WebUI 已存在把旧工具名映射成 `Read` / `Edit` 的显示层心智，但后端实际暴露名仍是旧名，工具契约不一致。

本设计只处理 `Read` / `Edit` 工具面的重构，不处理 `apply_patch` 的稳定性问题，也不引入 turn/file 级读写状态跟踪。

## Goals

- 对模型暴露统一且稳定的文件读取/编辑工具名：`Read` / `Edit`
- 彻底移除 `edit_file_range` 的对外暴露，避免行号编辑语义继续误导模型
- 将单文件编辑改为基于旧文本唯一匹配的替换语义
- 将“修改已有文件前先读取”从软提示改成 prompt/spec 中的硬规则
- 清理后端、TUI、WebUI 中围绕旧工具名的长期兼容尾巴

## Non-Goals

- 不处理 `apply_patch` 的行为、描述、稳定性或恢复策略
- 不实现文件级“读后才可再写”的状态跟踪 guard
- 不扩展 `Edit` 到新增文件、删除文件、重命名或多文件变更
- 不修改 `Read` 的参数结构，只改工具名

## Selected Approach

采用“最小但完整替换”方案：

- 保留读取工具能力，但将模型可见名称从 `read_file` 改为 `Read`
- 删除 `edit_file_range` 的默认对外暴露，新增 `Edit`
- `Edit` 仅接收 `file_path`、`old_string`、`new_string`
- `Edit` 只在指定文件内执行单次唯一文本替换
- 失败时通过清晰错误类别引导模型先 `Read` 再决定是否重试

不采用行号编辑增强或读写状态机。原因是这两类方案都会把本轮问题扩大化：前者仍保留定位脆弱性，后者需要额外维护 turn-local 文件状态，超出当前最短正确路径。

## Tool Contract

### Read

- 对模型暴露名称：`Read`
- 本轮仅改工具名，不改现有参数结构和读取行为
- `Read` 继续承担“获取目标文件最新内容”的职责

### Edit

- 对模型暴露名称：`Edit`
- 参数：
  - `file_path`
  - `old_string`
  - `new_string`
- 作用：在指定文件内，将唯一匹配到的一段旧文本替换为新文本

匹配规则：

- 先将文件内容与 `old_string` 的换行统一归一化为 `\n`
- 归一化后做精确文本匹配
- 除换行差异外，空格、缩进、标点、引号、尾部空白都必须严格一致

成功条件：

- `old_string` 在指定文件内恰好出现 1 次

失败条件：

- `not_found`
- `not_unique`
- `file_missing`
- `invalid_args`

`Edit` 只处理单文件中的已有文本替换，不承担其他文件系统变更职责。

## Prompt And Spec Rules

`prompt.rs` 与 `spec.rs` 统一改为硬规则，而不是“prefer”。

规则内容：

- 修改已有文件前，先调用 `Read`
- 对单文件已有内容修改，使用 `Edit`
- `Edit` 失败后，先调用 `Read` 获取最新内容，再决定是否重试
- 不要在未知最新文件状态下盲目重复同一编辑尝试

这组规则同时出现在：

- native loop 的 system prompt
- `Read` / `Edit` 的 tool spec description

目标是让模型在 prompt 和 tool description 两个入口看到一致契约，减少行为分叉。

## Return Semantics

### Edit success

- 返回简洁成功确认
- 不返回局部上下文
- 不内嵌 diff 片段

原因：

- `Edit` 的主要职责是可靠修改，不是展示 diff
- 大块代码编辑时回传上下文会增加 token 开销
- 需要查看结果时，模型可显式调用 `Read`

### Edit failure

- 返回结构化失败类别与简洁错误说明
- 失败文案明确提示：先 `Read` 再决定是否重试
- 失败文案不自动推荐其他写类工具

## Architecture Impact

涉及改动面：

- `openjax-core/src/tools/spec.rs`
  - 新增/替换 `Read`、`Edit` 的 spec
  - 删除默认对外暴露的 `edit_file_range`
- `openjax-core/src/tools/tool_builder.rs`
  - 默认工具注册改为 `Read` / `Edit`
- `openjax-core/src/agent/prompt.rs`
  - 将旧工具名和软提示切换为新工具名和硬规则
- `openjax-core/src/tools/handlers/`
  - `Read` 可复用现有读取处理逻辑并清理对外旧名
  - `Edit` 需要新增基于唯一文本匹配的 handler
  - 旧的 `edit_file_range` handler、注册和测试引用一起移除
- TUI / WebUI
  - 移除对旧工具名的人为显示映射
  - 直接展示后端真实工具名 `Read` / `Edit`

## Data Flow

单次编辑链路：

1. 模型调用 `Read` 获取目标文件内容
2. 模型根据读取结果构造 `Edit(file_path, old_string, new_string)`
3. `Edit` 在指定文件内做换行归一化后的唯一匹配
4. 若唯一命中，则执行一次替换并返回成功确认
5. 若失败，则返回失败类别；模型应重新 `Read`

这个链路的重点是把定位依据从“行号”改成“旧文本精确匹配”，从而消除多次编辑时的行号漂移问题。

## Error Handling

`Edit` 应保证失败是确定性的、可恢复的：

- `not_found`
  - 说明模型拿到的旧文本已过期、拼写不一致，或换行外的格式不匹配
- `not_unique`
  - 说明旧文本片段不足以唯一定位，需要模型读取更多上下文并提供更长的 `old_string`
- `file_missing`
  - 说明目标文件不存在或路径错误
- `invalid_args`
  - 说明入参为空、缺失或不合法

错误消息必须可直接驱动下一步行为，但不做超出本工具边界的建议。

## Testing

需要覆盖以下测试：

- `Read` / `Edit` 新工具名出现在默认工具列表中
- prompt 含有 `Read` / `Edit` 的硬规则，不再出现 `read_file` / `edit_file_range` 的旧引导
- `Edit` 成功替换唯一匹配 1 次
- `Edit` 在 `not_found` 时失败
- `Edit` 在 `not_unique` 时失败
- `Edit` 支持换行归一化匹配（文件为 `\r\n`，`old_string` 为 `\n`）
- `Edit` 对空格、缩进、标点不做宽松匹配
- `Edit` 只替换 1 处，不做全局批量替换
- TUI / WebUI 不再依赖旧工具名显示映射

## Migration Notes

- 本轮不保留长期双轨对外名称
- 对模型、prompt、spec、UI 展示统一只使用 `Read` / `Edit`
- 如果内部仍有旧文件名或旧模块名，应在本轮一并清理，不留尾巴到下一期

## Risks

- 模型可能在旧 prompt 或缓存上下文影响下继续尝试旧工具名，因此需要同步清理测试和展示层文本
- 过短的 `old_string` 会更容易命中 `not_unique`，需要在 spec 文案里明确鼓励提供足够长的定位文本
- 不返回 diff 或局部上下文会降低单次工具结果的信息量，但这是为换取更低 token 消耗和更清晰职责边界

## Acceptance Criteria

- 默认工具面只对模型暴露 `Read` / `Edit`
- `edit_file_range` 不再作为默认对外工具出现
- `Edit` 采用 `file_path`、`old_string`、`new_string` 三参数契约
- `Edit` 的成功条件是单文件内唯一文本匹配
- prompt 与 spec 都以硬规则要求“修改已有文件前先 `Read`”
- TUI / WebUI 不再依赖旧工具名映射
