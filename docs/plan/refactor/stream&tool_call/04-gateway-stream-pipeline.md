# 04 Gateway Stream Pipeline

状态：`in_progress`

## 目标

1. Gateway 仅承担：
- 会话路由
- 事件序列号包装
- SSE 推送与恢复

2. 不承担：
- 复杂事件业务拼装
- 大量 core 语义转换

## 重构任务

1. 将事件映射拆分为独立 mapper 模块（按响应/工具/审批分组）。
2. 将 session replay 逻辑改为复用 core replay 抽象。
3. 将广播容量与回放窗口改为配置化。
4. 标准化 lagged 恢复与越窗错误输出。

## 验收

1. `handlers.rs` 明显瘦身。
2. `state.rs` 中映射逻辑降到可维护长度。
3. SSE 恢复测试覆盖 query/header 两种恢复路径。

## 当前进展

1. 回放窗口已切到 `streaming::ReplayBuffer`。
2. replay/broadcast 容量已引入环境变量配置。
3. 下一步是将 `map_core_event` 继续拆分为 response/tool/approval mapper 子模块。
