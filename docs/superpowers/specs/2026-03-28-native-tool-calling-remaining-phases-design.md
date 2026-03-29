# Native Tool Calling Remaining Phases Design

> 日期：2026-03-28
> 状态：Phase 4-5 已完成，Phase 6 收尾中
> 基线提交：`4fb54fa2` `fix(core): 收口 native tool calling 剩余阶段回归问题`
> 关联文档：
> - `docs/plan/refactor/tools/native-tool-calling-plan.md`
> - `docs/superpowers/specs/2026-03-28-phase3-native-tool-calling-design.md`
> - `docs/superpowers/plans/2026-03-28-phase3-native-tool-calling.md`
> - `docs/superpowers/plans/2026-03-28-openjax-core-src-tests-split.md`

---

## 1. 文档定位

本文不是对 Native Tool Calling 迁移的重新设计，而是基于 `4fb54fa2` 已完成事实，对剩余收尾阶段给出执行基线。

约束如下：

- Phase 1-5 已视为完成，本轮不回退、不改写其目标定义。
- 继续沿用原计划编号，但当前焦点集中在 Phase 6 收尾。
- 本文只覆盖 `openjax-core` 主链路、相关测试、文档与清理项收口。
- `planner_tool_action.rs` 保持独立执行模块，不作为本轮默认合并目标。
- Phase 4 的 `write_file`、`glob_files` 描述归位已完成，本文保留为设计与验收记录。

旧总计划 `docs/plan/refactor/tools/native-tool-calling-plan.md` 继续保留为历史背景与最初迁移意图参考；从本文件开始，后续 Phase 4-6 以“当前真实代码状态”为准，而不是以旧计划中已经失效的前置假设为准。

---

## 2. 当前基线

截至 `4fb54fa2`，`openjax-core` 内与 Native Tool Calling 直接相关的主链路状态如下：

- agent 主循环已经从旧 JSON planner 路径切换到 native tool calling 路径。
- `planner_stream_flow.rs` 已移除 `DecisionJsonStreamParser`、`parse_model_decision` 以及 fallback-to-complete。
- `planner.rs` 已基于 `build_system_prompt + build_turn_messages + ModelResponse.content` 驱动对话循环。
- `planner_tool_action.rs` 保留为独立模块，并已具备 native 输入执行路径。
- `write_file`、`glob_files` 已作为正式工具能力接入并纳入权限/执行测试。
- 流式工具事件已保留真实 `tool_name`，不会在 `ToolCallArgsDelta` / `ToolCallReady` 阶段丢失名称。
- `tool_use` 轮次不再误发普通 `ResponseTextDelta`。
- 超出预算的 `tool_use` 不再静默截断，而会补发失败/完成事件进行收口。
- shell 结果语义已拆分为 `model_content`、`display_output` 与 `shell_metadata`，模型通道与展示通道职责分离。
- `openjax-core/src/tests.rs` 已拆分为目录模块，测试结构较此前更清晰，但子模块规模仍需关注。

已确认验证状态：

- `zsh -lc "cargo build -p openjax-core"` 通过
- `zsh -lc "cargo test -p openjax-core --no-run"` 通过
- `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"` 通过
- `zsh -lc "cargo test -p openjax-core --lib streaming -- --nocapture"` 通过

尚未确认：

- 剩余旧路径的文档与代码口径是否已完全一致
- Phase 6 文档/清理收尾后的最终验收证据是否已补齐

---

## 3. 剩余问题定义

Phase 5 完成后，Native Tool Calling 在 `openjax-core` 范围内的主路径、工具面和 shell 语义已成立。剩余问题集中在收尾一致性：

### 3.1 文档口径仍需统一

虽然代码主路径已完成迁移，但仍需要统一以下内容：

- `openjax-core` README/模块文档与当前代码现实的一致性
- 旧 JSON planner 残留结构的“非主路径”身份标注
- 回归验证矩阵与“什么算真正收口”的 done state

---

## 4. 设计目标

本轮剩余阶段的目标不再是扩展能力面，而是在 `openjax-core` 范围内完成最短闭环收口：

1. 统一文档与计划口径，确认 native tool calling 为唯一主路径。
2. 对保留旧结构明确“非主路径/历史用途”定位。
3. 补齐收尾验证证据，形成可追溯 done state。

成功标准：

- Native Tool Calling 是 `openjax-core` 默认且唯一主路径。
- Phase 4 工具补充已正式接入，并具备权限与执行测试。
- shell 执行结果对模型与对事件消费者的语义边界清晰。
- README、spec、plan、验证命令对当前架构口径一致。
- 保留下来的旧结构若未删除，已经被明确标注为非主路径，而不是处于模糊状态。

---

## 5. 设计原则

### 5.1 不走兼容性补丁路线

每个阶段都以“把当前主路径语义收正”为目标，不新增长期双轨方案，不为了兼容旧 JSON planner 再引入回退路径。

### 5.2 保持最短路径实现

只收口和 `openjax-core` Native Tool Calling 主链路直接相关的事项，不把本轮扩张成 `gateway`、`ui/tui` 或其他消费侧的大型联动改造计划。

### 5.3 清理必须建立在主路径已经稳定之上

对于已经脱离主路径、且删除不会改变目标行为的旧结构，可以删除；对于虽然过时但仍存在引用或验证价值的结构，只能先标注非主路径身份，不能为了文档整齐强行清除。

### 5.4 保持执行边界稳定

`planner_tool_action.rs` 在当前基线中承载高风险工具执行守卫逻辑，本轮默认保持独立，不把“文件合并”作为完成条件。

---

## 6. 剩余阶段设计

执行顺序曾按以下路径推进（现已执行到 Phase 6）：

`Phase 4 -> Phase 5 -> Phase 6`

设计时原因如下：

- 先补齐工具面，再做主链路数据契约收口，可减少验证返工。
- shell 输出分离属于高风险语义改造，应在工具集合稳定后集中推进。
- 文档与验证收尾最适合作为最终阶段统一完成。

### Phase 4：工具能力补充（已完成）

#### 目标

补齐原迁移计划中仍缺失的工具能力，并将本应属于工具定义的描述归位到 tool spec。

#### 范围

- 新增 `write_file`
- 新增 `glob_files`
- 补齐对应的 handler、spec、注册、权限声明与测试

#### 组件边界

主要修改范围限制在 `tools/` 体系：

- `openjax-core/src/tools/handlers/`
- `openjax-core/src/tools/spec.rs`
- `openjax-core/src/tools/tool_builder.rs`
- 必要时少量触达 `router_impl.rs`
- 对应 `tests/tools_sandbox_*` 相关测试

#### 非目标

- 不重写 planner 主循环
- 不引入新的工具调度策略
- 不把 shell 输出分离混入本阶段

#### 完成定义

- 模型能通过原生 `tools` 参数看到 `write_file` 与 `glob_files`
- 路由层与执行层可稳定执行
- 对新增工具具备 `allow`、`ask`/`escalate`、`deny` 三类权限覆盖

#### 主要风险

- 工具已注册但缺少 `PolicyDescriptor` 或同义权限声明
- schema、handler 入参和事件语义不一致
- 仅补 happy path 测试，导致工具接入并未真正完成

当前状态：`write_file` / `glob_files` 已接入并覆盖权限/执行测试。

### Phase 5：Shell 输出分离（已完成）

#### 目标

彻底分离 shell 工具结果中的“模型消费内容”和“事件/UI 展示内容”，完成 Native Tool Calling 主链路中最关键的数据语义收口。

#### 范围

- 扩展 `ToolExecOutcome` / `ToolCallCompleted`
- shell 工具返回 `model_content`、`display_output`、`shell_metadata`
- planner/native tool execution 路径改为使用干净的 `tool_result` 喂回模型
- 事件侧继续保留展示所需字段，但不再要求从 `output` 文本中反解析结构化信息
- 更新所有依赖该契约的测试

#### 组件边界

主要修改范围：

- `openjax-protocol` 中相关事件定义
- `openjax-core/src/tools/router_impl.rs`
- `openjax-core/src/sandbox/`
- `openjax-core/src/agent/planner.rs`
- `openjax-core/src/agent/planner_tool_action.rs`
- `openjax-core/src/tests/` 和相关 integration suites

#### 非目标

- 不扩展为 UI 渲染层重构
- 不把所有工具统一包装成复杂 metadata 框架
- 不顺手将 `planner_tool_action.rs` 并回 `planner.rs`

#### 完成定义

- 模型上下文里不再混入仅供展示或调试消费的 shell 元数据
- `ToolCallCompleted` 可携带结构化 shell metadata
- planner 追加的 `tool_result` 仅包含模型需要的内容
- 事件消费者不再必须通过解析 `output` 字符串推断 shell 结构信息

#### 主要风险

- 仅改字段名，没有真正切开模型通道与展示通道
- 流式事件、执行事件与 history/tool trace 之间语义不一致
- 回归测试未覆盖失败路径与审批/降级路径

当前状态：`ToolExecOutcome` 与 `ToolCallCompleted` 已承载分离后的 shell 语义，planner/tool_action 使用模型通道字段回写 `tool_result`。

### Phase 6：主链路收尾与清理（当前阶段）

#### 目标

在不扩大重构面的前提下，完成剩余文档、验证、清理项收口，使 `openjax-core` 范围内 Native Tool Calling 迁移进入 done state。

#### 范围

- 统一 README / spec / plan 对当前架构的描述
- 删除或降级标注已不再服务主链路的旧 JSON planner 残留
- 整理剩余验证矩阵
- 明确 `openjax-core` 范围内 Native Tool Calling 迁移完成定义

#### 组件边界

- `openjax-core/README.md`
- `openjax-core/src/agent/README.md` 及相关模块文档
- `docs/plan/refactor/tools/native-tool-calling-plan.md`
- 本轮新增的 remaining phases spec/plan
- `openjax-core` 内仍保留旧 planner 叙述或辅助代码的位置

#### 非目标

- 不进行跨模块联动改造
- 不在本阶段再开启新的大规模测试结构拆分
- 不为了“看起来干净”而删除仍可能被引用的过渡结构

#### 完成定义

- `openjax-core` 内不存在“默认仍走旧 JSON planner”的主路径入口
- 剩余旧结构若保留，文档已明确其非主路径身份
- README、spec、plan、测试命令口径一致
- 已完成可信的 build + focused tests + full tests 验证闭环

#### 主要风险

- 文档清理先于验证完成，导致描述和代码再次脱节
- 为了清理而过度删除旧结构，破坏已有稳定路径
- 测试命令只更新文档，未形成实际可执行验证闭环

---

## 7. 依赖关系

### 7.1 阶段依赖

- Phase 4 依赖已完成的 Phase 3 Native Tool Calling 主循环与 `ModelRequest.tools`
- Phase 5 依赖 Phase 3 的主链路稳定；与 Phase 4 无强耦合，但建议在工具面补齐后执行
- Phase 6 依赖 Phase 4-5 的代码和测试收口结果

### 7.2 代码边界依赖

- `tools/spec.rs` 与 handler/registration 必须始终保持同一语义源，避免 prompt 再次承担工具协议职责
- shell 输出分离涉及 `openjax-protocol` 与 `openjax-core` 协同变更，必须作为同一语义批次推进
- `planner_tool_action.rs` 的守卫逻辑与 `planner.rs` 的编排逻辑虽然相关，但本轮保持模块边界稳定，避免双重高风险改动叠加

---

## 8. 错误处理与清理策略

### 8.1 错误处理原则

- 不引入新的 fallback-to-complete 或 JSON repair 路线
- 对结构化工具执行错误继续使用明确的 `ok/is_error` 语义，而不是重新退回文本推断
- shell metadata 缺失时，应明确为 `None`，而不是通过“半结构化字符串”进行补偿

### 8.2 清理策略

- 已确认脱离主路径且不会影响当前行为的旧 JSON planner 结构，可在 Phase 6 删除
- 仍存在引用、测试用途或后续迁移价值的结构，可暂时保留，但必须在文档中明确其身份
- `planner_tool_action.rs` 不属于本轮默认清理对象

---

## 9. 验证策略

验证采用三层闭环，而不是只以一次 `cargo test` 作为完成依据。

### 9.1 基础编译

至少包括：

- `zsh -lc "cargo build -p openjax-core"`
- `zsh -lc "cargo test -p openjax-core --no-run"`

### 9.2 领域回归

根据变更范围，至少覆盖以下套件中的相关项：

- `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"`
- `zsh -lc "cargo test -p openjax-core --test streaming_suite"`
- `zsh -lc "cargo test -p openjax-core --test approval_suite"`
- `zsh -lc "cargo test -p openjax-core --test approval_events_suite"`
- `zsh -lc "cargo test -p openjax-core --test core_history_suite"`
- 如技能上下文或 prompt 行为受影响，再补 `zsh -lc "cargo test -p openjax-core --test skills_suite"`

### 9.3 最终收口验证

在剩余阶段全部完成后，应补齐：

- `zsh -lc "cargo test -p openjax-core --tests"`
- 如 crate 内单测也有修改，再补 `zsh -lc "cargo test -p openjax-core --lib"`

本文只定义验证要求，不提前声明这些验证已经全部通过。只有在执行阶段实际跑完并记录结果后，才能声称 Native Tool Calling 剩余阶段已完成收口。

---

## 10. Done State

当本设计对应的 Phase 4-6 全部完成时，`openjax-core` 范围内应满足以下条件：

- Native Tool Calling 是唯一主路径。
- `write_file`、`glob_files` 已完成正式接入，并具备权限与执行测试。
- shell 输出已分离为模型消费内容和展示/事件内容两个语义通道。
- `ToolCallCompleted` 的 shell 相关结构信息不再依赖字符串解析。
- `openjax-core` README、相关 spec/plan 与测试命令对当前实现口径一致。
- 保留的旧结构已经被明确标注为非主路径；可以删除的旧结构已被删除。

---

## 11. 后续产物

基于本文，下一步应新增对应的执行计划文档：

- `docs/superpowers/plans/2026-03-28-native-tool-calling-remaining-phases.md`

该计划文档需继续沿用旧编号，拆解为 Phase 4、Phase 5、Phase 6 的可执行任务列表，明确：

- 每阶段涉及文件
- 哪些步骤可单独提交，哪些步骤必须成组落地
- 每阶段最小验证命令
- 全部完成后的最终验证闭环
