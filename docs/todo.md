# OpenJax TODO（按顺序执行）

1. 查看日志中工具调用了三次的原因 [x] 
2. 对比codex 的tool 实现，看看跟我们有啥区别，排查patch_tool 的失败原因
<!-- 3. 添加Freeform 工具支持 [x]
4. 拆分openjax-core/src/tools/README.md [x]

5. 优化日志，增加用户输入信息[x]
6. 优化python tui 的权限请求一阶段[x] -->
7. 优化python tui 的markdown 显示支持
<!-- 8. 优化python tui 的状态动画，Thinking, Reading, Updating [x] -->
<!-- 9. 修改优化openjax 的系统提示词, 并且增强apply_patch 工具的提示词，增加格式保持一致的指令 [x] -->
10. 补充ctrl + C 退出时报错的覆盖测试用例，每次修改后都跑
11. 添加/ 命令提示
12. 添加 多行输入/换行 支持
13. 继续优化tui, 增加历史会话中用户输入的消息的区分度，优化输入框样式
<!-- 14. 优化权限申请和审批逻辑，同一个操作已经同意权限后重试不再需要重新获取权限 -->
<!-- 15. 拆分 python/openjax_tui/src/openjax_tui/app.py [x] -->
16. 简化python tui 启动命令
<!-- 17. 修复现在输入框不能输入y 和 n 的bug, 非权限审批模式下不禁用输入 [x] -->
<!-- 18. 修复edit_file_range 的修改失败的bug[x] -->
19. 大logo 加阴影，更立体
20. 优化tool 代用时间显示，超过1000 ms 就换成秒，超过60 秒就换成分钟 + 秒




