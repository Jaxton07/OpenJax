# OpenJax TODO（按顺序执行）

19. 大logo 加阴影，更立体





## TUI v2 TODO
[] 3. 添加多行输入支持





## Rust TUI
3. 等待模型回复时，thinking 状态无法ctrl + C 退出，添加esc 终止当前模型的操作进程
4. markdown 支持
7. 运行时按esc 终止
8. 沙箱权限切换


设计未来新的目录架构



## Web UI
3. 连接状态的颜色显示


## core
2. subAgent
3. 强化沙箱
4. 接入telegram
5. 一个规划多个tool 或者shell 调用，然后一起审批，等待所有执行完成一次返回所有结果
6. 本地数据库
7. 长期记忆存错与搜寻
8. web search tool
9. 更完整优化的provider 支持，针对不同provider 适配不同的api 模式，web UI 支持配置llm provider 和 api key, 并存储api key 到数据库


### skills
1. skills 执行完后要清除对应的无关上下文，或者只生产部分摘要，后期引入subAgent 后skill 可以丢给subAgent 去执行，避免污染主对话的上下文
