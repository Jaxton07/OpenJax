# cmux 快速参考

cmux 通过 Unix socket 控制 tmux，提供窗口、工作区、面板、表面等操作。

## 基础用法

```bash
cmux <path>                    # 在新工作区打开目录（需要时启动 cmux）
cmux [global-options] <command> [options]
```

## 身份引用格式

命令接受多种引用形式：
- **UUIDs**: 完整的 UUID
- **短引用**: `window:1`, `workspace:2`, `pane:3`, `surface:4`
- **索引**: 数字索引

## 环境变量

| 变量 | 说明 |
|------|------|
| `CMUX_WORKSPACE_ID` | 自动设置在 cmux 终端中，作为所有命令的默认 `--workspace` |
| `CMUX_TAB_ID` | 可选，作为 `tab-action`/`rename-tab` 的默认 `--tab` |
| `CMUX_SURFACE_ID` | 自动设置在 cmux 终端中，作为默认 `--surface` |
| `CMUX_SOCKET_PATH` | 覆盖 Unix socket 路径 |
| `CMUX_SOCKET_PASSWORD` | Socket 认证密码（优先级：`--password` > 环境变量 > Settings） |

---

## 窗口管理

| 命令 | 说明 |
|------|------|
| `list-windows` | 列出所有窗口 |
| `current-window` | 显示当前窗口 |
| `new-window` | 创建新窗口 |
| `focus-window --window <id>` | 聚焦窗口 |
| `close-window --window <id>` | 关闭窗口 |
| `move-workspace-to-window --workspace <id> --window <id>` | 移动工作区到窗口 |
| `reorder-workspace --workspace <id> (--index <n> \| --before <id> \| --after <id>)` | 重排工作区 |
| `rename-window [--workspace <id>] <title>` | 重命名窗口 |

---

## 工作区管理

| 命令 | 说明 |
|------|------|
| `list-workspaces` | 列出所有工作区 |
| `current-workspace` | 显示当前工作区 |
| `new-workspace [--cwd <path>] [--command <text>]` | 创建新工作区 |
| `select-workspace --workspace <id>` | 选择工作区 |
| `close-workspace --workspace <id>` | 关闭工作区 |
| `rename-workspace [--workspace <id>] <title>` | 重命名工作区 |
| `workspace-action --action <name> [--workspace <id>] [--title <text>]` | 执行工作区动作 |

---

## 面板与表面

### 面板 (Pane)

| 命令 | 说明 |
|------|------|
| `list-panes [--workspace <id>]` | 列出面板 |
| `list-pane-surfaces [--workspace <id>] [--pane <id>]` | 列出面板的表面 |
| `focus-pane --pane <id> [--workspace <id>]` | 聚焦面板 |
| `new-pane --type <terminal\|browser> [--direction <left\|right\|up\|down>]` | 新建面板 |
| `resize-pane --pane <id> (-L\|-R\|-U\|-D) [--amount <n>]` | 调整面板大小 |
| `swap-pane --pane <id> --target-pane <id>` | 交换面板 |
| `break-pane [--pane <id>]` | 拆分面板 |
| `join-pane --target-pane <id> [--pane <id>]` | 合并面板 |

### 表面 (Surface)

| 命令 | 说明 |
|------|------|
| `new-surface --type <terminal\|browser> [--pane <id>] [--url <url>]` | 创建表面 |
| `close-surface --surface <id>` | 关闭表面 |
| `move-surface --surface <id> [--before <id> \| --after <id> \| --index <n>]` | 移动表面 |
| `reorder-surface --surface <id> (--index <n> \| --before <id> \| --after <id>)` | 重排表面 |
| `focus-surface --surface <id>` | 聚焦表面 |
| `refresh-surfaces` | 刷新表面 |
| `surface-health [--workspace <id>]` | 检查表面健康状态 |
| `trigger-flash [--surface <id>]` | 触发闪烁 |
| `drag-surface-to-split --surface <id> <left\|right\|up\|down>` | 拖动表面到分割位置 |

---

## 标签页 (Tab)

| 命令 | 说明 |
|------|------|
| `tab-action --action <name> [--tab <id>] [--surface <id>]` | 标签页动作 |
| `rename-tab [--tab <id>] <title>` | 重命名标签页 |

---

## 发送输入

| 命令 | 说明 |
|------|------|
| `send [--workspace <id>] [--surface <id>] <text>` | 发送文本 |
| `send-key [--workspace <id>] [--surface <id>] <key>` | 发送按键 |
| `send-panel --panel <id> [--workspace <id>] <text>` | 向面板发送文本 |
| `send-key-panel --panel <id> [--workspace <id>] <key>` | 向面板发送按键 |

---

## 读取屏幕

| 命令 | 说明 |
|------|------|
| `read-screen [--surface <id>] [--scrollback] [--lines <n>]` | 读取屏幕内容 |
| `capture-pane [--surface <id>] [--scrollback] [--lines <n>]` | 捕获面板内容（tmux 兼容） |
| `clear-history [--surface <id>]` | 清除历史 |

---

## 通知

| 命令 | 说明 |
|------|------|
| `notify --title <text> [--subtitle <text>] [--body <text>]` | 发送通知 |
| `list-notifications` | 列出通知 |
| `clear-notifications` | 清除通知 |

---

## 侧边栏状态

| 命令 | 说明 |
|------|------|
| `set-status <key> <value> [--icon <name>] [--color <#hex>]` | 设置状态 |
| `clear-status <key>` | 清除状态 |
| `list-status` | 列出状态 |
| `set-progress <0.0-1.0> [--label <text>]` | 设置进度 |
| `clear-progress` | 清除进度 |
| `sidebar-state [--workspace <id>]` | 侧边栏状态 |

---

## 日志

| 命令 | 说明 |
|------|------|
| `log [--level <level>] [--source <name>] <message>` | 写入日志 |
| `clear-log` | 清除日志 |
| `list-log [--limit <n>]` | 列出日志 |

---

## 浏览器控制

```bash
cmux browser open [url]              # 创建浏览器分割
cmux browser open-split [url]        # 在分割中打开
cmux browser goto|navigate <url>     # 导航到 URL
cmux browser back|forward|reload     # 浏览器导航
cmux browser url|get-url              # 获取当前 URL
cmux browser snapshot                # 页面快照
cmux browser screenshot [--out <path>]  # 截图
cmux browser type <selector> <text>  # 输入文本
cmux browser fill <selector> [text]   # 填写表单
cmux browser click|dblclick <selector>  # 点击
cmux browser scroll [--dx <n>] [--dy <n>]  # 滚动
cmux browser select <selector> <value>  # 选择下拉框
cmux browser get <url|title|text|html|value|attr|count|box|styles>  # 获取属性
cmux browser is <visible|enabled|checked> <selector>  # 检查状态
cmux browser find <role|text|label|placeholder|alt|title|testid|first|last|nth>  # 查找元素
cmux browser dialog <accept|dismiss> [text]  # 处理对话框
cmux browser download [wait] [--path <path>]  # 下载
cmux browser cookies <get|set|clear>  # Cookie 管理
cmux browser storage <local|session> <get|set|clear>  # 存储管理
cmux browser tab <new|list|switch|close|<index>>  # 标签页管理
cmux browser console <list|clear>    # 控制台
cmux browser errors <list|clear>      # 错误列表
```

---

## Tmux 兼容命令

| 命令 | 说明 |
|------|------|
| `next-window \| previous-window \| last-window` | 窗口导航 |
| `last-pane [--workspace <id>]` | 上一个面板 |
| `find-window [--content] [--select] <query>` | 查找窗口 |
| `pipe-pane --command <shell-command>` | 管道输出 |
| `wait-for [-S\|--signal] <name>` | 等待信号 |
| `copy-mode` | 进入复制模式 |
| `bind-key \| unbind-key` | 绑定按键 |
| `set-buffer [--name <name>] <text>` | 设置缓冲区 |
| `list-buffers` | 列出缓冲区 |
| `paste-buffer [--name <name>]` | 粘贴缓冲区 |
| `respawn-pane [--command <cmd>]` | 重新生成面板 |
| `display-message [-p\|--print] <text>` | 显示消息 |
| `set-hook [--list] [--unset <event>] <event> <command>` | 设置钩子 |

---

## 其他命令

| 命令 | 说明 |
|------|------|
| `version` | 版本信息 |
| `welcome` | 欢迎信息 |
| `shortcuts` | 快捷键列表 |
| `feedback [--email <email> --body <text>]` | 反馈 |
| `themes [list\|set\|clear]` | 主题管理 |
| `claude-teams [claude-args...]` | Claude Teams |
| `ping` | 心跳检测 |
| `capabilities` | 获取能力列表 |
| `identify [--workspace <id>] [--surface <id>]` | 身份识别 |
| `tree [--all] [--workspace <id>]` | 树形结构 |
| `set-app-focus <active\|inactive\|clear>` | 应用焦点状态 |
| `simulate-app-active` | 模拟应用激活 |
| `popup` | 弹出窗口 |
| `markdown [open] <path>` | Markdown 查看器 |

---

## 常用工作流示例

### 打开新工作区
```bash
cmux ~/projects/myapp
```

### 在新窗口中打开目录
```bash
cmux /path/to/directory
```

### 聚焦特定工作区的面板
```bash
cmux focus-pane --pane pane:1 --workspace workspace:2
```

### 发送命令到终端
```bash
cmux send --surface surface:1 "ls -la\n"
```

### 创建浏览器分割
```bash
cmux new-pane --type browser --direction right
cmux browser open https://example.com
```

### 重命名当前工作区
```bash
cmux rename-workspace "My Project"
```

### 设置侧边栏进度
```bash
cmux set-progress 0.5 --label "Processing..."
```
