# OpenJax 重构阶段计划与 TODO 清单

目标：将当前项目从“Rust CLI 主导”演进为“Rust 内核 + Python 外层能力”分层架构。  
范围：优先完成协议冻结、Daemon 落地、Python SDK MVP、Python TUI 对齐；Bot 集成作为后续阶段。

---

## 0. 当前阶段定义

- 当前阶段：`阶段 D（Python SDK MVP）准备开始`
- 进入重构编码前提：前置准备清单达到 Go 条件
- 本文档用途：先按计划执行，再逐项勾选，不跳步推进

---

## 1. 阶段总览（按顺序）

1. 阶段 A：前置准备与文档修正
2. 阶段 B：跨语言协议 v1 冻结
3. 阶段 C：`openjaxd` MVP（Rust Daemon）
4. 阶段 D：Python SDK MVP
5. 阶段 E：Python TUI MVP（与 Rust TUI 对齐）
6. 阶段 F：外层扩展（Telegram）与发布治理

---

## 2. 阶段 A：前置准备与文档修正（现在先做这个）

### A.1 必做 TODO（Go 条件）

- [x] 对齐架构口径：更新 `docs/project-structure-index.md`（2026-02-20）
  - 增加“当前态 vs 目标态”双视图
  - 纳入 `openjax-tui` 当前状态
  - 预留 `openjaxd`/`python/*` 结构位
- [x] 修正文档边界：更新 `docs/tools/overview.md`（2026-02-20）
  - 将“与 Codex 一致体验”改为“能力对齐范围”
  - 明确暂不承诺项（例如并发多会话、完整 UI 能力等）
- [x] 衔接文档：更新 `docs/tui/technical-direction.md`（2026-02-20）
  - 补充“Rust TUI -> Python TUI 能力对齐清单”
  - 明确迁移期间回退策略（Rust TUI 保留到 Python 稳定）
- [x] 新增协议文档入口与目录结构（占位即可）（2026-02-20）
  - `docs/protocol/v1/README.md`
  - `docs/protocol/v1/schema/`
- [ ] 定义重构分支与提交流程
  - 推荐分支：`codex/refactor-rust-kernel-python-shell`
  - 约定提交粒度：一个阶段一个 PR（或一个里程碑一个 PR）

### A.2 建议补充 TODO（非阻塞）

- [ ] 增加 `docs/plan/refactor/risk-register.md`（风险台账）
- [ ] 增加 `docs/plan/refactor/decision-log.md`（关键决策记录）
- [ ] 增加跨语言术语表（session/turn/event/request_id 等）

### A.3 阶段 A 验收标准（Go/No-Go）

1. 三份核心文档完成修正：`project-structure-index.md`、`technical-direction.md`、`tools/overview.md`。
2. 协议文档与 schema 目录已建，后续可直接填充。
3. 团队对“先协议冻结再编码”的顺序达成一致。

---

## 3. 阶段 B：跨语言协议 v1 冻结

### B.1 TODO

- [x] 定义协议操作集：`start_session`、`submit_turn`、`stream_events`、`resolve_approval`、`shutdown_session`（2026-02-20）
- [x] 固定传输分帧方案（JSONL 或 length-prefix 二选一）（2026-02-20，选型：JSONL）
- [x] 定义统一错误模型：`code/message/retriable/details`（2026-02-20）
- [x] 定义 ID 体系：`request_id/session_id/turn_id`（2026-02-20）
- [x] 定义审批闭环：超时、取消、默认策略（2026-02-20）
- [x] 补齐示例 payload 与时序图（2026-02-20）
- [x] 输出 JSON Schema 并完成样例校验（2026-02-20）

### B.2 验收标准

1. Rust 与 Python 团队可仅凭文档独立实现并互通。
2. 样例消息可通过 schema 校验。
3. 协议变更策略明确（向前兼容规则 + 版本规则）。

---

## 4. 阶段 C：openjaxd MVP（Rust）

### C.1 TODO

- [x] 新建 crate：`openjaxd`（2026-02-20）
- [x] 接入 `openjax-core`，实现单会话最小闭环（2026-02-20）
- [x] 打通 `submit_turn -> stream_events`（2026-02-20）
- [x] 打通审批请求与回传（2026-02-20）
- [x] 增加超时回收与异常退出清理（2026-02-20，审批超时 + EOF 会话清理）
- [x] 增加结构化日志（带 request/turn/session 关联字段）（2026-02-20）

### C.2 验收标准

1. 单会话可稳定跑通完整 turn。
2. 异常退出后无挂起状态、无僵尸进程。
3. 协议集成测试通过。

---

## 5. 阶段 D：Python SDK MVP

### D.1 TODO

- [ ] 新建 `python/openjax_sdk`
- [ ] 封装 daemon 生命周期管理（启动/健康检查/退出）
- [ ] 提供 async 客户端（sync 可后补）
- [ ] 实现事件分发与 `AssistantDelta` 聚合
- [ ] 实现审批接口映射
- [ ] 增加协议类型约束（Pydantic 或 dataclass）
- [ ] 增加单元测试与端到端集成测试

### D.2 验收标准

1. SDK 作为无 UI 客户端可跑通完整 turn。
2. 审批与增量输出行为和现有 Rust TUI 一致。

---

## 6. 阶段 E：Python TUI MVP

### E.1 TODO

- [ ] 新建 `python/openjax_tui`
- [ ] 实现输入、流式输出、审批弹层
- [ ] 对齐关键键位和错误提示
- [ ] 做终端恢复兜底（异常退出场景）
- [ ] 完成 tmux/zellij 基础回归
- [ ] 保留 Rust TUI 作为可切换回退路径

### E.2 验收标准

1. 核心体验不弱于 `openjax-tui`。
2. 稳定性达到可日常使用水平。

---

## 7. 阶段 F：Telegram + 发布治理

### F.1 TODO

- [ ] 新建 `python/openjax_telegram`
- [ ] 用户维度 session 隔离
- [ ] 默认保守审批策略（可配置）
- [ ] 速率限制、重试、幂等处理
- [ ] CI 分层：Rust / Python / 跨语言协议集成
- [ ] 版本矩阵与发布策略文档化

### F.2 验收标准

1. Bot 能稳定执行多轮对话与工具调用。
2. 协议变更可被 CI 提前拦截不兼容问题。

---

## 8. 开工门槛（必须全部满足）

- [ ] 阶段 A 的 A.1 必做 TODO 全部完成
- [ ] 阶段 B 协议文档冻结并评审通过
- [ ] `openjaxd` MVP 范围已锁定（不夹带新需求）

---

## 9. 本周建议执行顺序（可直接照做）

1. 完成阶段 A 三份文档修正（半天到 1 天）。
2. 建立协议文档和 schema 占位并写 v1 草案（1 天）。
3. 创建 `openjaxd` crate skeleton + hello-turn 联调（1 到 2 天）。

---

## 10. 变更记录

- 2026-02-20：创建初版重构阶段计划与 TODO 清单。
- 2026-02-20：完成阶段 A 核心文档修正，进入阶段 B 协议草案与 schema 编写。
- 2026-02-20：完成阶段 B 协议草案 + schema；`openjaxd` MVP 骨架已落地并跑通 submit_turn 事件流。
- 2026-02-20：完成阶段 C 收尾（结构化日志、异常清理、`openjaxd` 协议集成测试通过）。
