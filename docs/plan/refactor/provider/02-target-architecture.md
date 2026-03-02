# 02 目标架构

## 三层模型架构

1. `Model Registry`：从配置加载命名模型池，产出标准 `RegisteredModel`。
2. `Model Router`：按 `ModelStage` 选主模型并执行 fallback。
3. `Provider Adapter`：协议实现层（Anthropic Messages / Chat Completions）。

## 职责边界

1. `Agent/Planner`：只声明调用阶段，不感知 provider 细节。
2. `ModelRouter`：负责选路、能力过滤、重试与日志。
3. `ProviderAdapter`：负责请求/解析/协议差异。

## 生命周期时序

1. Agent 启动 -> 读取 `Config`。
2. Factory 构建 `ModelRegistry`。
3. Factory 基于 registry 生成 adapter 实例。
4. Factory 注入 `ModelRouter` 到 Agent。
5. planner 阶段调用 `ModelStage::Planner`。
6. final writer 阶段调用 `ModelStage::FinalWriter`。
7. router 失败时按 fallback 链切换模型。

## 关键设计约束

1. Router 默认最多 `2` 级 fallback（共 3 次尝试）。
2. stream 调用只选择 `supports_stream=true` 模型。
3. require_reasoning 调用只选择 `supports_reasoning=true` 模型。
4. 配置冲突时，新配置优先，legacy 仅桥接。
