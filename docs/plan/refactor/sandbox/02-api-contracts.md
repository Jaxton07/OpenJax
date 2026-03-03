# 02 - API Contracts

状态: done

## 兼容约束
- `crate::tools::policy::*` 继续可用（转发到 `crate::sandbox::policy::*`）。
- `crate::tools::sandbox_runtime::*` 继续可用（转发到 `crate::sandbox::runtime::*`）。
- shell tool 输出保持旧字段，同时新增：
  - `runtime_allowed`
  - `runtime_deny_reason`

## 内部接口
- `sandbox::execute_shell(invocation, command, timeout_ms)`
- `sandbox::classifier::classify_command(command)`
- `sandbox::result::classify_shell_result(exit_code, stdout, stderr)`
