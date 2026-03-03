# 03 - Migration Plan

状态: in_progress

## 阶段划分
1. Phase 0 文档基线
2. Phase 1 机械搬迁（兼容 re-export）
3. Phase 2 流程收口（shell façade）
4. Phase 3 语义修正（runtime_allowed）
5. Phase 4 平台扩展（windows 占位）

## 当前进度
- 已完成：Phase 0、Phase 1、Phase 2 主体、Phase 3 主体
- 待完成：Phase 4 windows 占位与 feature gate

## 回滚点
- 保留 `tools/policy.rs` 与 `tools/sandbox_runtime.rs` 兼容入口。
- 若 façade 异常，可回滚 `tools/handlers/shell.rs` 到直接执行路径。
