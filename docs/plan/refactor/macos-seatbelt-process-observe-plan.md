# macOS Seatbelt 进程观测命令放通实施计划

## 1. 背景与问题定义

当前 OpenJax 在 macOS 上已能通过 `sandbox-exec` 启动沙箱执行链，但 `ps/top` 等进程观测命令在 seatbelt 内被拒绝，典型错误为：

- `/bin/sh: /bin/ps: Operation not permitted`
- `/bin/sh: /usr/bin/top: Operation not permitted`

这说明：
- 沙箱机制本身是生效的（不是“没进沙箱”）。
- 现有 seatbelt profile 过于保守，未满足“进程观测类命令”所需权限。
- `policy_decision=Allow` 只代表 OpenJax 策略层放行，不代表 seatbelt 运行时一定放行。
- 现网已出现 `exit_code=0` 但 stderr 为 `Operation not permitted` 的案例，说明不能只看退出码判断成功。

本计划目标是：在不放宽到危险级别的前提下，让 `ps/top` 类只读观测命令可在 macOS seatbelt 中成功执行。

### 1.1 根因假设（先验证，再放通）

必须先在目标 macOS 版本上完成根因分层验证，避免盲目扩大权限：

1. Profile 权限缺失：可通过最小规则增量放通。  
2. `sandbox-exec` + 进程观测存在系统硬限制：无法仅通过 profile 放通。  
3. 结果分类误判：命令失败被标记为 success（典型是管道场景）。  

在 Phase 0 完成前，不承诺 `ps/top` 一定可在 seatbelt 内放通。

---

## 2. 目标与验收标准

## 2.1 功能目标
- 在 `backend=macos_seatbelt` 下，以下命令可以成功执行并返回有效 stdout：
  - `ps aux -r | head -n 10`
  - `ps -eo pid,pcpu,pmem,comm -r | head -n 10`
  - `top -l 1 -n 10 -o cpu | head -n 20`

## 2.2 安全目标
- 仅放通“进程观测”所需能力，不放开通用写权限和网络权限。
- 保持路径越权防护与审批机制不回退。

## 2.3 观测目标
- 日志可明确识别：
  - 使用的 seatbelt profile 名称
  - seatbelt runner
  - 后端是否降级
  - 拒绝原因分类（若失败）

## 2.4 验收标准（必须全部满足）
1. seatbelt 后端执行 `ps/top` 不再普遍报 `Operation not permitted`。  
2. TUI 中工具状态与真实结果一致（成功/partial/失败）。  
3. 不影响已有 `m3/m5/m8/m10/m11/m12` 回归测试。  
4. 新增 macOS profile 选择与命令分类测试全部通过。  
5. `exit_code=0` 且 stderr 命中 fatal 模式时，不得标记为 `result_class=success`。  

---

## 3. 设计方案

## 3.1 引入 profile 模板分级（macOS 专用）

在 `openjax-core/src/tools/sandbox_runtime.rs` 中将现有单一 profile 拆分为多模板：

- `base_read_profile`：基础只读（当前已有风格）
- `process_observe_profile`：用于 `ps/top` 类命令，放通进程观测所需系统能力
- `network_enabled_overlay`：按 capability 叠加网络策略

实现方式：
- 通过命令分类决定 profile 类型，再渲染 profile 字符串。
- 在日志输出 `seatbelt_profile_kind=base_read|process_observe`。

## 3.2 新增命令分类：`ProcessObserve`

在策略/运行时之间增加命令特征判定（仅 macOS runner 使用）：

- 匹配命令前缀或 token：
  - `ps `
  - `top `
  - `pgrep `

命中后选择 `process_observe_profile`。

注意：
- 仅用于 profile 选择，不改变原审批策略决策。
- 仍受 `ApprovalPolicy` 与 `SandboxPolicy` 约束。

## 3.3 `process_observe_profile` 权限最小化原则

第一版按“最小增量”放通：

1. 保留 `(deny default)`。
2. 允许 `process*`（已有）。
3. 在只读范围内，补充 `ps/top` 所需系统信息访问能力（按 seatbelt 规则逐步放开）。
4. 保持 workspace 写权限不扩大。
5. 不默认放网络。

策略是渐进放通：
- 先放通到 `ps/top` 可跑。
- 通过测试与日志验证后，不再继续扩大权限。

若验证结果是“系统硬限制不可放通”，立即切换到 3.5 兜底方案，不继续扩大 profile。

## 3.4 保留降级机制但调整触发语义

当前逻辑：
- seatbelt 失败 => 按 degrade policy 降级。

调整：
- 如果是 `process_observe_profile` 且失败，先记录 `profile_kind` 和拒绝码。
- 降级前日志必须带：
  - `seatbelt_profile_kind`
  - `seatbelt_runner`
  - `stderr_preview`
  - `classified_reason`

并新增语义区分：
- `policy_decision=Allow`（策略层）
- `runtime_allowed=true|false`（seatbelt 运行时）

## 3.5 不可放通时的兜底方案（必须有）

若 Phase 0/Phase B 证明 `ps/top` 在目标 macOS 版本不可放通，采用受控降级：

1. 仅对 `ProcessObserve` 命令启用该兜底。  
2. 当 `backend=macos_seatbelt` 且 stderr 命中 `Operation not permitted` 时，触发 degrade。  
3. degrade 必须走审批链，并在文案中明确“仅进程观测命令临时降级”。  
4. 审计必须完整记录：`backend=none_escalated`、`degrade_reason`、`risk_tags`。  

---

## 4. 实施步骤（可执行）

## Phase 0：根因验证基线（0.5-1 天）
1. 在目标 macOS 版本执行最小复现矩阵：`true`、`echo`、`ps`、`top`。  
2. 记录每条命令的 `exit_code/stdout/stderr` 与 profile 摘要。  
3. 输出结论：可放通 / 不可放通（硬限制）。  

产出：
- 可复现根因记录，作为 Phase A/B 输入。  

## Phase A：Profile 框架改造（1 天）
1. 拆分 `render_macos_profile(...)` 为：
   - `render_macos_base_profile(...)`
   - `render_macos_process_observe_profile(...)`
2. 在执行前增加 `select_macos_profile_kind(command, capabilities)`。
3. 日志加入 `seatbelt_profile_kind`。

产出：
- 代码可编译，行为与现状一致（先不改权限内容）。

## Phase B：ProcessObserve 放通（1-2 天）
1. 在 `process_observe_profile` 中逐条增加 `ps/top` 所需的只读权限。
2. 本地复测三个目标命令是否能在 `backend=macos_seatbelt` 下成功。
3. 若仍拒绝，迭代最小必要权限（每次改动必须带日志证据）。
4. 若两轮最小增量后仍拒绝，按 3.5 进入兜底，不再继续扩大 profile。

产出：
- `ps/top` 跑通，且不需要降级到 `none_escalated`。

## Phase C：测试补齐（1 天）
新增测试：
1. 单元测试：
   - 命令分类正确映射 `process_observe_profile`
   - profile 选择函数对 `ps/top` 与普通命令行为正确
   - `exit_code=0 + fatal stderr` 被判定为 failure/partial，不再误判 success
2. 集成测试（macOS 条件）：
   - `ps aux -r | head -n 10` 在 seatbelt 下成功
   - 非 process_observe 命令仍走 base profile
   - 若触发兜底：审批事件、degrade 字段、backend 标注正确
3. 回归测试：
   - 现有 m3/m5/m8/m10/m11/m12 全通过

## Phase D：文档与运维说明（0.5 天）
更新文档：
1. `docs/sandbox-mechanism-overview.md`
2. `openjax-core/src/tools/docs/troubleshooting.md`

补充内容：
- `process_observe_profile` 设计原则
- 常见失败原因与排查命令
- 如何确认是否已跑在 seatbelt 且未降级

---

## 5. 代码改动清单（预期）

核心文件：
- `openjax-core/src/tools/sandbox_runtime.rs`（主改）
- `openjax-core/src/tools/policy.rs`（可选：命令特征辅助）
- `openjax-core/src/tools/handlers/shell.rs`（日志字段联动）

测试文件：
- `openjax-core/tests/m3_sandbox.rs`（补充 macOS 场景）
- 新增 `openjax-core/tests/m9_macos_seatbelt_process_observe.rs`（建议）

文档文件：
- `docs/sandbox-mechanism-overview.md`
- `openjax-core/src/tools/docs/troubleshooting.md`

---

## 6. 风险与回滚

## 6.1 风险
1. 过度放通导致 seatbelt 安全边界变宽。
2. macOS 版本差异导致 profile 在不同机器表现不一致。
3. `top` 输出格式变化影响模型解析稳定性。

## 6.2 风险控制
1. 使用 profile 分级，只对 process_observe 放通。
2. 所有新增权限逐条记录“放通理由”。
3. 保持降级策略与审计链路，不满足条件时可回退。

## 6.3 回滚方案
1. 环境层快速回滚：
   - `OPENJAX_SANDBOX_BACKEND=none`
2. 代码层回滚：
   - 切回 `base_read_profile` 单模板
   - 禁用 `process_observe_profile` 选择分支

---

## 7. 验证命令（执行清单）

```bash
zsh -lc "cd /Users/ericw/work/code/ai/openJax && cargo build"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && cargo test -p openjax-core --test m3_sandbox"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && cargo test -p openjax-core --test m5_approval_handler"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && cargo test -p openjax-core --test m8_approval_event_emission"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && cargo test -p tui_next --test m10_approval_panel_navigation"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && cargo test -p tui_next --test m11_shell_target_visibility"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && cargo test -p tui_next --test m12_tool_partial_status"
```

手工验证（macOS）：
1. 在 TUI 中执行 `ps aux -r | head -n 10`。  
2. 确认日志显示 `backend=macos_seatbelt` 且非 `backend unavailable`。  
3. 确认 `degrade_reason=none`。  

---

## 8. 完成定义（Definition of Done）

满足以下条件才算完成：
1. `ps/top` 在 macOS seatbelt 内可执行并有有效输出。  
2. 不再默认降级到 `none_escalated` 才能得到结果。  
3. 所有回归测试通过。  
4. 文档更新完成且可指导后续排障。  
