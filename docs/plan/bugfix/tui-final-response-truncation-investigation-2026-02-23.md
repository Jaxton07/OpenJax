# Python TUI 最终回复显示不全问题排查总结（2026-02-23，持续更新）

## 1. 问题定义（历史现象）

在 `python/openjax_tui` 交互中，曾稳定出现以下现象：

1. Core 侧 `final_response` 完整，`output_truncated=false`。
2. TUI debug 日志里 `assistant_delta`/`assistant_message` 也完整。
3. 终端界面渲染只显示前半段，向上可滚、向下看不到后续行。
4. 常见于内容接近终端底部时（视口/滚动相关特征明显）。

结论：数据链路完整，问题集中在 Python TUI 渲染和滚动层。

---

## 2. 当前状态快照（2026-02-23 夜间）

1. 用户多轮实测反馈：当前版本暂未再出现“底部内容不显示”。
2. 本轮同时发现并修复了一个独立问题：prompt_toolkit 模式启动后会异常退出（非用户主动退出）。
3. 对 `full_screen=True` 的方向结论更新为：不是当前主因，不作为短期主线。

---

## 3. 本轮已完成改动（已落地）

### 3.1 Turn 级历史块 upsert（避免流式与最终消息错位）

1. `assistant_message` 到达后，按 turn 维度覆盖历史块（authoritative final）。
2. 引入 `turn_block_index`/`assistant_message_by_turn`，避免同一 turn 重复 append。
3. `assistant_delta` 与 `assistant_message` 共用 upsert 路径，减少历史块漂移。

涉及文件：

- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/assistant_render.py`
- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/state.py`

### 3.2 历史区滚动与窗口管理重构（prompt_toolkit）

1. 增加 auto-follow/manual-scroll 状态：
   - `history_auto_follow`
   - `history_manual_scroll`
2. 增加 `PageUp/PageDown` 手动浏览逻辑。
3. auto-follow 时将滚动推到尾部，turn 完成时增加一次 tail 可见性兜底。
4. 新增历史窗口压缩策略，超长历史自动裁剪并输出到终端 scrollback：
   - 环境变量 `OPENJAX_TUI_HISTORY_WINDOW_LINES`（默认 500，最小 120）。

涉及文件：

- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/app.py`
- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/state.py`

### 3.3 诊断增强（便于后续复盘）

1. 事件日志增加关键信息：
   - `assistant_delta`: `delta_len/delta_preview/delta_truncated`
   - `assistant_message`: `content_len/content_preview/content_truncated`
2. 增加 streamed 内容与 final 内容不一致告警日志。
3. 增加 fatal traceback 记录，便于定位异常退出。

涉及文件：

- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/app.py`

### 3.4 启动闪退修复（已确认）

1. 现象：启动后程序自动退出，shell 留下 `^[[21;1R` 等终端响应残留。
2. 根因：对 `history_window.allow_scroll_beyond_bottom` 进行了错误赋值，触发 prompt_toolkit 渲染期异常：`TypeError: 'bool' object is not callable`。
3. 修复：移除该错误赋值；并在 prompt_toolkit loop 异常时记录 traceback，必要时回退到 basic 输入后端。

涉及文件：

- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/app.py`

### 3.5 Slash completer 边界崩溃修复

1. 空输入时 `split(...)[0]` 可能触发 `list index out of range`。
2. 新增 `_leading_token()` 统一处理空输入。
3. 补充单测覆盖空输入与 `/` 输入补全。

涉及文件：

- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/slash_commands.py`
- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/tests/test_startup_config.py`

---

## 4. 验证结果（本轮）

1. Python TUI 单测通过：

```bash
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest discover -s python/openjax_tui/tests -v
```

结果：`Ran 64 tests ... OK`。

2. 手动启动验证（debug 模式）：

```bash
OPENJAX_TUI_DEBUG=1 PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m openjax_tui
```

结果：修复后可持续运行，`/exit` 正常退出。

3. 链路一致性验证：
   - `.openjax/logs/openjax.log` 中 `final_response` 仍为完整，`output_truncated=false`。
   - `.openjax/logs/openjax_tui.log` 可见完整 assistant 事件与调试字段。

---

## 5. 当前判断（阶段性）

1. “底部不显示”问题当前阶段可能已被历史区重构与滚动策略修复覆盖，但还需要继续观察。
2. `full_screen=True` 不是必要条件，且与本次核心问题关联度不高，暂不作为优先路线。
3. 启动闪退问题已定位并修复，属于独立但高优先级稳定性问题。

---

## 6. 后续优化待办（下一轮）

1. 增加可重复的长文本压力样例（含 CJK 宽字符、混合换行、超长块）做回归。
2. 增加“历史区尾部一致性”调试指标（渲染后行数/滚动位置快照）。
3. 评估把历史渲染从“文本块拼接”升级为结构化消息列表（中期改造）。
4. 视需要补一个 smoke 脚本，自动验证“可向下滚动到最新行”。

---

## 7. 快速接续清单（给下次排查）

1. 先跑 debug 启动命令，确认无闪退：
   - `OPENJAX_TUI_DEBUG=1 PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m openjax_tui`
2. 若问题复现，先对照两个日志：
   - `/Users/ericw/work/code/ai/openJax/.openjax/logs/openjax.log`
   - `/Users/ericw/work/code/ai/openJax/.openjax/logs/openjax_tui.log`
3. 若日志显示内容完整但界面不完整，优先检查：
   - `history_auto_follow/history_manual_scroll` 状态变化
   - `history_window` 的 render_info 与 vertical_scroll 同步
   - 历史窗口压缩阈值 `OPENJAX_TUI_HISTORY_WINDOW_LINES`

---

## 8. 参考文件（当前实现）

- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/app.py`
- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/assistant_render.py`
- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/state.py`
- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/src/openjax_tui/slash_commands.py`
- `/Users/ericw/work/code/ai/openJax/python/openjax_tui/tests/test_startup_config.py`
- `/Users/ericw/work/code/ai/openJax/.openjax/logs/openjax.log`
- `/Users/ericw/work/code/ai/openJax/.openjax/logs/openjax_tui.log`
