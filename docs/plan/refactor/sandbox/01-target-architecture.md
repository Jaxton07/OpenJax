# 01 - Target Architecture

状态: done

## 模块结构
- `openjax-core/src/sandbox/mod.rs`: façade 入口 (`execute_shell`)
- `openjax-core/src/sandbox/policy.rs`: 策略与能力识别
- `openjax-core/src/sandbox/runtime/mod.rs`: runtime 调度
- `openjax-core/src/sandbox/runtime/macos.rs`: seatbelt 实现
- `openjax-core/src/sandbox/runtime/linux.rs`: linux runtime
- `openjax-core/src/sandbox/runtime/none.rs`: 非沙箱执行
- `openjax-core/src/sandbox/degrade.rs`: degrade 审批流程
- `openjax-core/src/sandbox/result.rs`: 成败分类
- `openjax-core/src/sandbox/classifier.rs`: 命令分类
- `openjax-core/src/sandbox/audit.rs`: 审计日志

## Tool 层边界
- `tools/handlers/shell.rs` 仅保留参数解析 + `sandbox::execute_shell` 调用。
- `tools/policy.rs`、`tools/sandbox_runtime.rs` 作为兼容转发层。
