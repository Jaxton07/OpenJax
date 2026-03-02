# 01 当前状态审计

## 代码路径

1. `openjax-core/src/model/factory.rs`：模型入口构建。
2. `openjax-core/src/model/chat_completions.rs`：OpenAI 兼容协议实现。
3. `openjax-core/src/model/anthropic_messages.rs`：Anthropic Messages 协议实现。
4. `openjax-core/src/agent/planner.rs`：planner/final writer 模型调用。
5. `openjax-core/src/config.rs`：模型配置解析。

## 现状能力

1. 已支持 OpenAI Chat Completions 与 Anthropic Messages 两类协议。
2. 已支持 GLM Anthropic 格式与 legacy 兼容处理。
3. 已支持 thinking 日志输出。
4. 已具备基础 fallback 思路（早期工厂顺序回退）。

## 痛点分级

### P0

1. 缺少显式模型注册表，模型定义分散在构造器与环境变量。
2. 缺少统一的阶段路由抽象（planner/final_writer/tool_reasoning）。
3. 缺少标准化 fallback 追踪字段，故障定位成本高。

### P1

1. provider/protocol 与路由策略耦合在工厂逻辑，新增供应商需要改多处。
2. 配置结构只有 legacy 单模型语义，不利于多模型混合。
3. 模型响应缺少统一结构化字段（usage/reasoning/raw）沉淀。

### P2

1. 适配器间存在重复逻辑（SSE 解析、请求封装、日志预览）。
2. 缺少 provider 能力矩阵文档与升级准入标准。

## 重构收益

1. 新 provider 接入最小化变更面（新增 adapter + 注册）。
2. 混合模型策略可配置，不需要改 planner 业务逻辑。
3. 出错后能直接看到 fallback 链路与失败点。
