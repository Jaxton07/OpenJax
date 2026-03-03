# OpenJax 当前沙箱机制总览（Linux + macOS）

本文档总结当前仓库已实现的沙箱机制与审批机制，覆盖架构、执行链路、配置项、测试覆盖、已知限制与后续建议。

## 1. 目标与范围

当前版本的沙箱机制目标：
- 不再依赖“纯字符串黑名单”作为唯一安全手段。
- 在工具执行前进行统一策略决策（允许/审批/拒绝/提权审批）。
- 在 Linux/macOS 上优先使用原生后端隔离执行。
- 当原生后端不可用时，支持可配置降级策略（拒绝或二次审批后非沙箱执行）。
- 在 TUI/CLI/daemon 侧提供可解释的审批信息与审计信息。

当前范围：
- 已实现：Linux、macOS、`shell/exec_command` 重点路径、审批可见性增强、协议扩展、测试回归。
- 未实现：Windows 原生后端、Docker 后端、完整 seccomp/landlock 精细策略装配（目前 Linux 为 bwrap 主链 + 能力探测/降级策略）。

## 2. 核心模块与职责

### 2.1 策略引擎（Policy Layer）
- 文件：`openjax-core/src/tools/policy.rs`
- 核心类型：
  - `SandboxBackend`
  - `SandboxCapability`
  - `PolicyDecision`：`Allow | AskApproval | AskEscalation | Deny`
  - `PolicyTrace`
  - `ApprovalContext`
- 入口函数：
  - `evaluate_tool_invocation_policy(invocation, is_mutating) -> PolicyOutcome`

职责：
- 识别命令能力（读写/网络/进程/环境变量）。
- 标记风险标签（如 `network`、`write`、`privilege_escalation`、`destructive`）。
- 产出结构化决策与可解释原因（用于审批 UI 与审计日志）。

### 2.2 审批编排层（Approval Orchestration）
- 文件：`openjax-core/src/tools/orchestrator.rs`
- 职责：
  - 对工具调用先走统一策略评估。
  - 对需要审批的调用发出 `ApprovalRequested` 事件。
  - 将审批结果回传 `ApprovalResolved`。
  - 对 shell 类审批请求显示真实命令目标，而非仅工具名。

### 2.3 沙箱运行时层（Sandbox Runtime）
- 文件：`openjax-core/src/tools/sandbox_runtime.rs`
- 核心能力：
  - 选择后端（`auto/linux_native/macos_seatbelt/none`）。
  - Linux 后端：`bwrap` 执行链（含网络 namespace 控制）。
  - macOS 后端：`sandbox-exec` + 动态 profile。
  - 原生后端不可用时，按策略决定拒绝或降级执行。
  - 生成审计日志（包含 command hash、backend、capabilities、decision/degrade）。

### 2.4 Shell Handler（执行入口）
- 文件：`openjax-core/src/tools/handlers/shell.rs`
- 职责：
  - 解析参数（含 `require_escalated` 的 bool/string 兼容解析）。
  - 调用策略引擎得到决策。
  - 执行 workspace 路径约束检查。
  - 调用沙箱运行时执行。
  - 若后端不可用且策略允许降级，触发“二次审批后非沙箱执行”。

## 3. 当前执行链路（单次 shell 调用）

1. `ToolInvocation` 进入 `ToolOrchestrator::run`。  
2. 调用 `evaluate_tool_invocation_policy` 生成 `PolicyOutcome`。  
3. 若 `Deny`：直接拒绝并返回可解释原因。  
4. 若 `AskApproval/AskEscalation`：发出 `ApprovalRequested`，等待 `ApprovalResolved`。  
5. shell handler 进入运行时执行：优先原生后端。  
6. 原生后端可用：返回执行结果（exit/stdout/stderr/backend/trace）。  
7. 原生后端不可用：  
   - `OPENJAX_SANDBOX_DEGRADE_POLICY=deny`：拒绝。  
   - `ask_then_allow`：触发降级审批，通过后非沙箱执行。  
8. 全流程输出审计日志，携带策略与后端信息。

## 4. 策略语义（当前版本）

### 4.1 默认判断
- 明显破坏性命令模式：`Deny`。
- `require_escalated=true`：`AskEscalation`。
- 网络/写入/环境写能力：`AskApproval`。
- 只读与普通进程执行：`Allow`。

### 4.2 与审批策略（ApprovalPolicy）折叠
- `always_ask`：非拒绝调用统一要求审批。
- `on_request`：沿用策略引擎结果。
- `never`：
  - shell 若需要审批则改为拒绝（防止静默放行高风险 shell）。
  - 非 shell mutating 工具保持原兼容行为（避免破坏既有语义）。

## 5. 协议与 UI 可见性

## 5.1 协议字段扩展
- 文件：`openjax-protocol/src/lib.rs`
- `Event::ApprovalRequested` 新增可选字段：
  - `tool_name`
  - `command_preview`
  - `risk_tags`
  - `sandbox_backend`
  - `degrade_reason`

## 5.2 TUI 展示
- 文件：
  - `ui/tui/src/state/app_state.rs`
  - `ui/tui/src/app/reducer.rs`
  - `ui/tui/src/app/render_model.rs`
- 审批面板现可显示：
  - 操作目标
  - 原因
  - 命令预览
  - 风险标签
  - 沙箱后端与降级原因

## 5.3 CLI / daemon 透传
- 文件：
  - `openjax-cli/src/main.rs`
  - `openjaxd/src/main.rs`
- 能输出扩展审批字段，便于排查“看不到命令”或“为何审批”问题。

## 6. 配置项（运行时）

通过环境变量控制：

- `OPENJAX_SANDBOX_BACKEND`
  - `auto`（默认）
  - `linux_native`
  - `macos_seatbelt`
  - `none`

- `OPENJAX_SANDBOX_DEGRADE_POLICY`
  - `ask_then_allow`（默认）
  - `deny`

- `OPENJAX_SANDBOX_AUDIT`
  - `1`（默认）
  - `0`

已有配置继续生效：
- `OPENJAX_SANDBOX_MODE`
- `OPENJAX_APPROVAL_POLICY`

## 7. 安全约束（当前实现）

- 仍保留 workspace 路径约束（禁止绝对路径越权、父目录穿越等）。
- 不再把 `|`、`>` 等操作符作为绝对拒绝条件；改由能力+风险判断。
- 后端能力缺失时不会“静默放行”，会按降级策略处理并记录审计。

## 8. 审计与可观测性

运行时审计日志记录：
- `command_hash`
- `backend`
- `capabilities`
- `decision`
- `degrade_reason`（若有）

工具执行日志新增可读预览：
- `stdout_preview`
- `stderr_preview`
- `output_preview`（回传给模型的文本预览）

这解决了“看不到模型实际拿到的 tool/shell 返回内容”的排障问题。

TUI 历史区工具起始行会展示命令目标：
- `Run shell (<command>)`

这解决了“只看到 Run shell，看不到具体执行命令”的可见性问题。

命令执行准确性增强：
- 在 `bash/zsh` 执行前自动注入 `set -o pipefail`。
- 管道中任一子命令失败会导致整体失败，避免 `a | b` 中 `a` 失败但整体被判成功。

macOS 后端健康探测：
- 启动后首次使用 `macos_seatbelt` 时会执行一次健康探测并缓存结果。
- 若探测判定不可用，后续命令直接走降级路径并标注 `macos_seatbelt_unavailable_cached`。
- 错误原因会按类型分类：
  - `macos_seatbelt_apply_denied`
  - `macos_seatbelt_profile_invalid`
  - `macos_seatbelt_permission_denied`
  - `macos_seatbelt_runtime_error`
  - `macos_seatbelt_unknown_nonzero`

macOS runner 策略：
- seatbelt 后端默认使用 `/bin/sh -c <command>` 执行（不依赖 zsh 启动链）。
- 这样可以降低在 `deny default` profile 下 shell 初始化路径导致的失败概率。

Shell fallback 策略：
- Unix 平台 shell 解析采用回退链：
  - `zsh -> bash -> sh`
  - `bash -> zsh -> sh`
- 避免某台机器缺少特定 shell 时直接失败。

这使得排障时可定位：
- 为什么触发审批
- 为什么被拒绝
- 是否发生了降级执行
- 最终用的是什么后端

## 9. 测试覆盖（已验证）

已通过的关键测试：
- `openjax-core/tests/m3_sandbox.rs`
  - pipeline 命令可执行
  - 网络/写入在 `approval_policy=never` 下被拒绝
  - 路径越权仍被拦截
- `openjax-core/tests/m5_approval_handler.rs`
  - 非 shell mutating 与旧语义兼容
- `openjax-core/tests/m8_approval_event_emission.rs`
  - 审批事件生命周期正确
- `ui/tui/tests/m10_approval_panel_navigation.rs`
  - 扩展审批数据下 UI 行为稳定

## 10. 已知限制与后续建议

当前限制：
- Windows 原生后端未接入。
- Linux 仍以 bwrap 为主，landlock/seccomp 的“完整功能矩阵”尚未全面落地。
- 规则仍含启发式解析，未来应引入更严格命令 AST/能力映射。

建议后续工作：
1. 补全 Linux 下 seccomp/landlock 精细策略生成与验证。  
2. 增加 macOS profile 模板分级（只读/读写/网络）回归集。  
3. 加入 Windows 后端并补齐跨平台一致性测试。  
4. 把 policy/runtime 抽成独立 crate，进一步降低 core 耦合。  

## 11. 相关设计与规划文档

本次重构设计文档位于：
- `docs/plan/refactor/sandbox/`
  - `00-current-state.md`
  - `01-architecture.md`
  - `02-risk-register.md`
  - `03-api-contracts.md`
  - `04-policy-engine.md`
  - `05-approval-ux.md`
  - `06-linux-runtime.md`
  - `07-macos-runtime.md`
  - `08-module-boundary.md`
  - `09-test-plan.md`
  - `10-migration-rollout.md`
