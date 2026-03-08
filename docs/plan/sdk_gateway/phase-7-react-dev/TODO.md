# Phase 7 执行清单

## 任务

| 任务 | 状态 | 验收标准 |
|---|---|---|
| 初始化 React 项目与基础布局 | done | 页面可启动并显示基础框架 |
| 实现会话与消息流页面 | done | 可展示历史、增量与最终消息 |
| 实现审批与 clear/compact 交互 | done | 交互可触发对应网关接口 |
| 实现错误提示与重连机制 | done | 断连/鉴权/限流可恢复或可提示 |
| 联调双输出模式 | done | SSE 与 Polling 至少一主一备可用 |

## 阻塞项

- 无

## 结果摘要（阶段完成后保留）

- 已新增 `ui/web/`（Vite + React + TypeScript）并落地 ChatGPT 风格极简 UI 骨架。
- 已实现会话侧栏（新聊天/历史）+ 主聊天区 + 设置弹窗（API Key/Base URL/输出模式/连接测试）。
- 已实现 Gateway 接入：`start/submit/get_turn/stream_events/resolve/clear/compact/shutdown`。
- 已实现 SSE 主通道与 Polling 备用路径、`event_seq` 去重、增量拼接与审批闭环。
- 已补充前端单元测试：事件 reducer、错误映射、设置持久化。
