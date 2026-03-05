# OpenJax TODO（按顺序执行）

1. 查看日志中工具调用了三次的原因 [x] 
<!-- 2. 对比codex 的tool 实现，看看跟我们有啥区别，排查patch_tool 的失败原因 -->
<!-- 3. 添加Freeform 工具支持 [x]
4. 拆分openjax-core/src/tools/README.md [x]

5. 优化日志，增加用户输入信息[x]
6. 优化python tui 的权限请求一阶段[x] -->
<!-- 7. 优化python tui 的markdown 显示支持 -->
<!-- 8. 优化python tui 的状态动画，Thinking, Reading, Updating [x] -->
<!-- 9. 修改优化openjax 的系统提示词, 并且增强apply_patch 工具的提示词，增加格式保持一致的指令 [x] -->
10. 补充ctrl + C 退出时报错的覆盖测试用例，每次修改后都跑
<!-- 11. 添加/ 命令提示 -->
12. 添加 多行输入/换行 支持
<!-- 13. 继续优化tui, 增加历史会话中用户输入的消息的区分度，优化输入框样式[x] -->
<!-- 14. 优化权限申请和审批逻辑，同一个操作已经同意权限后重试不再需要重新获取权限 -->
<!-- 15. 拆分 python/openjax_tui/src/openjax_tui/app.py [x] -->
16. 简化python tui 启动命令
<!-- 17. 修复现在输入框不能输入y 和 n 的bug, 非权限审批模式下不禁用输入 [x] -->
<!-- 18. 修复edit_file_range 的修改失败的bug[x] -->
19. 大logo 加阴影，更立体
<!-- 20. 优化tool 代用时间显示，超过1000 ms 就换成秒，超过60 秒就换成分钟 + 秒[x]
21. 确认是否可以简化final writer[x]
22. 修复优化用户消息[x] -->




## TUI v2 TODO
[x] 1. 添加状态等待动画
[] 2. 添加流式输出支持
[] 3. 添加多行输入支持
[] 4. 添加logo
[x] 5. 添加终端的选择复制支持
[x] 6. 优化markdown 换行显示问题
[] 7. 检查权限审批逻辑是否需要优化，记录审批逻辑文档
[] 8. 出界的历史记录看不到，要不要刷到shell 终端显示




## Rust TUI
1. apply_patch 成功后再次调用的问题
2. 打印顶部版本信息和logo
3. 等待模型回复时，thinking 状态无法ctrl + C 退出，添加esc 终止当前模型的操作进程
4. markdown 支持
5. /cmd 提示面板


设计未来新的目录架构


## core
1. skills
2. subAgent
3. 强化沙箱
4. 接入telegram


### skills
1. skills 执行完后要清除对应的无关上下文，或者只生产部分摘要，后期引入subAgent 后skill 可以丢给subAgent 去执行，避免污染主对话的上下文