# Python TUI 回归报告模板

## 1. 基本信息

1. 日期：
2. 测试人：
3. 机器/系统：
4. Git 提交：

## 2. 环境版本

1. Python 版本：
2. tmux 版本：
3. zellij 版本：
4. openjaxd 启动方式（`target/debug/openjaxd` 或 `cargo run`）：

## 3. 回归结果

| 项目 | 环境 | 结果（PASS/FAIL） | 备注 |
|---|---|---|---|
| 基础启动 | tmux | PASS |  |
| 事件流回归 | tmux | PASS |  |
| 审批回归 | tmux |  |  |
| 分离恢复/切 pane | tmux | PASS |  |
| 基础启动 | zellij |  |  |
| 事件流回归 | zellij |  |  |
| 审批回归 | zellij |  |  |
| 切 tab/pane | zellij |  |  |

## 4. 失败项详情

按失败项逐条记录：

1. 失败项：
2. 复现步骤：
3. 预期行为：
4. 实际行为：
5. 日志片段：

## 5. 结论

1. 是否满足阶段 E 验收：`是/否`
2. 需要修复的阻塞问题：
