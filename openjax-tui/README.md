# openjax-tui

基于 Rust + Ratatui 的终端 UI，为 OpenJax 提供实时对话、审批弹层与工具事件显示能力。

## 项目结构

```
openjax-tui/
├── README.md                        # 项目文档
├── Cargo.toml                       # crate 配置与依赖
├── src/
│   ├── lib.rs                       # 模块导出入口
│   ├── main.rs                      # 程序入口（终端模式、agent 循环、事件汇总）
│   ├── app.rs                       # 应用层事件处理与整体渲染编排
│   ├── chatwidget.rs                # Transcript 渲染与流式尾部显示
│   ├── app_event.rs                 # 应用事件定义（键盘、核心事件、退出）
│   ├── approval.rs                  # TUI 审批处理器（请求队列与回传）
│   ├── tui.rs                       # 终端模式控制与键盘事件映射
│   ├── bottom_pane/                 # 输入框与弹层（slash/approval/footer）
│   │   ├── mod.rs
│   │   ├── chat_composer.rs
│   │   ├── slash_commands.rs
│   │   ├── command_popup.rs
│   │   ├── approval_overlay.rs
│   │   └── footer.rs
│   ├── state/                       # 状态分层与协议事件映射
│   │   ├── mod.rs
│   │   ├── app_state.rs
│   │   ├── turn_state.rs
│   │   ├── input_state.rs
│   │   ├── approval_state.rs
│   │   └── event_mapper.rs
│   ├── render/
│   │   ├── mod.rs                   # render 子模块导出
│   │   ├── markdown.rs              # Markdown 到纯文本渲染
│   │   ├── renderable.rs            # 可渲染接口
│   │   └── theme.rs                 # 主题样式
│   └── ui/
│       ├── mod.rs                   # UI 子模块导出
│       ├── chat_view.rs             # 聊天区渲染
│       ├── composer.rs              # 输入框渲染与光标定位
│       ├── logo.rs                  # 启动 Logo 渲染
│       ├── overlay_approval.rs      # 审批弹层数据结构
│       └── status_bar.rs            # 底部状态栏渲染
└── tests/
    ├── m1_app_state.rs
    ├── m2_event_mapping.rs
    ├── m3_render_smoke.rs
    ├── m4_approval_overlay.rs
    ├── m5_streaming_merge.rs
    ├── m6_markdown_render.rs
    ├── m7_keymap.rs
    ├── m8_terminal_restore.rs
    ├── m9_tui_approval_handler.rs
    ├── m10_chat_view_layout.rs
    └── m11_chat_scroll_visual.rs
```

## 各模块功能介绍

### 核心模块

| 模块 | 功能描述 |
|------|----------|
| `main.rs` | 初始化终端 raw/alt 模式、构造 `Agent` 和 `TuiApprovalHandler`、驱动输入事件与核心事件循环 |
| `app.rs` | 统一路由输入事件/Core 事件，处理 slash/审批优先级与页面渲染 |
| `chatwidget.rs` | 管理 transcript 视图（用户/助手/tool/system）与流式显示 |
| `state/*` | 拆分状态层：turn/input/approval/transcript，并通过 `event_mapper` 映射协议事件 |
| `approval.rs` | 实现 `ApprovalHandler`，在异步通道中管理待审批请求与用户决策 |
| `tui.rs` | crossterm 事件映射（含 `Esc`）、终端模式切换与恢复 |
| `bottom_pane/*` | 输入框状态机、slash 候选、审批弹层和 footer 状态提示 |

### 渲染与 UI 模块

| 模块 | 功能描述 |
|------|----------|
| `render/markdown.rs` | 使用 `pulldown-cmark` 将 Markdown 转换为 TUI 可读文本 |
| `render/theme.rs` | 定义角色颜色与标题样式 |
| `ui/chat_view.rs` | 渲染消息行（按 role 着色） |
| `ui/composer.rs` | 输入行与光标偏移计算，支持宽字符宽度 |
| `ui/logo.rs` | 顶部 OPENJAX 彩色 Logo |
| `ui/status_bar.rs` | 显示快捷键和 runtime 上下文（model/approval/sandbox） |

## 运行

在仓库根目录执行：

```bash
zsh -lc "cargo run -q -p openjax-tui"
```

## 交互快捷键

| 操作 | 说明 |
|------|------|
| `Enter` | 提交输入 |
| `Backspace` | 删除字符 |
| `Left` / `Right` | 光标左右移动 |
| `Up` / `Down` | 输入历史切换 |
| `PageUp` / `PageDown` | 聊天区翻页 |
| `Home` / `End` | 跳转到聊天顶部/底部 |
| `?` | 显示/隐藏帮助面板 |
| `y` / `n` | 对当前审批弹层快速通过/拒绝 |
| `Ctrl-C` | 退出应用 |

> 说明：在 alt-screen 模式下，终端原生滚动历史通常不可用，请使用 `PageUp / PageDown / Home / End` 浏览聊天区内容。

## 环境变量配置

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `OPENJAX_TUI_ALT_SCREEN` | 终端模式：`auto` / `always` / `never` | `auto` |
| `OPENJAX_TUI_SHOW_SYSTEM_EVENTS` | 是否显示系统事件消息（`1/true/yes`） | 关闭 |
| `OPENJAX_MODEL` | 模型标识（由 `openjax-core` 读取） | core 默认 |
| `OPENJAX_SANDBOX_MODE` | 沙箱模式（由 `openjax-core` 读取） | core 默认 |
| `OPENJAX_APPROVAL_POLICY` | 审批策略（由 `openjax-core` 读取） | core 默认 |

## 测试

运行 TUI 测试：

```bash
zsh -lc "cargo test -p openjax-tui"
```

运行关键集成测试：

```bash
zsh -lc "cargo test -p openjax-tui --test m1_app_state"
zsh -lc "cargo test -p openjax-tui --test m4_approval_overlay"
```

## 架构特点

- **事件驱动 UI**：键盘输入事件与 core 事件统一进入 `AppEvent` 流
- **状态集中管理**：视图渲染完全依赖 `AppState`
- **终端兼容性**：支持 alt-screen 自动判定与恢复流程
- **审批闭环**：审批请求从 core 到弹层再到回传链路完整
