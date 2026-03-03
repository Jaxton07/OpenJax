# 04 - Test Strategy

状态: in_progress

## 单元测试
- `sandbox/result.rs`
  - `exit_code=0 + Operation not permitted` -> `Failure`
  - `exit_code=141 + stdout` -> `PartialSuccess`
- `sandbox/classifier.rs`
  - `ps/top/pgrep` 命中 `ProcessObserve`

## 集成测试关注
- 安全读命令 `cat/ls/grep/vm_stat` 继续成功
- macOS `ps/top` 拒绝时不再假成功
- degrade 审批链保持完整

## 回归测试
- `m3_sandbox`
- `m5_approval_handler`
- `m8_approval_event_emission`
- `m10_approval_panel_navigation`
- `m11_shell_target_visibility`
- `m12_tool_partial_status`
