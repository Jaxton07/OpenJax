# 硬切执行计划：移除 `approval_policy`，权限审批统一收敛到 Policy Center

## 文档定位
- 路径：`docs/plan/refactor/policy/`
- 文件：`2026-03-23-hard-cut-remove-approval-policy-plan.md`
- 用途：作为独立会话直接执行的主计划文档（含背景、分步实施、测试、审查要求）

## 1. 背景与问题定义
当前系统存在两套语义重叠的审批机制：

1. 运行时审批策略：`always_ask | on_request | never`（`approval_policy`）
2. 策略中心决策：`allow | ask | escalate | deny`（policy center）

并行机制导致：

1. 决策路径分叉，阅读和维护成本高
2. 行为来源不透明（同一结果可能来自不同层）
3. 沙箱拒绝后提权场景难形成单一闭环

本次目标是“硬切”：

1. 彻底删除 `approval_policy` 相关类型、配置、环境变量、逻辑、文档
2. 审批与拒绝完全由 policy center 决策驱动
3. 不保留兼容分支，不引入补丁式 fallback

## 2. 目标状态（硬切后语义）
- 单一决策源：policy center
- 唯一业务决策：`allow | ask | escalate | deny`

执行语义：

1. `allow`：直接执行
2. `ask`：进入审批通道
3. `escalate`：进入提权审批通道（同审批流程，保留语义区分）
4. `deny`：直接拒绝（不可审批放行）

职责边界：

1. `sandbox_mode` 仅负责执行边界（`workspace_write` / `danger_full_access`）
2. 不再存在 `approval_policy` 这一运行时闸门
3. `deny` 始终最高优先级，不可被 full access 绕过

## 3. 影响范围评估（执行前基线）
已扫描命中规模：

1. 全仓命中约 `189` 处（含文档/测试）
2. 生产代码直接受影响文件约 `19` 个（集中在 `openjax-core`，含 `ui/tui`）
3. 测试受影响文件约 `16` 个（另有 `openjax-core/src/tests.rs`）

高影响子系统：

1. core 运行时配置与 agent 构造
2. tool 调度链路与上下文结构体
3. sandbox 审批折叠与 degrade 分支
4. tui 运行时信息展示
5. openjaxd 测试环境变量注入
6. 全套文档（README/AGENTS/CLAUDE/config/tools）

## 4. 分步执行方案（可直接开工）

### 阶段 A：类型与配置硬切（先收敛编译面）
1. 删除 `ApprovalPolicy` 类型与导出
- 删除 `openjax-core/src/tools/context.rs` 中 `ApprovalPolicy` 枚举和 `from_env/as_str`
- 删除 `openjax-core/src/lib.rs` 中 `pub use tools::ApprovalPolicy`

2. 删除运行时配置字段
- 删除 `ToolTurnContext.approval_policy`
- 删除 `ToolRuntimeConfig.approval_policy`
- 删除 `CreateToolInvocationParams.approval_policy`
- 删除 `ToolExecutionRequest` 中 approval_policy 传递

3. 删除 Agent 层参数
- 修改 `Agent::with_runtime(...)`、`Agent::with_config_and_runtime(...)` 签名，去掉 approval_policy
- 删除 `approval_policy_name()`
- 修改 `spawn_sub_agent()`，不再传 approval_policy

4. 删除配置解析入口
- 删除 `OPENJAX_APPROVAL_POLICY` 读取和解析（`runtime_policy.rs`）
- 删除 `resolve_approval_policy(...)`
- 删除 `SandboxConfig.approval_policy`
- 删除默认配置模板中的 `approval_policy = "on_request"`

### 阶段 B：决策链路收敛到 policy center
1. 删除基于 approval_policy 的二次折叠
- 删除 `sandbox/policy.rs` 中 `AlwaysAsk/OnRequest/Never` 分支
- 删除 `orchestrator.rs` 中 `approval_reason(..., approval_policy)` 的 override 逻辑

2. 删除 degrade 中 `Never` 特判
- `sandbox/degrade.rs` 不再检查 `ApprovalPolicy::Never`
- degrade 是否可审批仅依据 policy center 决策 + 风险分类

3. 固化统一优先级
1. `deny` 直接拒绝
2. `ask/escalate` 发审批事件
3. 审批通过后继续执行
4. 审批拒绝/超时直接失败

4. 保留 `ask` 与 `escalate` 区分
- 审批流程统一
- 事件中保留 `approval_kind`（`normal` / `escalation`）供 UI/审计使用

### 阶段 C：UI / Daemon / 接口清理
1. TUI 清理
- 删除 runtime info 中 `approval_policy` 显示字段
- 保留 model / sandbox_mode / cwd

2. openjaxd 清理
- 删除测试中 `OPENJAX_APPROVAL_POLICY` 注入
- 改为策略规则前置构造

3. 公共 API 与示例清理
- 对外文档与示例不再出现 `approval_policy` / `OPENJAX_APPROVAL_POLICY`
- 统一改为 policy center 规则驱动

### 阶段 D：测试重构与补齐（必须同批完成）
1. 编译层修复
- 批量修复结构体字段删除引起的测试编译错误（core tests / tools_sandbox / streaming / approval / history）

2. 行为测试补齐
1. `deny` 永不走审批
2. `ask` 触发审批并可继续
3. `escalate` 触发提权审批语义
4. sandbox degrade 在无 approval_policy 下行为正确

3. E2E 验证
- 保留并扩展 `create/update/publish -> submit turn` 用例
- timeline 断言 `policy_version` / `matched_rule_id` / `approval_kind`

4. 回归基线
- `cargo fmt -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p openjax-core --test policy_center_suite`
- `cargo test -p openjax-core --test tools_sandbox_suite`
- `cargo test -p openjax-core --test approval_events_suite`
- `cargo test -p openjax-gateway --test policy_api_suite`

### 阶段 E：文档与规范统一
1. 删除所有 `approval_policy` 文档引用
- `AGENTS.md`
- `CLAUDE.md`
- `README.md` / `README.zh-CN.md`
- `docs/config.md`
- `openjax-core/src/tools/docs/*`
- `openjaxd/README.md`

2. 增补单中心决策说明
- 权限审批仅由 policy center 决定
- `sandbox_mode` 不承担审批策略

3. 更新工具接入门禁
- 新工具必须声明 policy descriptor
- 必测 `allow/ask(or escalate)/deny`

## 5. 子代理执行与独立审查安排

### 执行分工（Subagent-Driven）
1. 子代理 A（`gpt-5.3-codex`）：阶段 A（类型/配置硬切）
2. 子代理 B（`gpt-5.3-codex`）：阶段 B（决策链路收敛）
3. 子代理 C（`gpt-5.3-codex`）：阶段 D（测试重构与补齐）
4. 子代理 D（轻量模型）：阶段 E（文档清理）

要求：

1. 子代理优先使用 `rustrover-index` 做符号/调用链定位
2. 写入范围互斥，避免冲突
3. 每个阶段落地后立即回归关键测试

### 最终独立审查（必须）
新增一名审查子代理（不参与实现，`gpt-5.3-codex`）做只读复核：

1. 是否仍有 `ApprovalPolicy` / `approval_policy` / `OPENJAX_APPROVAL_POLICY` 残留
2. 是否仍存在非 policy center 的审批决策分支
3. `deny` 是否可能被错误放行
4. 是否存在静默失败或无限审批重试
5. 事件字段与 UI 展示语义是否一致

有阻塞项则不得收尾。

## 6. 风险与注意点
1. `openjax-core/src/tests.rs` 与多套历史测试大量依赖 `ApprovalPolicy::Never`，改造成本高
2. `spawn_sub_agent` 构造链路受签名变化影响，易漏改
3. TUI runtime 字段删除可能影响渲染/快照测试
4. 文档残留概率高，必须全仓关键字扫尾
5. 本次为硬切，不允许新增兼容分支或临时 fallback

## 7. 完成判定（DoD）
代码层：

1. 全仓无 `ApprovalPolicy` 类型定义与调用
2. 全仓无 `approval_policy` 运行时字段
3. 全仓无 `OPENJAX_APPROVAL_POLICY`

行为层：

1. 审批行为仅由 policy center 驱动
2. `allow/ask/escalate/deny` 语义经测试验证

质量层：

1. `fmt / clippy / tests` 全绿
2. 独立审查子代理输出“无阻塞问题”
