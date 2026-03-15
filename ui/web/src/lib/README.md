# `src/lib` 索引

该目录存放前端“无 UI 逻辑”层：网关通信、事件归并、流式渲染运行时、本地存储与错误映射。

## 目录职责

- Gateway 客户端：请求/响应与 API 封装
- Stream 处理：SSE 事件归并、渲染缓存、顺序控制
- 本地持久化：设置与会话存储
- 错误标准化：前端可读错误信息

## 关键文件

- `gatewayClient.ts`：HTTP API 与事件流接口封装。
- `eventReducer.ts`：将流式事件折叠成会话/消息状态。
- `streamRuntime.ts`：流式事件处理辅助与顺序门控。
- `streamRenderStore.ts`：按 `session + turn` 聚合文本渲染缓存。
- `storage.ts`：`localStorage` 读写与数据版本兼容。
- `errors.ts`：错误类型与错误消息处理。

## 测试文件

- `gatewayClient.test.ts`
- `eventReducer.test.ts`
- `streamRuntime.test.ts`
- `streamRenderStore.test.ts`
- `storage.test.ts`
- `errors.test.ts`

## 维护建议

- 业务状态归并优先放在 `eventReducer.ts`，避免在组件中做事件拼装。
- Gateway 协议改动先改 `types/gateway.ts`，再同步 `gatewayClient.ts` 与测试。

## 上层文档

- 返回 Web 模块总文档：[ui/web/README.md](../../README.md)
