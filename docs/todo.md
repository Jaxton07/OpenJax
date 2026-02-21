# OpenJax TODO（按顺序执行）

1. 查看日志中工具调用了三次的原因 [x] 
2. 对比codex 的tool 实现，看看跟我们有啥区别，排查patch_tool 的失败原因
3. 添加Freeform 工具支持 [x]
4. 拆分openjax-core/src/tools/README.md [x]

5. 优化日志，增加用户输入信息
6. 优化python tui 的权限请求



## 下一步建议（M4 -> M5）

1. 补齐 CLI 级最小 e2e 测试脚本（`apply_patch` + 审批交互 + 事件输出一致性）。
2. 扩展 `apply_patch` 语法覆盖面，并补相应失败回滚用例。
3. 把工具调用从 `tool:` 文本协议升级为结构化 tool call 协议。
4. 开始整理 M5 所需参数体系与 `config.toml`。
