# 00 - Current State

状态: done

## 现状问题
- sandbox 相关逻辑分散于 `tools/policy.rs`、`tools/sandbox_runtime.rs`、`tools/handlers/shell.rs`、`tools/orchestrator.rs`。
- shell handler 同时处理策略、runtime、degrade、审批、结果分类。
- orchestrator 与 shell handler 对 shell 命令存在策略/审批职责重复。
- macOS `ps/top` 失败路径在管道场景可能出现状态判定偏差。

## 重构目标
- 把 sandbox 逻辑收拢到 `openjax-core/src/sandbox`。
- 保持 `crate::tools::*` 对外兼容（通过 re-export）。
