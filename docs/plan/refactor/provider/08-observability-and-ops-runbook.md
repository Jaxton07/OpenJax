# 08 可观测性与运维 Runbook

## 必备日志字段

1. `stage`
2. `model_id`
3. `backend`
4. `provider`
5. `protocol`
6. `attempt_index`
7. `fallback_from`
8. `latency_ms`

## 推荐指标

1. `provider_request_total{stage,model_id,status}`
2. `provider_request_latency_ms{stage,model_id}`
3. `provider_fallback_total{stage,from,to}`
4. `provider_bridge_legacy_total`

## 告警建议

1. 某 stage fallback 率 > 阈值（例如 20%）。
2. 某 model_id 错误率持续升高。
3. legacy 桥接命中率长期高于阈值（提示迁移配置）。

## 排障 SOP

1. 先查 `model_router attempt failed/succeeded` 日志，确认尝试链。
2. 再核对 `model_id/provider/protocol` 是否与期望路由一致。
3. 若 stream 失败，先看能力过滤日志（是否被 `supports_stream=false` 跳过）。
4. 若请求被桥接，检查是否仍在使用 legacy `[model]`。
