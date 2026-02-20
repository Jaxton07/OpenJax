# OpenJax TUI 技术方向（重构期版）

本文档用于指导 TUI 从当前 Rust 实现逐步迁移到 Python 外层实现。  
目标是先对齐能力，再切换默认入口，避免体验回退。

---

## 1. 当前状态（Rust TUI 已落地）

- `openjax-tui` 已接入 `openjax-core::Agent`，通过 `submit_with_sink` 消费事件流。
- 审批链路可用：
  - 协议事件：`ApprovalRequested` / `ApprovalResolved`
  - UI 弹层展示审批请求
  - `y/n` 决策可回填 core
- 增量输出可用：
  - 协议事件：`AssistantDelta`
  - UI 合并到同一 assistant 消息
- 终端行为基础稳定：
  - raw mode 进入/恢复
  - `OPENJAX_TUI_ALT_SCREEN=auto|always|never`
- 回归测试已覆盖主路径（状态、审批、增量、键位、终端恢复等）

---

## 2. 核心代码地图（Rust 现状）

1. `openjax-tui/src/main.rs`
2. `openjax-tui/src/app.rs`
3. `openjax-tui/src/state.rs`
4. `openjax-tui/src/approval.rs`
5. `openjax-tui/src/tui.rs`

关联模块：
- `openjax-protocol/src/lib.rs`
- `openjax-core/src/lib.rs`
- `openjax-core/src/approval.rs`

---

## 3. 迁移目标（Python TUI）

目标模块：`python/openjax_tui`（规划中）  
依赖通道：`python/openjax_sdk` -> `openjaxd` -> `openjax-core`

迁移原则：
1. Rust TUI 在迁移期持续可用。
2. Python TUI 先达到“能力等价”，再考虑体验增强。
3. 任何阶段都保留可切回 Rust TUI 的运行路径。

---

## 4. Rust TUI -> Python TUI 能力对齐清单

以下项目是阶段 E 的最低对齐基线（全部达成才可切默认）：

- [ ] 输入提交与回车行为一致
- [ ] `AssistantDelta` 合并策略一致（同 turn 同消息聚合）
- [ ] 审批弹层行为一致（显示关键信息 + `y/n` 快捷决策）
- [ ] 工具调用与错误事件可视化不弱于 Rust TUI
- [ ] 终端异常退出可恢复（raw mode/alt screen）
- [ ] 在 tmux/zellij 下基本行为稳定

建议增强项（非切换阻塞）：

- [ ] 多行输入编辑
- [ ] 更完整 Markdown 渲染
- [ ] 长消息滚动和代码块可读性优化

---

## 5. 回退策略（强制）

在 Python TUI 达标前，`openjax-tui` 保持维护并可随时启用。  
发生以下情况时应立即回退到 Rust TUI：

1. 审批链路出现卡死或丢响应
2. 增量输出合并错误导致上下文错乱
3. 终端恢复异常影响用户 shell 状态
4. tmux/zellij 下出现高频交互故障

回退后处理要求：
1. 记录复现步骤与事件日志（request_id/session_id/turn_id）
2. 先修复 SDK/daemon 协议层问题，再恢复 Python TUI 验证

---

## 6. 运行与验证

Rust TUI（当前基线）：

```bash
zsh -lc "cargo run -p openjax-tui"
zsh -lc "cargo test -p openjax-tui"
```

关联回归：

```bash
zsh -lc "cargo test -p openjax-core"
zsh -lc "cargo test -p openjax-cli"
```

---

## 7. 文档索引

- 重构主计划：`docs/plan/refactor/phase-plan-and-todo.md`
- 架构重构计划：`docs/plan/rust-kernel-python-expansion-plan.md`
- 本文档：`docs/tui/technical-direction.md`
