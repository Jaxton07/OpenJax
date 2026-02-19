# Codex TUI 项目架构介绍

本文档介绍 Codex TUI (Terminal User Interface) 的整体架构设计，旨在为后续开发类似 TUI 项目提供参考。

## 技术栈

| 组件 | 技术选型 | 说明 |
|------|----------|------|
| TUI 框架 | [ratatui](https://github.com/ratatui-org/ratatui) | Rust 终端 UI 框架，提供 widget、layout、buffer 等核心抽象 |
| 终端后端 | crossterm | 跨平台终端控制库，处理原始模式、事件流、alternate screen |
| 异步运行时 | tokio | 处理事件流、后台任务、异步 I/O |
| Markdown 解析 | pulldown-cmark | 用于渲染 Markdown 内容 |
| 日志追踪 | tracing + tracing-subscriber | 结构化日志和追踪 |

## 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         lib.rs (入口)                           │
│  - 配置加载、认证管理、日志初始化                                    │
│  - 启动 App 主循环                                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                          App (app.rs)                           │
│  - 主事件循环                                                    │
│  - 管理全局状态、线程管理器                                         │
│  - 协调 ChatWidget 和 BottomPane                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│     Tui         │  │   ChatWidget    │  │   BottomPane    │
│   (tui.rs)      │  │ (chatwidget.rs) │  │ (bottom_pane/)  │
│                 │  │                 │  │                 │
│ - 终端抽象      │  │ - 聊天历史显示   │  │ - 输入框        │
│ - 事件流        │  │ - 流式输出处理   │  │ - 弹窗/覆盖层   │
│ - alternate     │  │ - 历史单元格     │  │ - 状态栏        │
│   screen 管理   │  │ - 工具调用显示   │  │ - 快捷键提示    │
└─────────────────┘  └─────────────────┘  └─────────────────┘
          │                   │                   │
          └───────────────────┼───────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       渲染层 (render/)                           │
│  - Markdown 渲染 (markdown_render.rs)                           │
│  - 语法高亮 (highlight.rs)                                       │
│  - 行工具函数 (line_utils.rs)                                    │
│  - 可渲染 trait (renderable.rs)                                  │
└─────────────────────────────────────────────────────────────────┘
```

## 核心模块详解

### 1. Tui 模块 ([tui.rs](file:///Users/ericw/work/code/ai/codex/codex-rs/tui/src/tui.rs))

终端抽象层，封装了与终端交互的所有底层操作。

**核心职责：**
- 终端初始化/恢复（raw mode、bracketed paste、keyboard enhancement）
- Alternate screen 管理（全屏模式切换）
- 事件流管理（键盘、粘贴、重绘）
- 桌面通知支持

**关键设计：**
```rust
pub struct Tui {
    terminal: Terminal,           // ratatui 终端实例
    event_broker: EventBroker,    // 事件分发器
    alt_screen_active: Arc<AtomicBool>,  // alternate screen 状态
    alt_screen_enabled: bool,     // 是否启用 alternate screen
}
```

**Alternate Screen 策略：**
- `auto`（默认）：自动检测终端复用器，在 Zellij 中禁用以保留滚动历史
- `always`：始终使用全屏模式
- `never`：始终使用内联模式

详见：[tui-alternate-screen.md](file:///Users/ericw/work/code/ai/codex/docs/tui-alternate-screen.md)

### 2. App 模块 ([app.rs](file:///Users/ericw/work/code/ai/codex/codex-rs/tui/src/app.rs))

主应用循环，协调所有组件。

**核心职责：**
- 事件循环（TUI 事件 + 后台事件）
- 线程管理器（与 AI 模型通信）
- 配置热更新
- 退出处理

**事件流：**
```
TuiEvent (键盘/粘贴/重绘)
        │
        ▼
App::handle_event()
        │
        ├── ChatWidget::handle_key_event()
        ├── BottomPane::handle_key_event()
        └── 全局快捷键处理
```

### 3. ChatWidget 模块 ([chatwidget.rs](file:///Users/ericw/work/code/ai/codex/codex-rs/tui/src/chatwidget.rs))

聊天界面核心，管理对话历史和流式输出。

**核心职责：**
- 消息历史显示（HistoryCell 列表）
- 流式输出处理（streaming/）
- 工具调用显示（exec_cell/）
- 状态同步（token 使用量、速率限制）

**关键数据结构：**
```rust
pub struct ChatWidget {
    history: Vec<Box<dyn HistoryCell>>,  // 历史消息
    active_cell: Option<Box<dyn HistoryCell>>,  // 当前活跃单元格
    streaming_state: StreamState,        // 流式输出状态
    // ...
}
```

### 4. BottomPane 模块 ([bottom_pane/](file:///Users/ericw/work/code/ai/codex/codex-rs/tui/src/bottom_pane/))

底部交互面板，包含输入框和各种弹窗。

**子模块：**
| 模块 | 功能 |
|------|------|
| `chat_composer.rs` | 输入框核心，支持多行编辑、历史导航、粘贴检测 |
| `footer.rs` | 底部状态栏，显示快捷键提示 |
| `approval_overlay.rs` | 命令审批覆盖层 |
| `request_user_input/` | 用户输入请求覆盖层 |
| `skill_popup.rs` | Skill 选择弹窗 |
| `file_search_popup.rs` | 文件搜索弹窗 |
| `list_selection_view.rs` | 通用列表选择视图 |

**Chat Composer 状态机：**
详见：[tui-chat-composer.md](file:///Users/ericw/work/code/ai/codex/docs/tui-chat-composer.md)

**Paste Burst 检测：**
用于处理 Windows 终端中不稳定的粘贴事件，防止粘贴过程中触发快捷键或意外提交。

### 5. Streaming 模块 ([streaming/](file:///Users/ericw/work/code/ai/codex/codex-rs/tui/src/streaming/))

流式输出处理，实现自适应的输出节奏控制。

**核心组件：**
| 文件 | 功能 |
|------|------|
| `mod.rs` | StreamState - 流状态管理，队列操作 |
| `chunking.rs` | 自适应分块策略，Smooth/CatchUp 模式切换 |
| `commit_tick.rs` | 提交节奏编排 |
| `controller.rs` | 队列/排放原语 |

**两种模式：**
- **Smooth**：正常模式，每 tick 提交一行，提供打字机效果
- **CatchUp**：追赶模式，当队列积压时批量提交，减少延迟

详见：
- [tui-stream-chunking-review.md](file:///Users/ericw/work/code/ai/codex/docs/tui-stream-chunking-review.md)
- [tui-stream-chunking-tuning.md](file:///Users/ericw/work/code/ai/codex/docs/tui-stream-chunking-tuning.md)
- [tui-stream-chunking-validation.md](file:///Users/ericw/work/code/ai/codex/docs/tui-stream-chunking-validation.md)

### 6. HistoryCell 模块 ([history_cell.rs](file:///Users/ericw/work/code/ai/codex/codex-rs/tui/src/history_cell.rs))

对话历史单元，每条消息都是一个 HistoryCell。

**核心 trait：**
```rust
pub trait HistoryCell: Debug + Send + Sync {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>>;
    fn desired_height(&self, width: u16) -> u16;
    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>>;
}
```

**实现类型：**
- `PlainHistoryCell` - 普通文本消息
- `AgentMessageCell` - AI 响应消息
- `ExecCell` - 命令执行单元
- `McpToolCallCell` - MCP 工具调用
- `WebSearchCell` - 网页搜索结果

### 7. Render 模块 ([render/](file:///Users/ericw/work/code/ai/codex/codex-rs/tui/src/render/))

渲染工具集。

**子模块：**
| 模块 | 功能 |
|------|------|
| `highlight.rs` | Bash 语法高亮（使用 tree-sitter） |
| `line_utils.rs` | 行操作工具函数 |
| `renderable.rs` | Renderable trait，统一渲染接口 |

### 8. Markdown 渲染 ([markdown_render.rs](file:///Users/ericw/work/code/ai/codex/codex-rs/tui/src/markdown_render.rs))

将 Markdown 文本转换为 ratatui Text。

**支持的元素：**
- 标题（h1-h6）
- 代码块（带语法高亮）
- 列表（有序/无序）
- 链接、强调、加粗
- 引用块

## 关键设计模式

### 1. 事件驱动架构

```
用户输入 ──► TuiEventStream ──► App 事件循环 ──► 组件处理
                                        │
后台任务 ◄─────────────────────────────┘
(AI 响应)
```

### 2. 组件化渲染

所有可渲染组件实现 `Renderable` trait：
```rust
pub trait Renderable {
    fn render(&mut self, area: Rect, buf: &mut Buffer);
}
```

### 3. 状态机模式

- ChatComposer 的 popup 状态（None/Command/File/Skill）
- PasteBurst 的粘贴检测状态（Idle/Pending/Active）
- Streaming 的模式切换（Smooth/CatchUp）

### 4. 异步任务管理

使用 `tokio::sync::mpsc` 进行任务间通信：
- `AppEvent` - 应用级事件
- `Event` - 协议级事件（来自 AI 模型）

## 文件结构索引

```
codex-rs/tui/
├── Cargo.toml                    # 依赖配置
├── src/
│   ├── lib.rs                    # 入口点，配置加载
│   ├── main.rs                   # 二进制入口
│   ├── tui.rs                    # 终端抽象层
│   ├── app.rs                    # 主应用循环
│   ├── app_event.rs              # 应用事件定义
│   ├── chatwidget.rs             # 聊天界面核心
│   ├── history_cell.rs           # 历史消息单元
│   ├── markdown_render.rs        # Markdown 渲染
│   ├── markdown_stream.rs        # 流式 Markdown 解析
│   ├── style.rs                  # 样式定义
│   ├── color.rs                  # 颜色工具
│   │
│   ├── bottom_pane/              # 底部面板
│   │   ├── mod.rs
│   │   ├── chat_composer.rs      # 输入框
│   │   ├── footer.rs             # 状态栏
│   │   ├── approval_overlay.rs   # 审批覆盖层
│   │   ├── paste_burst.rs        # 粘贴检测
│   │   ├── request_user_input/   # 用户输入请求
│   │   ├── skill_popup.rs        # Skill 弹窗
│   │   └── ...
│   │
│   ├── streaming/                # 流式输出
│   │   ├── mod.rs                # StreamState
│   │   ├── chunking.rs           # 自适应分块
│   │   ├── commit_tick.rs        # 提交节奏
│   │   └── controller.rs         # 控制器
│   │
│   ├── exec_cell/                # 命令执行单元
│   │   ├── mod.rs
│   │   ├── model.rs
│   │   └── render.rs
│   │
│   ├── render/                   # 渲染工具
│   │   ├── mod.rs
│   │   ├── highlight.rs          # 语法高亮
│   │   ├── line_utils.rs         # 行工具
│   │   └── renderable.rs         # 渲染 trait
│   │
│   ├── status/                   # 状态显示
│   │   ├── mod.rs
│   │   ├── account.rs            # 账户信息
│   │   ├── rate_limits.rs        # 速率限制
│   │   └── ...
│   │
│   ├── onboarding/               # 引导流程
│   ├── notifications/            # 桌面通知
│   └── ...
│
└── frames/                       # ASCII 动画帧
```

## 依赖关系图

```
codex-tui
    │
    ├── ratatui (TUI 框架)
    │
    ├── crossterm (终端后端)
    │
    ├── codex-core (核心业务逻辑)
    │   ├── config
    │   ├── protocol
    │   └── ...
    │
    ├── codex-protocol (协议定义)
    │
    ├── pulldown-cmark (Markdown)
    │
    ├── tree-sitter-bash (语法高亮)
    │
    └── tokio (异步运行时)
```

## 开发参考

### 添加新的 HistoryCell 类型

1. 在 `history_cell.rs` 中定义新类型
2. 实现 `HistoryCell` trait
3. 在 `ChatWidget` 中处理相应事件并创建实例

### 添加新的弹窗/覆盖层

1. 在 `bottom_pane/` 中创建新模块
2. 实现 `BottomPaneView` trait
3. 在 `BottomPane` 中添加状态管理和路由

### 自定义渲染

1. 使用 `render/renderable.rs` 中的 trait
2. 利用 `render/line_utils.rs` 的工具函数
3. 参考 `markdown_render.rs` 的样式系统

### 流式输出调优

参考 [tui-stream-chunking-tuning.md](file:///Users/ericw/work/code/ai/codex/docs/tui-stream-chunking-tuning.md) 中的调优指南。

## 相关文档

- [Alternate Screen 设计](file:///Users/ericw/work/code/ai/codex/docs/tui-alternate-screen.md)
- [Chat Composer 状态机](file:///Users/ericw/work/code/ai/codex/docs/tui-chat-composer.md)
- [用户输入请求覆盖层](file:///Users/ericw/work/code/ai/codex/docs/tui-request-user-input.md)
- [流式分块机制](file:///Users/ericw/work/code/ai/codex/docs/tui-stream-chunking-review.md)
- [流式分块调优](file:///Users/ericw/work/code/ai/codex/docs/tui-stream-chunking-tuning.md)
- [流式分块验证](file:///Users/ericw/work/code/ai/codex/docs/tui-stream-chunking-validation.md)
