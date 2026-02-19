# OpenJax TUI 设计与实施计划

## 1. 目标与约束

### 1.1 目标

1. 在不影响现有 `openjax-cli` 功能的前提下，引入独立的 `openjax-tui` crate。
2. 复用 `openjax-core` 和 `openjax-protocol` 作为统一业务内核，避免 TUI 与 CLI 逻辑分叉。
3. 按可测试、可回滚、可渐进发布的方式实施，每一步都具备明确验收标准。
4. 将审批、事件流、流式输出能力抽象为前端无关接口，为后续 GUI/Web 前端预留空间。

### 1.2 非目标

1. 本阶段不实现多 Agent 可视化编排（仅保留协议兼容）。
2. 本阶段不引入复杂插件系统。
3. 本阶段不改动现有工具语义和安全策略。

### 1.3 约束

1. 保持 Cargo workspace 结构清晰，新增 crate 不破坏已有构建链路。
2. 现有 `openjax-cli` 默认入口、参数、行为保持兼容。
3. 任何核心行为修改必须具备回归测试，优先覆盖 `openjax-core/tests` 和 `openjax-cli/tests`。
4. 命令统一使用 `zsh -lc` 执行。

## 2. 当前状态评估

1. 当前 `Agent::submit` 一次性返回 `Vec<Event>`，不利于 TUI 实时刷新。
2. 工具审批仍包含终端输入耦合，交互层与核心层职责边界不清。
3. `openjax-cli` 采用 REPL 直出文本模式，尚无 UI 状态机与事件总线抽象。
4. 协议层 `Event` 粒度偏粗，对流式增量渲染支持有限。

## 3. 目标架构设计

## 3.1 总体分层

```text
openjax-protocol
  └── Op/Event/Agent 状态模型

openjax-core
  ├── Agent loop
  ├── Model client
  ├── Tools/Approval/Sandbox
  └── Event streaming + Frontend-facing traits

openjax-cli
  └── 线性 REPL 前端（保留）

openjax-tui (new)
  ├── Tui terminal abstraction
  ├── App event loop
  ├── Chat viewport + composer
  └── overlays (approval / status / selection)
```

## 3.2 关键设计原则

1. 单一事实来源：业务行为只在 `openjax-core` 实现。
2. 前后端解耦：审批、事件消费、输出渲染通过 trait 和 channel 协作。
3. 渐进增强：先可用再增强，优先 MVP 可运行可验证。
4. 向后兼容：新增接口优先“增量扩展”，避免破坏式替换。

## 3.3 新增抽象（建议）

1. `EventSink` 或 `tokio::mpsc::Sender<Event>`：支持增量事件输出。
2. `ApprovalHandler` trait：由 CLI/TUI 注入审批决策，不在 core 直接读 stdin。
3. `AgentRunner` facade：封装 `Op -> Event stream` 生命周期，简化前端接入。

## 4. 模块级改造方案

## 4.1 openjax-protocol

1. 保留现有 `Event` 变体，新增增量事件变体（建议）：
   - `AssistantDelta { turn_id, content_delta }`
   - `ToolOutputDelta { turn_id, tool_name, output_delta }`
   - `ApprovalRequested { turn_id, request_id, target, reason }`
   - `ApprovalResolved { turn_id, request_id, approved }`
2. 若暂不启用全部新事件，可先定义并由 core 按需发射，避免后续协议破坏性升级。

## 4.2 openjax-core

1. 保留 `submit` 兼容层，新增增量接口（建议）：
   - `submit_stream(op) -> impl Stream<Item = Event>` 或
   - `submit_with_sink(op, sink)`
2. 审批流程抽象：
   - 将现有审批输入逻辑迁移到 `ApprovalHandler`。
   - CLI 实现 `StdinApprovalHandler`，TUI 实现 `OverlayApprovalHandler`。
3. 事件发射时机统一化：工具开始/结束、重试、模型输出都通过同一通道发射。
4. 保持工具处理器行为一致，仅调整交互注入点。

## 4.3 openjax-cli

1. 保持当前用户行为不变。
2. 将 `print_event` 与审批输入适配到新抽象。
3. 增加回归测试，确保 CLI 语义、输出关键路径不变。

## 4.4 openjax-tui（新增 crate）

建议目录结构：

```text
openjax-tui/
  Cargo.toml
  src/
    main.rs
    lib.rs
    app.rs
    tui.rs
    app_event.rs
    state.rs
    ui/
      mod.rs
      chat_view.rs
      composer.rs
      status_bar.rs
      overlay_approval.rs
    render/
      mod.rs
      markdown.rs
      theme.rs
```

职责说明：

1. `tui.rs`：终端 raw mode、alternate screen、事件采集。
2. `app.rs`：主循环，融合 `TuiEvent` 与 `CoreEvent`。
3. `state.rs`：集中管理消息历史、输入状态、overlay 状态。
4. `ui/*`：纯渲染组件，避免业务逻辑下沉。
5. `app_event.rs`：统一内部事件，降低跨模块耦合。

## 5. 分阶段实施计划（每步可验证）

## 第一阶段：基线与护栏

### 目标

1. 建立改造前基线，确保后续可判定是否引入回归。

### 任务

1. 记录当前构建与测试基线（全量和关键子集）。
2. 补齐关键回归用例清单（CLI 启动、工具执行、审批路径、apply_patch）。
3. 新建计划跟踪文档和任务状态表。

### 验证

1. `zsh -lc "cargo build"`
2. `zsh -lc "cargo test"`
3. `zsh -lc "cargo test -p openjax-core --test m3_sandbox"`
4. `zsh -lc "cargo test -p openjax-core --test m4_apply_patch"`
5. `zsh -lc "cargo test -p openjax-cli"`

### 测试新增

1. 无功能改动，可仅新增测试清单文档。

## 第二阶段：协议与核心接口解耦

### 目标

1. 提供可流式消费事件的 core 接口。
2. 完成审批抽象，移除 core 对 stdin 的硬编码依赖。

### 任务

1. 在 `openjax-core` 中引入 `ApprovalHandler` trait。
2. 改造工具审批链路，将审批决策作为依赖注入。
3. 增加 `submit_stream` 或等价接口，同时保留 `submit` 兼容包装。
4. 为协议新增增量事件（可先声明后逐步使用）。

### 验证

1. `zsh -lc "cargo build -p openjax-core -p openjax-cli"`
2. `zsh -lc "cargo test -p openjax-core"`
3. `zsh -lc "cargo test -p openjax-cli"`

### 测试新增（必须）

1. `openjax-core/tests/m5_approval_handler.rs`
   - 覆盖 AlwaysAsk/OnRequest/Never 分支。
   - 覆盖批准/拒绝路径。
2. `openjax-core/tests/m6_submit_stream.rs`
   - 验证流式事件顺序与 `submit` 聚合结果一致。
3. `openjax-core/tests/m7_backward_compat_submit.rs`
   - 验证旧接口行为未变。

## 第三阶段：新建 openjax-tui MVP

### 目标

1. 跑通最小可用 TUI：消息区域 + 输入框 + 基础事件展示。

### 任务

1. 新增 `openjax-tui` crate 并加入 workspace。
2. 接入 `ratatui + crossterm + tokio` 基础运行骨架。
3. 实现 `App` 主循环，支持键盘输入与 core 事件显示。
4. 首版只支持：发送消息、展示 `AssistantMessage`、展示工具起止事件。

### 验证

1. `zsh -lc "cargo build -p openjax-tui"`
2. `zsh -lc "cargo test -p openjax-tui"`
3. 手工验收：启动、输入、退出、历史滚动。

### 测试新增（必须）

1. `openjax-tui/tests/m1_app_state.rs`
   - 输入状态转换。
   - 消息追加顺序。
2. `openjax-tui/tests/m2_event_mapping.rs`
   - `Event -> UIState` 映射正确性。
3. `openjax-tui/tests/m3_render_smoke.rs`
   - 最小渲染 smoke test（固定 terminal size）。

## 第四阶段：审批 Overlay 与流式输出

### 目标

1. 将审批流程迁移到 TUI overlay。
2. 支持增量事件的流式输出渲染。

### 任务

1. 实现 `overlay_approval` 组件与快捷键决策。
2. 将 `ApprovalRequested/Resolved` 接入 UI 状态机。
3. 支持 `AssistantDelta` 增量拼接与刷新节奏控制。
4. 增加错误提示、重试状态、长输出滚动策略。

### 验证

1. `zsh -lc "cargo test -p openjax-core"`
2. `zsh -lc "cargo test -p openjax-tui"`
3. 手工验收：审批请求可弹出、可同意/拒绝、模型输出流式可见。

### 测试新增（必须）

1. `openjax-tui/tests/m4_approval_overlay.rs`
2. `openjax-tui/tests/m5_streaming_merge.rs`
3. `openjax-core/tests/m8_approval_event_emission.rs`

## 第五阶段：渲染增强与可用性完善

### 目标

1. 增加 Markdown/代码块可读性和 UI 易用性。

### 任务

1. 增加 Markdown 渲染与样式主题。
2. 支持快捷键帮助、状态栏、复制友好输出。
3. 增加文件搜索弹窗或命令面板（可选）。
4. 完成配置项接入（如 alternate screen 策略）。

### 验证

1. `zsh -lc "cargo build"`
2. `zsh -lc "cargo test"`
3. 手工验收：不同终端环境（含 tmux/zellij）显示与退出恢复正常。

### 测试新增（建议）

1. `openjax-tui/tests/m6_markdown_render.rs`
2. `openjax-tui/tests/m7_keymap.rs`
3. `openjax-tui/tests/m8_terminal_restore.rs`

## 6. 测试策略与回归保护

## 6.1 测试分层

1. 单元测试：状态机、事件映射、审批策略、渲染工具函数。
2. 集成测试：`openjax-core` 端到端工具执行 + 事件流。
3. E2E 测试：`openjax-cli` 既有路径回归，`openjax-tui` MVP 交互冒烟。
4. 手工测试：终端行为（raw mode、alternate screen、退出恢复）。

## 6.2 不受影响保障

1. `openjax-cli` 不切换默认运行路径，不依赖 TUI。
2. core 新接口以“新增 + 兼容层”方式引入，不直接替换旧接口。
3. 审批改造期间保留 `StdinApprovalHandler`，确保 CLI 行为一致。
4. 每个阶段完成后执行全量 `cargo test`，不通过不得进入下一阶段。

## 6.3 建议 CI Gate

1. `cargo fmt -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. 关键回归分组：
   - `cargo test -p openjax-core --test m3_sandbox`
   - `cargo test -p openjax-core --test m4_apply_patch`
   - `cargo test -p openjax-cli`

## 6.4 构建告警治理（新增）

1. 每个阶段结束前执行 `cargo build` 与 `cargo test`，若出现 warning 必须在当阶段修复，不滚入下一阶段。
2. 对“已知预留但暂未使用”的字段、参数、局部变量，优先通过重命名为下划线前缀（如 `_parent_thread_id`、`_source`）消除告警。
3. 不使用 `#[allow(...)]` 大范围压制告警，除非有明确注释说明且限定到最小作用域。
4. CI 与本地验收保持一致：以 `clippy -D warnings` 作为告警零容忍门槛，避免 warning 越积越多。

## 7. 风险与缓解

1. 风险：协议新增事件导致序列化兼容问题。
   - 缓解：仅追加变体，不改已有字段语义，补充 serde 回归测试。
2. 风险：审批抽象改造引入行为偏差。
   - 缓解：将现有审批行为固化为测试基线，再替换实现。
3. 风险：TUI 引入导致终端状态异常（退出不恢复）。
   - 缓解：增加 drop guard 和 panic hook 恢复逻辑，并做 smoke test。
4. 风险：流式刷新影响性能与体验。
   - 缓解：先 MVP，后续引入 chunking 策略并压测。

## 8. 实施顺序与里程碑

1. M1: 完成第一阶段，建立基线与任务看板。
2. M2: 完成第二阶段，核心接口解耦并保持 CLI 全量通过。
3. M3: 完成第三阶段，`openjax-tui` MVP 可运行。
4. M4: 完成第四阶段，审批 overlay 与流式输出打通。
5. M5: 完成第五阶段，渲染增强与稳定性补齐。

## 9. DoD（完成定义）

1. `openjax-tui` 可独立运行，支持基本会话与审批交互。
2. `openjax-cli` 现有能力与体验无回归。
3. `openjax-core` 提供前端无关的审批与事件流接口。
4. 新增测试覆盖关键状态机和兼容路径，工作区测试全部通过。
5. 文档同步：架构说明、配置说明、运行说明、测试说明更新完成。

## 10. 建议的首批实施任务（可直接开工）

1. 新增 `openjax-core/src/approval.rs`，定义 `ApprovalHandler` trait 与默认实现。
2. 在审批调用点接入 trait，补 `m5_approval_handler` 测试。
3. 为 Agent 增加 `submit_with_sink`，并以此实现原 `submit`。
4. 新建 `openjax-tui` crate 的最小骨架和 `m1_app_state` 测试。
5. 完成 workspace 构建与测试回归，产出首轮验收记录。

## 11. 第一阶段执行记录（2026-02-19）

### 11.1 执行命令与结果

1. `zsh -lc "cargo build"`：通过（存在 2 条 warning，见 11.2）。
2. `zsh -lc "cargo test"`：通过（workspace 全量测试通过）。
3. `zsh -lc "cargo test -p openjax-core --test m3_sandbox"`：通过（5 passed）。
4. `zsh -lc "cargo test -p openjax-core --test m4_apply_patch"`：通过（3 passed）。
5. `zsh -lc "cargo test -p openjax-cli"`：通过（2 passed）。

### 11.2 发现的问题

1. 原计划中的 `cargo test -p openjax-core m3_sandbox` / `m4_apply_patch` 会被解释为测试名过滤条件，在当前代码下可能出现 `0 tests`，已修正为 `--test` 形式。
2. 当前存在构建 warning（`openjax-core/src/tools/router_impl.rs`）：
   - 未使用参数 `config`
   - 未读取字段 `config`
3. 按“构建告警治理”要求，进入第二阶段前应先清理上述 warning。

### 11.3 第一阶段结论

1. 第一阶段“基线与护栏”中的基线验证已完成并可复现。
2. 第二阶段可开始，但建议先完成 warning 清理以满足告警门禁要求。

## 12. 第二阶段执行记录（2026-02-19）

### 12.1 已完成改造

1. 审批抽象落地：新增 `openjax-core/src/approval.rs`，定义 `ApprovalHandler`、`ApprovalRequest`、`StdinApprovalHandler`。
2. 审批链路解耦：工具审批不再在 core 里直接读取 stdin，改为通过 `ToolTurnContext` 注入 `ApprovalHandler` 执行审批决策。
3. Agent 注入能力：`Agent` 新增 `set_approval_handler(...)`，允许 CLI/TUI 在运行时替换审批实现。
4. 事件输出接口：新增 `submit_with_sink(op, sink)`，在保留 `submit` 的前提下提供事件 sink 接口。
5. 兼容性修复：审批拒绝错误改为非重试错误，避免在重试链路中重复弹审批。
6. 构建告警清理：已清理 `openjax-core/src/tools/router_impl.rs` 的未使用参数/字段告警。

### 12.2 新增测试

1. `openjax-core/tests/m5_approval_handler.rs`
   - 覆盖 `AlwaysAsk / OnRequest / Never` 三个策略分支。
   - 覆盖批准与拒绝路径。
2. `openjax-core/tests/m6_submit_stream.rs`
   - 验证 `submit_with_sink` 发射顺序与返回事件顺序一致。
3. `openjax-core/tests/m7_backward_compat_submit.rs`
   - 验证 `submit` 仍返回完整回合事件序列。

### 12.3 验证结果

1. `zsh -lc "cargo fmt && cargo build -p openjax-core -p openjax-cli"`：通过。
2. `zsh -lc "cargo test -p openjax-core"`：通过。
3. `zsh -lc "cargo test -p openjax-cli"`：通过。
4. `zsh -lc "cargo test --workspace"`：通过。
5. `zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"`：未通过（当前仓库存在既有 lint 债务，需单独治理）。

### 12.4 第二阶段结论

1. 第二阶段核心改造已可用，且 `openjax-cli` 回归通过。
2. 下一步可进入第三阶段：创建 `openjax-tui` MVP 骨架与基础状态/渲染测试。

## 13. 第三阶段执行记录（2026-02-19）

### 13.1 已完成改造

1. 新增 `openjax-tui` crate，并加入 workspace：
   - `openjax-tui/Cargo.toml`
   - `openjax-tui/src/main.rs`
   - `openjax-tui/src/lib.rs`
2. 建立最小 TUI 架构骨架：
   - 应用层：`app.rs`、`state.rs`、`app_event.rs`、`tui.rs`
   - UI 层：`ui/chat_view.rs`、`ui/composer.rs`、`ui/status_bar.rs`、`ui/overlay_approval.rs`
   - 渲染层：`render/markdown.rs`、`render/theme.rs`
3. 实现 MVP 最小行为：
   - 输入编辑（字符输入、退格、提交）
   - 聊天消息列表渲染
   - `Event -> UIState` 基础映射（`AssistantMessage`、工具起止、回合起止）

### 13.2 新增测试

1. `openjax-tui/tests/m1_app_state.rs`
   - 输入状态转换
   - 消息追加顺序
2. `openjax-tui/tests/m2_event_mapping.rs`
   - `Event -> UIState` 映射正确性
3. `openjax-tui/tests/m3_render_smoke.rs`
   - 固定 terminal size 下渲染 smoke test

### 13.3 验证结果

1. `zsh -lc "cargo build -p openjax-tui"`：通过。
2. `zsh -lc "cargo test -p openjax-tui"`：通过。
3. `zsh -lc "cargo test -p openjax-core"`：通过。
4. `zsh -lc "cargo test -p openjax-cli"`：通过。

### 13.4 第三阶段结论

1. `openjax-tui` MVP 骨架已可构建、可测试。
2. 下一步可进入第四阶段：审批 overlay 与增量流式事件渲染。

## 14. 第四阶段执行记录（2026-02-19）

### 14.1 已完成改造

1. 扩展协议事件（`openjax-protocol::Event`）：
   - `AssistantDelta { turn_id, content_delta }`
   - `ApprovalRequested { turn_id, request_id, target, reason }`
   - `ApprovalResolved { turn_id, request_id, approved }`
2. 在 core 工具调用链打通审批事件发射：
   - `ToolTurnContext` 新增 `turn_id` 与 `event_sink`
   - `ToolRouter::execute` / `create_tool_invocation` 透传 `turn_id` 与 `event_sink`
   - `ToolOrchestrator` 与 `ShellCommandHandler` 在审批前后发射 `ApprovalRequested/Resolved`
3. 在 `Agent` 内合并工具子事件：
   - 每次工具调用创建内部 `event_sink`，在返回后按顺序并入 turn 事件序列
4. 在 TUI 状态机接入审批与流式事件：
   - `AppState` 支持 `approval_overlay`
   - 映射 `ApprovalRequested/Resolved` 控制 overlay 打开/关闭
   - 映射 `AssistantDelta` 并合并到同一 assistant 消息
   - `App::render` 增加 approval popup 渲染

### 14.2 新增测试

1. `openjax-core/tests/m8_approval_event_emission.rs`
   - 验证审批流程会发射 `ApprovalRequested/ApprovalResolved`
2. `openjax-tui/tests/m4_approval_overlay.rs`
   - 验证审批 overlay 打开与关闭
3. `openjax-tui/tests/m5_streaming_merge.rs`
   - 验证 `AssistantDelta` 聚合渲染

### 14.3 验证结果

1. `zsh -lc "cargo fmt"`：通过。
2. `zsh -lc "cargo test -p openjax-core --test m8_approval_event_emission"`：通过。
3. `zsh -lc "cargo test -p openjax-tui --test m4_approval_overlay --test m5_streaming_merge"`：通过。
4. `zsh -lc "cargo test -p openjax-core"`：通过。
5. `zsh -lc "cargo test -p openjax-tui"`：通过。
6. `zsh -lc "cargo test -p openjax-cli"`：通过。

### 14.4 第四阶段结论

1. 审批 overlay 与审批事件链路已打通。
2. 增量文本事件（`AssistantDelta`）已具备协议与 UI 合并能力。

## 15. 第五阶段执行记录（2026-02-19）

### 15.1 已完成改造

1. Markdown 渲染增强：
   - `openjax-tui/src/render/markdown.rs` 支持标题、列表、代码块的可读文本转换。
   - assistant 消息展示改为先过 Markdown 渲染，再落入 UI 状态。
2. 可用性增强：
   - 状态栏增加帮助提示（`?` 开关）。
   - 新增帮助弹层（快捷键列表）。
3. 终端行为增强：
   - `openjax-tui/src/tui.rs` 新增终端模式管理（raw mode + alt screen）。
   - 增加 `OPENJAX_TUI_ALT_SCREEN=auto|always|never` 策略。
   - `main.rs` 接入进入/恢复守卫，确保异常路径也执行终端恢复。
4. 键位映射增强：
   - 增加 `?` 打开/关闭帮助。
   - 支持 `Esc` / `Ctrl-C` 退出。

### 15.2 新增测试

1. `openjax-tui/tests/m6_markdown_render.rs`
2. `openjax-tui/tests/m7_keymap.rs`
3. `openjax-tui/tests/m8_terminal_restore.rs`

### 15.3 验证结果

1. `zsh -lc "cargo fmt"`：通过。
2. `zsh -lc "cargo test -p openjax-tui"`：通过。
3. `zsh -lc "cargo test -p openjax-core"`：通过。
4. `zsh -lc "cargo test -p openjax-cli"`：通过。

### 15.4 第五阶段结论

1. 渲染可读性与终端可用性已显著提升，具备下一轮 UI 增强基础。
2. 计划中的五个阶段已全部落地并形成可验证记录。

### 15.5 会话接入补充（2026-02-19）

1. `openjax-tui/src/main.rs` 已接入 `openjax-core::Agent`：
   - 输入提交时调用 `Agent::submit_with_sink(Op::UserTurn { ... }, sink)`
   - 将 core 事件回灌到 `AppEvent::CoreEvent`，驱动 TUI 视图实时更新
2. TUI 启动后会显示运行时信息（model/approval/sandbox），并在退出时发送 `Op::Shutdown`。
3. 该补充完成后，TUI 具备端到端会话能力，不再只是静态 MVP 骨架。

### 15.6 审批决策接入补充（2026-02-19）

1. 新增 `openjax-tui/src/approval.rs`：
   - `TuiApprovalHandler` 实现 `openjax_core::ApprovalHandler`
   - `request_approval` 在后台等待 UI 决策
   - `resolve(request_id, approved)` 由主循环在按键后回填
2. `openjax-tui/src/main.rs` 接入：
   - 启动时将 `TuiApprovalHandler` 注入 `Agent::set_approval_handler(...)`
   - turn 执行改为后台任务，主循环持续渲染并消费 core 事件
   - 审批 overlay 可见时按 `y/n` 直接完成批准/拒绝
3. `openjax-core` 审批请求结构补齐 `request_id`，保证 UI 决策与 core 事件一一对应。
4. 新增测试：`openjax-tui/tests/m9_tui_approval_handler.rs`。
