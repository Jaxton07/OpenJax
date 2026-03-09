# Web Tool 卡片接入文档中心

本目录用于管理 `Web UI 接入 Tool 卡片` 的完整实施文档体系。

## 目标
- 将实施拆分为可追踪的阶段，避免边做边改导致返工。
- 让任意同学通过文档就能定位当前阶段、任务、验收标准和证据。
- 统一决策记录方式，避免关键结论散落在聊天与口头沟通中。

## 快速开始
1. 阅读 [INDEX.md](./INDEX.md) 了解当前阶段地图。
2. 阅读 [WORKFLOW.md](./WORKFLOW.md) 按标准流程推进。
3. 进入对应阶段目录，按 `README -> TODO -> ACCEPTANCE -> DECISIONS` 顺序执行。

## 阶段完成判定
单个阶段被标记为 `Done` 必须同时满足：
- 该阶段 `TODO.md` 所有条目已勾选。
- 该阶段 `ACCEPTANCE.md` 所有验收项通过并附证据。
- 该阶段 `DECISIONS.md` 不存在未决关键决策。

## 文档约定
- 语言：中文为主，必要术语可中英并列。
- 状态：`Not Started / In Progress / Blocked / Done`。
- 证据：每个完成任务需要附 `PR / 测试结果 / 截图 / 日志` 至少一种。
