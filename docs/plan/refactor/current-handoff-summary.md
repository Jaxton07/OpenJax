# OpenJax 重构当前节点交接（2026-02-20）

## 1. 我们在做什么

将项目从“Rust CLI 主导”重构为“Rust 内核 + Python 外层”：
1. Rust 负责内核安全边界与执行能力（Agent/Tools/Sandbox/Approval）。
2. Python 负责交互层与后续平台扩展（先 SDK，再 TUI）。

---

## 2. 已完成内容（截至当前）

### A-B 阶段
1. 重构计划与文档基线建立完成。
2. 跨语言协议 v1 草案完成（JSONL/stdin/stdout）。
3. schema 与示例样例已落地并通过校验。

### C 阶段（openjaxd）
1. `openjaxd` crate 已落地并接入 workspace。
2. 已打通：`start_session / stream_events / submit_turn / resolve_approval / shutdown_session`。
3. 审批超时与 EOF 清理、结构化日志已补齐。
4. Rust 侧协议集成测试已通过。

### D 阶段（Python SDK）
1. `python/openjax_sdk` MVP 已落地（async client）。
2. 已支持 daemon 生命周期、事件订阅、审批回传、delta 聚合。
3. Python 集成测试已通过。

### E 阶段（Python TUI，进行中）
1. `python/openjax_tui` 已可用，支持输入、流式输出、审批。
2. 已修复输入冲突问题（方向键乱码、中文输入、输入残留）。
3. 输出已做精简：`you>` / `assistant>` / `tool>` / `approval>`。
4. smoke 脚本与回归清单已补齐。

---

## 3. 当前阶段与状态

当前阶段：`阶段 E（Python TUI MVP）`  
状态：`功能可用，收尾优化中`

剩余关键项：
1. tmux/zellij 实机回归结果落档（手工）。
2. Python TUI 视觉和布局继续优化（参考 Claude Code 风格）。

---

## 4. 已知问题 / 注意事项

1. zellij 中 `Command+C` 复制行为属于复用器快捷键差异，不是 OpenJax TUI 逻辑缺陷。
2. 目前 Python TUI 是“精简终端视图”，不是全屏布局（后续可演进到顶部状态栏 + 底部固定输入区）。
3. 单 TUI 多会话编排未实现（当前仍是单会话模型）。

---

## 5. 推荐下一步（短期）

1. 完成 tmux/zellij 回归并填报告：
   - `docs/tui/python-tui-regression-checklist.md`
   - `docs/tui/python-tui-regression-report-template.md`
2. 继续做 Python TUI UI 优化：
   - 更平滑的流式刷新与段落换行策略
   - 工具事件折叠/摘要
   - 顶部状态信息与底部输入区布局
3. 阶段 E 收尾后再评估阶段 F（Telegram）优先级。

---

## 6. 当前关键目录（简版）

```text
openjax-core/                    # Rust 内核
openjax-protocol/                # 协议类型
openjaxd/                        # Rust daemon
python/
  openjax_sdk/                   # Python SDK (MVP)
  openjax_tui/                   # Python TUI (MVP)
docs/
  protocol/v1/                   # 协议文档 + schema + examples
  plan/refactor/                 # 重构计划与交接文档
  tui/                           # TUI 回归清单与报告模板
smoke_test/
  python_tui_smoke.sh
  python_tui_mux_check.sh
```

---

## 7. 常用验证命令

```bash
# Rust daemon
cargo test -p openjaxd

# Python SDK
PYTHONPATH=python/openjax_sdk/src \
python3 -m unittest discover -s python/openjax_sdk/tests -v

# Python TUI
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m unittest discover -s python/openjax_tui/tests -v

# TUI smoke
zsh smoke_test/python_tui_smoke.sh
zsh smoke_test/python_tui_mux_check.sh
```
