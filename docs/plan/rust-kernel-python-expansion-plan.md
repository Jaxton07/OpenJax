# OpenJax Rust 内核 + Python 外层扩展执行计划

本文档用于指导 OpenJax 从“Rust CLI 主导”演进为“Rust 内核 + Python 外层能力”的分层架构。目标是保留 Rust 在工具调用、沙箱、安全边界上的稳定性，同时提升 Python 在 TUI、Bot 集成、生态扩展上的迭代效率。

## 1. 目标与范围

### 1.1 总体目标

1. Rust 负责内核能力：Agent Loop、Tool Router、Sandbox、Approval、Model Client。
2. Python 负责外层能力：TUI、Telegram Bot、后续第三方平台集成。
3. 通过稳定跨语言协议连接 Rust 与 Python，避免业务逻辑散落在双端。

### 1.2 非目标（当前阶段不做）

1. 不在第一阶段将全部 Rust CLI 功能立即迁移到 Python。
2. 不引入多套并行协议（仅保留 1 套主协议）。
3. 不在 MVP 阶段支持多 Agent 并发编排（先单会话跑通）。

## 2. 目标架构

1. `openjax-core`（Rust）：内核库，保持现有核心逻辑。
2. `openjaxd`（Rust，新建）：Daemon 进程，提供 JSON-RPC/stdio 或流式事件协议。
3. `openjax-python-sdk`（Python，新建）：协议封装、会话管理、事件订阅。
4. `openjax-python-tui`（Python，新建）：基于 SDK 的交互 UI。
5. `openjax-python-telegram`（Python，新建）：基于 SDK 的 Telegram 适配层。

## 3. 分阶段执行计划

### 阶段 0：基线冻结与接口盘点

1. 梳理 `openjax-protocol` 中 `Op/Event` 的当前语义。
2. 梳理 `openjax-core::Agent::submit_with_sink` 的事件时序与状态约束。
3. 明确审批、工具调用、错误输出在协议层的统一表示。
4. 输出一份“协议冻结清单（v1）”到 `docs/`（便于 Python 侧并行开发）。

验收标准：
1. 形成 v1 协议草案，包含字段、必填项、错误码、版本号策略。
2. Rust 与 Python 两侧对同一事件序列有一致理解。

### 阶段 1：定义跨语言协议（v1）

1. 设计最小 API：
2. `start_session`
3. `submit_turn`
4. `stream_events`
5. `resolve_approval`
6. `shutdown_session`
7. 给每个请求定义 `request_id/session_id/turn_id`，确保可追踪。
8. 建立错误模型：`code/message/retriable/details`。
9. 规定向前兼容规则：新增字段不破坏旧客户端、移除字段必须升 major。

交付物：
1. `docs/` 下协议文档（含时序图与示例 payload）。
2. 协议 JSON Schema（可用于 Rust/Python 自动校验）。

验收标准：
1. 协议文档可直接指导双方开发，无需口头补充。
2. Schema 可校验核心消息（请求、事件、错误）且通过样例测试。

### 阶段 2：实现 Rust Daemon（openjaxd）

1. 新增 crate：`openjaxd`。
2. 将 `openjax-core` 作为内部依赖，进程内持有 Agent 实例。
3. 实现 stdio 双向通信（首选），采用 JSON 行分帧或 length-prefix（二选一，统一全局）。
4. 实现会话生命周期与资源回收（包含超时与异常退出）。
5. 打通审批闭环：Rust 发 `ApprovalRequested`，等待 Python 回 `resolve_approval`。
6. 输出结构化日志，便于 Python 侧排障。

测试：
1. `cargo test -p openjax-core`
2. `cargo test -p openjaxd`（新增）
3. 增加协议集成测试：模拟客户端请求并断言事件序列。

验收标准：
1. 单会话可完整执行：用户输入 -> 规划 -> 工具调用 -> 审批 -> 最终输出。
2. Daemon 异常退出时不会遗留挂起状态或僵尸子进程。

### 阶段 3：实现 Python SDK（MVP）

1. 新建 Python 包（建议 `python/openjax_sdk`）。
2. 封装 Daemon 管理：启动、健康检查、退出、重连。
3. 封装同步/异步两套客户端接口（至少保证 async 可用）。
4. 提供事件分发器：按 `turn_id` 聚合 `AssistantDelta`，对外暴露回调或 async iterator。
5. 提供审批接口：将 UI/Bot 决策标准化为 `resolve_approval`。
6. 增加类型层：Pydantic/dataclass（至少二选一）约束协议对象。

测试：
1. Python 单元测试（协议序列化、事件聚合、异常处理）。
2. Python 集成测试（拉起真实 `openjaxd` 端到端通信）。

验收标准：
1. SDK 可独立作为“无 UI 客户端”跑通完整 turn。
2. 审批与增量输出在 Python 侧行为与 Rust TUI 当前体验一致。

### 阶段 4：Python TUI 迁移与对齐

1. 基于 SDK 构建 Python TUI（先实现单会话）。
2. 对齐当前 `tui_next` 的关键能力：
3. 输入提交与增量输出合并。
4. 审批弹层（y/n 或快捷键）。
5. 终端恢复保障（异常退出恢复 raw mode）。
6. 保留 `tui_next`（Rust）作为回退实现，直到 Python 版本稳定。

测试：
1. 关键交互路径 E2E（输入、审批、取消、退出）。
2. 在 tmux/zellij 下执行稳定性回归。

验收标准：
1. Python TUI 核心体验不弱于当前 Rust TUI。
2. 出现异常时可恢复终端状态，且无卡死。

### 阶段 5：Telegram Bot 集成（首个外部扩展）

1. 新建 `python/openjax_telegram` 模块。
2. 将 Telegram 消息映射为 session/turn（用户维度隔离会话）。
3. 定义审批策略映射：
4. 自动拒绝高风险操作（可配置）。
5. 管理员审批（可配置）。
6. 回传模型输出与工具执行摘要。
7. 增加速率限制与重试机制，防止 bot 被刷。

测试：
1. 沙盒环境 Bot 联调。
2. 异常场景：网络闪断、重复 webhook、超时重试。

验收标准：
1. Bot 可稳定执行多轮对话与工具调用。
2. 审批与安全策略可配置且默认保守。

### 阶段 6：发布与治理

1. CI 分层：
2. Rust（build/test）
3. Python（lint/test/type check）
4. 跨语言协议集成测试
5. 增加兼容性矩阵：Rust core 版本 vs Python SDK 版本。
6. 制定版本策略：协议版本与 SDK 版本绑定规则。

验收标准：
1. 任一协议变更都能触发跨语言回归测试。
2. 发布流程中可提前发现不兼容变更。

## 4. 里程碑与建议工期

1. M1（1 周）：阶段 0-1 完成（协议冻结 + schema）。
2. M2（1-2 周）：阶段 2 完成（openjaxd 可用）。
3. M3（1-2 周）：阶段 3 完成（Python SDK MVP）。
4. M4（1-2 周）：阶段 4 完成（Python TUI 可替代）。
5. M5（1 周）：阶段 5 完成（Telegram Bot MVP）。

## 5. 风险与缓解

1. 风险：协议频繁变化导致双端反复改动。
2. 缓解：先冻结 v1，后续只做向前兼容新增。
3. 风险：审批交互卡死（Rust 等待，Python 未响应）。
4. 缓解：加审批超时、取消机制、会话级 watchdog。
5. 风险：跨语言调试成本高。
6. 缓解：统一 request_id/turn_id 日志链路，提供 debug 模式。

## 6. 执行完成后的文档更新清单（强制）

在 **Python 集成完成（至少到阶段 4）后**，必须同步更新以下文档，避免架构认知滞后：

1. 更新 `AGENTS.md`
2. 补充新的项目结构与开发命令（Rust + Python 双栈）。
3. 补充 Python 扩展层职责边界（TUI/Bot/外部连接）。
4. 更新 `docs/project-structure-index.md`
5. 把 `openjaxd`、Python SDK、Python TUI、Telegram 模块纳入结构索引。
6. 更新运行与测试路径（Rust 测试 + Python 测试 + 跨语言集成测试）。

## 7. 立即可执行的下一步

1. 建立协议草案文档与 schema 占位文件（阶段 0-1）。
2. 新建 `openjaxd` crate 骨架并打通最小 `submit_turn -> stream_events`。
3. 新建 Python SDK 骨架并完成端到端 hello-turn 测试。
