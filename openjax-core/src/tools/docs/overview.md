# OpenJax 工具系统概述

本文档描述 OpenJax 工具系统的当前能力、边界与重构期定位。

---

## 1. 定位

OpenJax 工具系统是 Rust 内核中的执行与安全中枢，负责：

1. 工具注册与调度
2. 沙箱与审批策略落地
3. 工具调用生命周期事件输出
4. 为上层（CLI/TUI/Python 外层）提供稳定执行能力

说明：本系统参考 Codex 架构思想，但当前口径是“能力对齐范围”，不是“完全一致体验承诺”。

---

## 2. 核心能力（当前已具备）

- 统一接口抽象：`ToolHandler` trait
- 工具注册与分发：`ToolRegistry`
- 输出格式：Text / Json（含成功标志）
- 工具规范：JSON Schema 描述输入输出
- Hooks：执行前后挂钩（Before/After）
- 沙箱与审批：统一编排管理
- 动态工具：运行时注册支持

---

## 3. 当前模块地图

路径：`openjax-core/src/tools/`

- 核心抽象：`context.rs`、`registry.rs`、`spec.rs`、`error.rs`
- 路由与编排：`router_impl.rs`、`orchestrator.rs`、`sandboxing.rs`
- 事件与钩子：`events.rs`、`hooks.rs`
- 动态扩展：`dynamic.rs`、`tool_builder.rs`
- 工具处理器：`handlers/*.rs`

---

## 4. 重构期能力边界（必须明确）

### 4.1 承诺范围

1. Rust 内核继续作为唯一工具执行入口。
2. Python 外层通过协议调用工具能力，不直接复制工具执行逻辑。
3. 审批与沙箱策略在 Rust 侧统一判定和执行。

### 4.2 暂不承诺项（当前阶段）

1. 不承诺与 Codex 的所有工具特性一一对齐。
2. 不承诺首阶段支持多会话并发工具编排。
3. 不承诺 Python 侧直接替代 Rust 侧安全边界实现。

---

## 5. 与 Rust + Python 重构的关系

在目标架构中：

- `openjax-core` 继续承载工具系统主体。
- `openjaxd`（规划中）作为跨语言访问入口。
- `openjax_sdk`（规划中）只做协议封装与事件分发，不重写工具内核。

---

## 6. 验证建议

```bash
zsh -lc "cargo build -p openjax-core"
zsh -lc "cargo test -p openjax-core"
```

重构阶段建议增加：

1. 协议集成测试（daemon <-> sdk）
2. 审批超时与取消场景测试
3. 高风险命令审批策略回归测试

---

## 7. 相关文档

- `./architecture.md`
- `./core-components.md`
- `/docs/plan/refactor/phase-plan-and-todo.md`
- `/docs/plan/rust-kernel-python-expansion-plan.md`
