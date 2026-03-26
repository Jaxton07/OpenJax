# OpenJax 沙箱与权限审批机制重构设计（方案 B）

- 日期：2026-03-23
- 状态：Draft（已与用户确认核心边界）
- 作者：Codex

## 1. 背景与问题

当前权限控制分散在 `openjax-core` 多处逻辑中，存在以下结构性问题：

1. 策略语义耦合：`sandbox_mode` 与 `approval_policy` 在工具编排、shell 运行时、降级流程中交叉生效，难以统一解释与扩展。
2. 分级粒度不足：当前主路径仍以粗粒度模式驱动（如 `workspace_write` / `danger_full_access`），不适合工具类型持续扩张。
3. 配置切换成本高：主要依赖环境变量与启动时读取配置，不利于会话级动态治理与线上应急。
4. 新工具接入门槛不清：缺乏统一“权限声明”契约，存在接入后策略覆盖不完整风险。
5. 可观测性不统一：审计信息与审批理由可见性有提升，但未形成“策略版本 + 规则命中”的完整追溯闭环。

本次重构采用**破坏性变更**，目标是从根本上建立统一权限治理架构。

## 2. 目标与非目标

### 2.1 M1 目标

1. 引入独立模块 `openjax-policy`，作为所有工具执行前的**唯一决策中枢**。
2. 全工具统一接入权限模型：`shell/exec_command/apply_patch/edit_file_range` 与系统工具一致走 `PolicyDecision`。
3. 构建策略中心能力：持久化规则、会话覆盖（overlay）、运行时热更新（versioned publish）。
4. 默认安全姿态采用 `ask by default`。
5. 审批与审计统一输出：`policy_version`、`matched_rule_id`、`decision_reason`。
6. 明确工具接入完成标准：**权限声明 + 权限测试通过**，否则不算接入完成。

### 2.2 非目标（M1 不做）

1. Windows 原生沙箱后端重构。
2. 多节点分布式策略同步。
3. 可视化规则编排 UI。

## 3. 核心原则

1. 单一决策源：权限判定只由 `openjax-policy` 输出。
2. 执行与策略解耦：`openjax-core` 专注执行，策略中枢专注判定。
3. 默认询问：对未知或未声明能力默认 `ask`，不隐式放行。
4. 发布门禁：新工具未完成权限声明或测试，不允许标记“接入完成”。
5. 调用级一致性：单次工具调用绑定一个策略版本，避免执行中策略漂移。

## 4. 总体架构

### 4.1 模块拆分

新增 workspace crate：`openjax-policy`

内部子模块建议：

1. `schema`：规则结构、匹配条件、裁决枚举、元数据定义。
2. `engine`：规则匹配、优先级排序、冲突消解、默认姿态收敛。
3. `store`：策略持久化抽象（初期可 SQLite/本地存储实现）。
4. `overlay`：会话级覆盖规则管理。
5. `runtime`：热更新发布、版本快照、缓存原子切换。
6. `audit`：决策快照结构化输出。

### 4.2 依赖关系

1. `openjax-core` -> `openjax-policy`（决策查询接口）
2. `openjax-gateway` -> `openjax-policy`（策略管理 API、发布 API、会话覆盖 API）
3. `ui/web` -> `openjax-gateway` -> `openjax-core`（保持现有 Web 链路）
4. `ui/tui` -> `openjax-core`（默认链路，不强制依赖 gateway）

### 4.3 UI 接入策略决策（M1）

1. M1 不强制 TUI 经过 gateway，确保 `tui` 启动不依赖额外进程。
2. 策略一致性通过共享 `openjax-policy` 保证，而不是强行统一传输链路。
3. Web 继续通过 gateway 管理会话和事件流，TUI 默认保持本地直连低延迟体验。

## 5. 决策模型

### 5.1 统一输入 `PolicyInput`

建议字段：

1. `tool_name`
2. `action`
3. `session_id`
4. `actor`（用户、系统、子代理等）
5. `resource`（路径、目标对象）
6. `capabilities`（如 `fs_read/fs_write/network/process_exec/env_read/env_write`）
7. `risk_tags`（工具静态声明 + 运行时提取）
8. `context`（cwd、平台、require_escalated、sandbox backend 偏好等）

### 5.2 统一输出 `PolicyDecision`

1. `allow`
2. `ask`
3. `deny`
4. `escalate`

并附带：

1. `policy_version`
2. `matched_rule_id`
3. `reason`
4. `trace`（可选，便于审计和调试）

### 5.3 优先级与冲突消解

执行顺序：

1. 会话覆盖规则（session overlay）
2. 全局规则（global rules）
3. 默认姿态（default posture = ask）
4. 硬拒绝规则（hard deny，最高安全约束，可在实现中前置）

冲突时按：

1. `priority`（数值大者优先）
2. 匹配具体度（更具体条件优先）
3. 同级冲突采用保守裁决（`deny > escalate > ask > allow`）

## 6. 策略中心与热更新

### 6.1 策略版本化

1. 每次 `publish` 生成新 `policy_version`。
2. 新调用读取最新版本快照。
3. in-flight 调用继续使用启动时版本（调用级一致性）。

### 6.2 会话覆盖

1. 以 `session_id` 维度挂载 overlay（可设 TTL 或显式清除）。
2. overlay 优先级高于全局策略。
3. overlay 变更即时生效于新调用。

### 6.3 热更新机制

1. gateway 写入策略后触发 `publish`。
2. policy runtime 原子替换内存快照。
3. core 在每次工具调用前读取快照句柄，避免频繁 IO。

## 7. Core 改造方案

### 7.1 职责收敛

`openjax-core` 仅保留：

1. 构造 `PolicyInput`。
2. 请求 `openjax-policy` 获取 `PolicyDecision`。
3. 按决策执行工具/审批/拒绝。
4. 将决策元数据注入事件与审计。

### 7.2 语义替换

1. 移除旧 `SandboxMode/ApprovalPolicy` 作为核心业务语义。
2. `OPENJAX_SANDBOX_MODE` / `OPENJAX_APPROVAL_POLICY` 不再驱动主决策流程。
3. 保留迁移期日志提示，明确新策略入口。

## 8. Gateway API 契约（M1）

### 8.1 全局策略管理

1. `POST /api/v1/policy/rules`
2. `PUT /api/v1/policy/rules/:rule_id`
3. `DELETE /api/v1/policy/rules/:rule_id`
4. `GET /api/v1/policy/rules`

### 8.2 版本发布与查询

1. `POST /api/v1/policy/publish`
2. `GET /api/v1/policy/version`
3. `GET /api/v1/policy/effective?session_id=...`

### 8.3 会话覆盖

1. `PUT /api/v1/sessions/:session_id/policy-overlay`
2. `DELETE /api/v1/sessions/:session_id/policy-overlay`
3. `GET /api/v1/sessions/:session_id/policy-overlay`

## 9. 工具接入规范（必须落地）

新增“工具接入完成标准”：

1. 必须实现 `PolicyDescriptor`（声明工具动作、能力、风险标签、资源类型）。
2. 必须提供权限测试：
  - `allow` 场景
  - `ask` 或 `escalate` 场景
  - `deny` 场景
3. 未声明 descriptor 的工具默认 `ask`。
4. CI 门禁：缺少 descriptor 或缺少权限测试即失败。

结论：**“实现功能”不等于“接入完成”，通过权限声明与测试门禁才算完成。**

## 10. 审批与审计

审批事件统一追加：

1. `policy_version`
2. `matched_rule_id`
3. `decision`
4. `reason`
5. `risk_tags`

审计日志至少包含：

1. `command_hash` / `tool_input_hash`
2. `policy_version`
3. `matched_rule_id`
4. `decision`
5. `backend`
6. `degrade_reason`

## 11. 测试与验证计划

### 11.1 `openjax-policy` 单元测试

1. 规则匹配正确性。
2. 优先级与冲突消解。
3. 默认 `ask` 收敛。
4. 会话 overlay 覆盖行为。

### 11.2 集成测试（core + gateway）

1. 全工具路径统一触发策略判定。
2. `publish` 后新调用使用新版本。
3. in-flight 调用版本稳定。
4. overlay 对单会话生效，不污染其他会话。

### 11.3 回归测试

1. 原高风险命令仍可被拒绝或升级审批。
2. 现有审批事件链路不回退。
3. TUI/Web 审批展示字段完整。

## 12. 迁移与发布步骤（M1）

1. 引入 `openjax-policy` crate 与最小可用 schema/engine/store/runtime。
2. core 接入统一决策入口（初期可并行写审计，快速验证）。
3. gateway 增加策略 CRUD、publish、overlay API。
4. 切换主决策到 `openjax-policy`，下线旧语义路径。
5. 补齐测试、更新文档与接入流程。

## 13. 风险与缓解

1. 规则误配导致大量 `ask`：
  - 缓解：提供策略 dry-run 与命中预览接口。
2. 热更新引发行为不可解释：
  - 缓解：调用级版本快照 + 审计写入 `policy_version/rule_id`。
3. 工具新增绕过权限：
  - 缓解：descriptor + CI 强门禁。

## 14. 里程碑定义

M1 完成标准：

1. 全工具统一经 `openjax-policy` 判定。
2. 策略中心三能力可用：持久化规则、会话覆盖、热更新发布。
3. 审批与审计具备版本化可追溯能力。
4. 工具文档更新并明确“权限声明是接入完成门槛”。
5. TUI 默认直连 core 能力保持不变，不新增“必须先启动 gateway”的运行前置条件。
