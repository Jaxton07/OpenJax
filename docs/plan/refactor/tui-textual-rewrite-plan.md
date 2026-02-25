# OpenJax TUI Textual 重写计划

## 1. 背景与动机

### 1.1 当前方案 (prompt_toolkit) 的问题

| 问题 | 说明 |
|------|------|
| 复杂度高 | 需要手动管理视口适配器、scrollback flush、布局计算 |
| 代码冗余 | Pilot/TextArea 双视口实现，维护成本高 |
| 状态分散 | 动画、审批、流式状态分散在多个模块 |
| 降级复杂 | 需要显式处理 basic 后端回退 |
| 样式受限 | CSS-like 样式系统缺失，界面美化困难 |

### 1.2 Textual 的优势

| 特性 | Textual 优势 |
|------|--------------|
| **响应式编程** | `reactive` 装饰器自动处理状态变化和 UI 刷新 |
| **组件化架构** | Widget 系统，职责清晰，易于复用 |
| **CSS 样式** | 类 CSS 样式系统，支持主题、响应式布局 |
| **内置组件** | Input、RichLog、Static、Container 等开箱即用 |
| **事件系统** | 声明式事件处理 (`on_button_pressed`, `on_input_changed`) |
| **自动刷新** | 状态变化自动触发重绘，无需手动管理 |
| **数据绑定** | `data_bind` 实现父子组件状态同步 |
| **测试支持** | `Pilot` 测试工具，支持自动化 UI 测试 |
| **命令面板** | 内置 `CommandPalette` 支持模糊搜索 |

---

## 2. 技术方案对比

### 2.1 prompt_toolkit vs Textual

| 维度 | prompt_toolkit | Textual |
|------|----------------|---------|
| **架构模式** | 命令式，手动管理 | 声明式，响应式 |
| **状态管理** | 集中式 `AppState` + 手动刷新 | 分布式 `reactive` + 自动刷新 |
| **布局系统** | 手动计算 Dimension | CSS Grid/Flex 自动布局 |
| **组件复用** | 函数闭包 | Widget 类继承 |
| **事件处理** | 回调函数 | 装饰器方法 (`@on`) |
| **动画实现** | 手动 `asyncio.Task` | 内置 `set_interval` |
| **日志显示** | 自定义视口适配器 | `RichLog` 组件 |
| **输入处理** | 手动队列管理 | `Input` 组件事件 |
| **命令面板** | 手动实现 | 内置 `CommandPalette` |
| **代码量** | ~3000 行 | 预计 ~1500 行 |

### 2.2 核心概念映射

| prompt_toolkit 概念 | Textual 对应 |
|---------------------|--------------|
| `AppState` | `App` 类的 `reactive` 属性 |
| `HistoryViewportAdapter` | `RichLog` / `Static` + `ScrollableContainer` |
| `prompt_invalidator` | 自动响应式刷新 |
| `approval_toolbar` | `ApprovalScreen` 全屏审批 |
| `status_animation` | `set_interval` + `reactive` |
| `slash_commands` | `CommandPalette` + `/` 触发 |
| `event_dispatch` | `App` 事件处理方法 |

---

## 3. 新架构设计

### 3.1 项目结构

```
python/tui/
├── pyproject.toml              # 包配置
├── README.md                   # 使用文档
├── src/
│   └── openjax_tui/
│       ├── __init__.py         # 包入口
│       ├── __main__.py         # CLI 入口
│       ├── app.py              # 主应用，Screen 路由
│       ├── screens/            # 屏幕组件
│       │   ├── __init__.py
│       │   ├── chat_screen.py      # 主对话界面
│       │   ├── approval_screen.py  # 全屏审批
│       │   ├── config_screen.py    # 配置界面
│       │   └── help_screen.py      # 快捷键帮助
│       ├── widgets/            # 可复用组件
│       │   ├── __init__.py
│       │   ├── message_list.py     # 消息列表
│       │   ├── message_item.py     # 单条消息
│       │   ├── input_area.py       # 输入区域
│       │   ├── tool_card.py        # 工具调用卡片
│       │   └── status_bar.py       # 状态栏
│       ├── commands.py         # 命令面板命令定义
│       ├── events.py           # 自定义事件类型
│       └── styles.tcss         # 样式文件
└── tests/
    ├── conftest.py
    ├── test_app.py
    ├── test_screens/
    │   ├── test_chat_screen.py
    │   ├── test_approval_screen.py
    │   └── test_help_screen.py
    └── test_widgets/
        ├── test_message_list.py
        ├── test_input_area.py
        └── test_tool_card.py
```

### 3.2 Screen 路由设计

```
┌─────────────────────────────────────┐
│  Header (会话信息、模型选择)          │
├─────────────────────────────────────┤
│                                     │
│  ChatScreen (主对话界面)             │
│  ├─ MessageList (消息列表)          │
│  ├─ InputArea (输入区)              │
│  └─ StatusBar (状态栏)              │
│                                     │
│  ApprovalScreen (全屏审批)           │
│  ├─ 请求详情                        │
│  └─ Yes/No 按钮                     │
│                                     │
│  HelpScreen (快捷键帮助)             │
│  └─ 快捷键列表                      │
│                                     │
│  CommandPalette (命令面板)           │
│  └─ / 触发模糊搜索                  │
│                                     │
└─────────────────────────────────────┘
```

### 3.3 核心组件设计

#### 3.3.1 主应用 (OpenJaxApp)

```python
class OpenJaxApp(App):
    """OpenJax TUI 主应用"""
    
    CSS_PATH = "styles.tcss"
    COMMANDS = {OpenJaxCommandProvider}
    
    # Reactive 状态
    session_id: reactive[str | None] = reactive(None)
    turn_phase: reactive[str] = reactive("idle")
    pending_approvals: reactive[dict] = reactive({})
    messages: reactive[list] = reactive([])
    
    def on_mount(self) -> None:
        """应用启动时"""
        self.push_screen(ChatScreen())
    
    def action_command_palette(self) -> None:
        """打开命令面板"""
        self.push_screen(CommandPalette())
```

#### 3.3.2 聊天界面 (ChatScreen)

```python
class ChatScreen(Screen):
    """主对话界面"""
    
    def compose(self) -> ComposeResult:
        yield Header()
        yield MessageList()
        yield StatusBar()
        yield InputArea()
        yield Footer()
    
    def on_input_submitted(self, event: InputArea.Submitted) -> None:
        """处理用户输入"""
        if event.text.startswith("/"):
            self.app.action_command_palette()
        else:
            self.app.submit_turn(event.text)
```

#### 3.3.3 命令面板 (CommandPalette)

```python
class OpenJaxCommandProvider(CommandProvider):
    """命令面板命令提供器"""
    
    async def search(self, query: str) -> AsyncIterator[CommandSource]:
        """模糊搜索命令"""
        commands = [
            Command("approve", "批准当前请求", self.action_approve),
            Command("approve-all", "批准所有请求", self.action_approve_all),
            Command("clear", "清空对话", self.action_clear),
            Command("exit", "退出程序", self.action_exit),
            Command("help", "显示帮助", self.action_help),
            Command("pending", "查看待处理请求", self.action_pending),
        ]
        
        for cmd in commands:
            if query.lower() in cmd.name.lower():
                yield CommandSource(cmd.name, cmd.description, cmd.callback)
```

#### 3.3.4 审批界面 (ApprovalScreen)

```python
class ApprovalScreen(Screen):
    """全屏审批界面"""
    
    def compose(self) -> ComposeResult:
        yield Static("Permission Request", classes="title")
        yield Static(id="action")
        yield Static(id="reason")
        with Horizontal():
            yield Button("✓ Yes (y)", variant="success", id="approve")
            yield Button("✗ No (n)", variant="error", id="deny")
    
    def on_key(self, event: events.Key) -> None:
        """键盘快捷键"""
        if event.key == "y":
            self.app.resolve_approval(True)
        elif event.key == "n":
            self.app.resolve_approval(False)
        elif event.key == "escape":
            self.app.pop_screen()
```

### 3.4 样式系统 (styles.tcss)

```css
/* 主布局 */
Screen {
    layout: vertical;
}

/* 聊天界面 */
ChatScreen {
    layout: vertical;
}

MessageList {
    height: 1fr;
    border: solid $primary;
    overflow-y: auto;
}

/* 输入区域 */
InputArea {
    height: auto;
    max-height: 5;
    dock: bottom;
}

InputArea Input {
    width: 1fr;
}

/* 审批界面 */
ApprovalScreen {
    align: center middle;
}

ApprovalScreen .title {
    text-align: center;
    text-style: bold;
}

/* 状态栏 */
StatusBar {
    height: 1;
    background: $surface-darken-1;
    color: $text-muted;
}

/* 工具卡片 */
ToolCard {
    height: auto;
    background: $surface;
    border: solid $primary-darken-2;
}

/* 命令面板 */
CommandPalette {
    background: $surface;
    border: thick $primary;
}
```

---

## 4. 功能特性优先级

| 特性 | 优先级 | 触发方式 | 说明 |
|------|--------|----------|------|
| **Screen 路由** | P0 | 自动 | 审批时全屏切换 |
| **命令面板** | P0 | `/` | 模糊搜索命令 |
| **Markdown 渲染** | P1 | 自动 | 助手消息自动渲染 |
| **代码语法高亮** | P1 | 自动 | Markdown 代码块内 |
| **快捷键提示** | P1 | Footer | 底部显示快捷键 |
| **消息搜索** | P2 | `Ctrl+F` | 搜索历史消息 |
| **主题切换** | P3 | - | 暂不实现 |
| **配置文件** | P2 | - | `~/.config/openjax/config.toml` |

---

## 5. 执行计划（增量交付）

### 阶段 1: MVP 骨架 (Day 1-2)

**目标**: 搭建基础框架，能运行，能看到界面

| 任务 | 说明 | 预计工时 | 测试要求 |
|------|------|----------|----------|
| 1.1 项目初始化 | 创建 `python/tui/` 目录结构，配置 `pyproject.toml` | 1h | - |
| 1.2 最小 App | 实现 `OpenJaxApp`，集成 Header + RichLog + Input | 3h | 单元测试：App 能启动 |
| 1.3 基础样式 | 创建 `styles.tcss`，定义基础主题 | 1h | 人工测试：界面正常显示 |
| 1.4 Makefile 更新 | 添加 `setup-new` 和 `dev-new` 命令 | 0.5h | - |

**交付物**:
- 可运行的基础 TUI
- 输入文字能显示在 RichLog 中
- 单元测试通过

**人工测试点**:
- [ ] 运行 `make dev-new`，界面能正常显示
- [ ] 输入文字，确认出现在 RichLog 中
- [ ] 界面布局合理，无错位

---

### 阶段 2: SDK 集成 + 命令面板 (Day 3-5)

**目标**: 能发送消息，能收到响应，`/` 触发命令面板

| 任务 | 说明 | 预计工时 | 测试要求 |
|------|------|----------|----------|
| 2.1 SDK 集成 | 集成 `openjax_sdk`，实现消息发送 | 3h | 单元测试：mock SDK 调用 |
| 2.2 命令面板 | 实现 `CommandPalette`，`/` 触发 | 3h | 单元测试：命令搜索 |
| 2.3 基础命令 | 实现 /help, /exit, /clear | 2h | 单元测试：命令执行 |
| 2.4 消息显示 | 用户消息和助手响应显示 | 2h | 人工测试：端到端流程 |

**交付物**:
- 能发送消息到后端
- 能显示助手响应（非流式）
- `/` 触发命令面板，支持模糊搜索

**人工测试点**:
- [ ] 输入消息，确认能发送到后端
- [ ] 查看响应是否正确显示
- [ ] `/` 触发命令面板，模糊搜索正常
- [ ] /help, /exit 命令正常工作

---

### 阶段 3: 流式响应 (Day 6-7)

**目标**: 打字机效果流式显示

| 任务 | 说明 | 预计工时 | 测试要求 |
|------|------|----------|----------|
| 3.1 流式处理 | 实现 `assistant_delta` 事件处理 | 3h | 单元测试：模拟流式数据 |
| 3.2 打字机效果 | 实现逐字显示动画 | 2h | 人工测试：视觉效果 |
| 3.3 性能优化 | 确保长文本不卡顿 | 1h | 人工测试：1000 字以上 |

**交付物**:
- 流式响应打字机效果
- 长文本不卡顿

**人工测试点**:
- [ ] 长回复是否有打字机效果
- [ ] 快速输入是否有卡顿
- [ ] 1000 字以上文本流畅显示

---

### 阶段 4: 审批系统 (Day 8-9)

**目标**: 权限申请和响应，全屏审批界面

| 任务 | 说明 | 预计工时 | 测试要求 |
|------|------|----------|----------|
| 4.1 审批界面 | 实现 `ApprovalScreen` 全屏审批 | 3h | 单元测试：界面渲染 |
| 4.2 审批命令 | 命令面板添加 approve 命令 | 2h | 单元测试：命令执行 |
| 4.3 快捷键 | y/n 快捷键响应 | 1h | 人工测试：快捷键 |
| 4.4 状态同步 | 审批状态与界面同步 | 2h | 单元测试：状态变化 |

**交付物**:
- 全屏审批界面
- y/n 快捷键响应
- 命令面板支持 approve 相关命令

**人工测试点**:
- [ ] 触发审批，确认界面切换到全屏
- [ ] y/n 快捷键正常工作
- [ ] approve 命令在命令面板中可用
- [ ] 审批完成后正确返回聊天界面

---

### 阶段 5: 工具显示 + Markdown (Day 10-12)

**目标**: 工具调用结果美观显示，Markdown 渲染

| 任务 | 说明 | 预计工时 | 测试要求 |
|------|------|----------|----------|
| 5.1 ToolCard | 实现工具调用结果卡片 | 3h | 单元测试：卡片渲染 |
| 5.2 Markdown | 集成 Markdown 渲染 | 3h | 单元测试：Markdown 解析 |
| 5.3 代码高亮 | 代码块语法高亮 | 2h | 人工测试：视觉效果 |
| 5.4 折叠展开 | 工具详情折叠/展开 | 2h | 人工测试：交互 |

**交付物**:
- ToolCard 组件显示工具结果
- Markdown 消息自动渲染
- 代码块语法高亮

**人工测试点**:
- [ ] 工具调用结果以卡片形式显示
- [ ] Markdown 消息正确渲染
- [ ] 代码块有语法高亮
- [ ] 工具详情可以折叠/展开

---

### 阶段 6: 优化打磨 (Day 13-14)

**目标**: 快捷键系统，配置文件，完整测试

| 任务 | 说明 | 预计工时 | 测试要求 |
|------|------|----------|----------|
| 6.1 快捷键 | 实现 Ctrl+F 搜索等快捷键 | 2h | 单元测试：快捷键绑定 |
| 6.2 搜索功能 | 消息历史搜索 | 2h | 单元测试：搜索逻辑 |
| 6.3 配置文件 | 支持 `~/.config/openjax/config.toml` | 2h | 单元测试：配置读取 |
| 6.4 测试覆盖 | 补充遗漏的单元测试 | 3h | 覆盖率 > 80% |
| 6.5 文档 | 更新 README 和使用文档 | 2h | - |

**交付物**:
- 完整快捷键系统
- 配置文件支持
- 测试覆盖率 > 80%
- 更新文档

**人工测试点**:
- [ ] Ctrl+F 能搜索历史消息
- [ ] 配置文件正确读取
- [ ] 所有快捷键正常工作
- [ ] 文档清晰完整

---

## 6. 测试策略

### 6.1 每阶段测试要求

每个阶段完成后必须：

1. **单元测试** - 自动化运行，覆盖率 > 80%
2. **人工测试** - 按照该阶段的人工测试点逐一验证
3. **回归测试** - 确保之前阶段功能正常

### 6.2 测试工具

- **单元测试**: `pytest` + `textual.pilot.Pilot`
- **人工测试**: 按照各阶段的测试清单
- **回归测试**: 运行全部已有测试

### 6.3 测试示例

```python
# test_chat_screen.py
async def test_chat_screen_message_display():
    """测试消息显示"""
    app = OpenJaxApp()
    async with app.run_test() as pilot:
        # 输入消息
        await pilot.click("#input")
        await pilot.press("h", "e", "l", "l", "o")
        await pilot.press("enter")
        
        # 验证消息显示
        message_list = pilot.app.query_one(MessageList)
        assert len(message_list.messages) == 1
        assert message_list.messages[0].text == "hello"
```

---

## 7. 风险与应对

### 7.1 技术风险

| 风险 | 影响 | 应对措施 |
|------|------|----------|
| Textual 性能问题 | 高 | 早期进行长会话测试，必要时优化 RichLog 使用 |
| SDK 事件兼容 | 高 | 保持事件处理逻辑不变，仅替换 UI 层 |
| 命令面板复杂 | 中 | 使用 Textual 内置 CommandPalette |
| 终端兼容性 | 中 | 测试主流终端 (iTerm2, Terminal, VSCode) |

### 7.2 项目风险

| 风险 | 影响 | 应对措施 |
|------|------|----------|
| 进度延误 | 中 | 每阶段 2 天，超时则裁剪低优先级功能 |
| 功能遗漏 | 中 | 每阶段人工测试清单逐项验证 |
| 回滚需求 | 低 | 保留旧版本直到新版本稳定 |

---

## 8. 成功标准

### 8.1 功能对等

- [ ] 所有现有斜杠命令可用（通过命令面板）
- [ ] 流式响应正常显示
- [ ] 工具调用结果正确展示
- [ ] 权限审批流程完整
- [ ] 多行输入支持

### 8.2 性能指标

- [ ] 启动时间 < 2s
- [ ] 流式延迟 < 100ms
- [ ] 1000 轮对话不卡顿
- [ ] 内存占用 < 200MB

### 8.3 代码质量

- [ ] 测试覆盖率 > 80%
- [ ] 代码行数减少 30%+
- [ ] 无类型检查错误
- [ ] 文档完整

---

## 9. 当前阶段：阶段 1 详细计划

### 阶段 1.1: 项目初始化

**具体任务**:
1. 创建 `python/tui/` 目录结构
2. 创建 `pyproject.toml`，配置依赖（textual, rich）
3. 创建 `src/openjax_tui/__init__.py`
4. 创建 `src/openjax_tui/__main__.py`

**验收标准**:
- 目录结构正确
- `pip install -e python/tui` 能成功安装

### 阶段 1.2: 最小 App

**具体任务**:
1. 实现 `OpenJaxApp` 类，继承 `App`
2. 实现 `ChatScreen`，包含 Header + RichLog + Input + Footer
3. 实现基础事件：输入回车显示在 RichLog

**验收标准**:
- 运行 `python -m openjax_tui` 能显示界面
- 输入文字回车后显示在 RichLog

### 阶段 1.3: 基础样式

**具体任务**:
1. 创建 `styles.tcss`
2. 定义基础颜色、布局
3. 美化 Header 和 Footer

**验收标准**:
- 界面美观，布局合理
- 颜色搭配协调

### 阶段 1.4: Makefile 更新

**具体任务**:
1. 添加 `setup-new` 命令
2. 添加 `dev-new` 命令
3. 添加 `test-new` 命令

**验收标准**:
- `make setup-new` 能完成环境配置
- `make dev-new` 能启动新 TUI

---

## 10. 进度追踪

### 进度日志文档

工作进度记录在：[docs/plan/refactor/tui/progress-log.md](./tui/progress-log.md)

### 更新规则

每完成一个步骤，立即更新进度日志文档，包括：
1. **当前状态**: 代码已完成 / 测试代码已完成 / 测试通过 / 等待人工测试 / 人工测试通过
2. **更新时间**: 记录更新时间
3. **备注**: 任何问题或说明

### 人工测试说明

- **需要人工测试的步骤**: 完成后状态为"等待人工测试"，你需要按照测试清单验证，通过后我更新为"人工测试通过"才能进入下一步
- **不需要人工测试的步骤**: 直接写明"本步骤不需要人工测试，可直接进入下一步"

---

## 11. 附录

### 11.1 参考资源

- [Textual 官方文档](https://textual.textualize.io/)
- [Textual Widgets 参考](https://textual.textualize.io/widgets/)
- [Textual CSS 指南](https://textual.textualize.io/guide/CSS/)
- [Textual CommandPalette](https://textual.textualize.io/guide/command_palette/)
- [Rich 文档](https://rich.readthedocs.io/)

### 11.2 项目文档

- [工作进度日志](./tui/progress-log.md) - 实时更新执行进度

### 11.3 命令面板命令列表

| 命令 | 描述 | 快捷键 |
|------|------|--------|
| approve | 批准当前请求 | y |
| approve-all | 批准所有请求 | - |
| clear | 清空对话 | - |
| exit | 退出程序 | Ctrl+C |
| help | 显示帮助 | F1 |
| pending | 查看待处理请求 | - |

### 11.4 快捷键列表

| 快捷键 | 功能 |
|--------|------|
| `/` | 打开命令面板 |
| `Enter` | 发送消息 |
| `Shift+Enter` | 输入框换行 |
| `Ctrl+F` | 搜索历史 |
| `F1` | 显示帮助 |
| `y` | 批准（审批界面） |
| `n` | 拒绝（审批界面） |
| `Esc` | 关闭弹窗/返回 |
| `Ctrl+C` | 退出程序 |
