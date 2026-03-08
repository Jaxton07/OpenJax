# 01 术语与范围边界

## 核心术语

- Rust 内核：`openjax-core`，负责 agent loop、tools、sandbox、approval。
- Rust Gateway：面向 Web/移动端的服务层，提供 API、SSE、鉴权、会话编排。
- React 前端：对话与管理界面，消费 Gateway 协议。
- 协议契约：对外 API 与事件格式、错误码、版本策略。

## In Scope（本路线包含）

- 统一文档体系与阶段化推进。
- Gateway 协议与服务设计。
- Web 前端接入与交互模型。
- 面向后续移动端接入的标准化接口准备。

## Out of Scope（本路线暂不包含）

- 重写 Rust 内核为 Java。
- 本轮内完成移动端客户端实现。
- 非 Gateway 路线的历史方案继续扩展。

## 边界约束

- 根目录只保留导航、节奏、决策和阶段看板。
- 执行细节只存在于当前阶段目录。
- 非当前阶段内容默认不加载。
