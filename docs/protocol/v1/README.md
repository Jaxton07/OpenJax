# OpenJax Cross-Language Protocol v1

本目录用于维护 Rust 内核与 Python 外层之间的 v1 协议定义与 schema。

## 文档

- `protocol-v1.md`：v1 协议主文档（操作集、事件、错误、时序、示例）
- `schema/request.schema.json`：请求信封 schema
- `schema/response.schema.json`：响应信封 schema
- `schema/event.schema.json`：事件信封 schema
- `examples/*.json`：schema 校验样例

## 校验

```bash
jsonschema docs/protocol/v1/schema/request.schema.json \
  -i docs/protocol/v1/examples/request-submit-turn.json

jsonschema docs/protocol/v1/schema/response.schema.json \
  -i docs/protocol/v1/examples/response-submit-turn-ok.json \
  -i docs/protocol/v1/examples/response-error-invalid-params.json

jsonschema docs/protocol/v1/schema/event.schema.json \
  -i docs/protocol/v1/examples/event-assistant-delta.json \
  -i docs/protocol/v1/examples/event-approval-requested.json
```

## 版本与兼容策略

1. 新增字段必须保持向前兼容（旧客户端可忽略未知字段）。
2. 删除字段或破坏性语义变更必须升级 major。
3. `protocol_version` 必须在握手阶段明确声明（当前为 `v1`）。
