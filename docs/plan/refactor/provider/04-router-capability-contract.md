# 04 Router 与能力契约

## 路由阶段

1. `planner`
2. `final_writer`
3. `tool_reasoning`

## 尝试链规则

1. 取该阶段主模型作为 attempt-1。
2. 读取 `routing.fallbacks[model_id]` 追加候选。
3. 去重并按声明顺序执行。
4. 最多执行 `1 + max_fallback_chain` 次。

## 能力约束

1. stream 请求：仅 `supports_stream=true`。
2. reasoning 请求：若 `require_reasoning=true`，仅 `supports_reasoning=true`。

## 错误传播

1. 每次失败记录日志并继续 fallback。
2. 所有候选失败时返回最后一个错误。
3. 若无可用 adapter，返回 `no available model adapter`。

## 行为保证

1. planner 业务代码不感知 provider。
2. fallback 过程可观测且可复现。
