# Provider 重构计划索引

本目录沉淀 OpenJax `openjax-core` 的 LLM Provider 架构重构方案，目标是支持：

1. 多供应商并存（OpenAI/Anthropic/GLM/MiniMax/后续供应商）。
2. 同供应商多模型路由。
3. 分阶段静态路由（planner/final_writer/tool_reasoning）。
4. 可观测的 fallback 与能力约束。
5. 新旧配置兼容桥接（legacy `[model]` -> new `[model.models]`）。

## 文档清单

1. [01-current-state-audit.md](./01-current-state-audit.md)
2. [02-target-architecture.md](./02-target-architecture.md)
3. [03-config-schema-and-compat.md](./03-config-schema-and-compat.md)
4. [04-router-capability-contract.md](./04-router-capability-contract.md)
5. [05-adapter-interface-and-provider-matrix.md](./05-adapter-interface-and-provider-matrix.md)
6. [06-rollout-migration-and-risk.md](./06-rollout-migration-and-risk.md)
7. [07-test-plan-and-acceptance.md](./07-test-plan-and-acceptance.md)
8. [08-observability-and-ops-runbook.md](./08-observability-and-ops-runbook.md)

## 阅读顺序

1. 先看 01/02（理解现状与目标）。
2. 再看 03/04/05（配置、路由、适配器契约）。
3. 最后看 06/07/08（发布、验证、运维）。
