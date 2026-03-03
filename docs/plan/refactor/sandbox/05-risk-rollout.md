# 05 - Risk & Rollout

状态: planned

## 风险
1. shell 审批路径从 orchestrator 收口到 sandbox 后产生行为差异。
2. tool 输出新增字段影响下游解析。
3. 兼容 re-export 漏项导致外部编译失败。

## 控制措施
1. 阶段化交付：先迁移、再收口、后修语义。
2. 保持旧字段不变，新增字段只做增量扩展。
3. 跑核心回归测试并保留回滚入口。

## 发布建议
1. 先在 macOS 开发环境验证 shell 行为。
2. 通过回归后再扩展 Linux/Windows backend 调试。
