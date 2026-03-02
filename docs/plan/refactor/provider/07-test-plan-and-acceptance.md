# 07 测试计划与验收

## 单元测试

### registry

1. 新 schema 解析成功。
2. legacy 桥接成功。
3. new+legacy 并存优先 new。

### router

1. 阶段主模型选择正确。
2. fallback 链顺序正确。
3. stream/reasoning 能力过滤生效。

### adapter

1. 非流式解析输出正确。
2. 流式增量拼装正确。
3. reasoning/usage 提取稳定。

## 集成测试

1. planner 走 `planner` 路由。
2. final writer 走 `final_writer` 路由。
3. 主模型失败后切备用模型并返回成功结果。
4. legacy 配置回归：行为与旧版本一致。

## 验收标准

1. 新 provider 接入无需修改 `agent/planner`。
2. 可同时支持 legacy 与 new schema。
3. fallback 链可从日志完整还原。
4. `cargo build` 与 `cargo test -p openjax-core --lib` 通过。
