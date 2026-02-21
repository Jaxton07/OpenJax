# Gemini CLI TUI 实现参考指南（面向 Python 技术栈）

## 1. 目的与范围
本文档用于参考 Gemini CLI 在 `packages/cli/src/ui` 下的 TUI 架构与关键交互，并给出一套 **Python 技术栈**的等价实现建议。

关键前提：
- Gemini CLI 当前实现基于 TypeScript + React + Ink（非 Python）。
- 你当前项目使用 Python，因此应做“架构借鉴 + 组件重映射”，而不是直接照搬代码。

## 2. Gemini TUI 在仓库中的定位
与 TUI 直接相关的核心目录：
- `packages/cli/src/ui`

关键文件（本次已核对）：
- `packages/cli/src/ui/components/AsciiArt.ts`
  - 定义 `longAsciiLogo` / `shortAsciiLogo` / `tinyAsciiLogo`
- `packages/cli/src/ui/components/Header.tsx`
  - 根据终端宽度选择不同尺寸 logo
  - 用 `ThemedGradient` 渲染渐变标题
- `packages/cli/src/ui/components/AppHeader.tsx`
  - 在头部挂载 `Header`，并组合 `Banner`、`Tips`、`UserIdentity`
- `packages/cli/src/ui/components/Banner.tsx`
  - 启动提示/警告条（不是 logo）
- `packages/cli/src/ui/hooks/useSnowfall.ts`
  - 节日主题动画（雪花 + 树）
- `packages/cli/src/ui/layouts/DefaultAppLayout.tsx`
  - 默认布局（主内容区 + 通知区 + 输入区）

## 3. 你截图中的“大 GEMINI logo”对应实现
截图中的顶部大 logo 与下列逻辑一致：
1. ASCII 源文本在 `AsciiArt.ts`
2. `Header.tsx` 中按终端宽度选择 long/short/tiny
3. `ThemedGradient` 对 logo 做渐变渲染
4. `AppHeader.tsx` 在启动区域渲染该 Header

补充：
- `Banner.tsx` 是边框提示文本，不是大 logo 本体。

## 4. 架构抽象：把 Ink 思路迁移到 Python
Gemini 侧可抽象成 4 层：
1. 渲染层：React/Ink 组件树（`Header`, `MainContent`, `Composer`...）
2. 状态层：contexts + hooks（`UIStateContext`, `useBanner`, `useSnowfall`...）
3. 终端能力层：尺寸、颜色、键盘协议、滚动、alternate buffer
4. 业务层：消息流、命令、工具执行、确认流程

Python 迁移时建议保持同样分层：
1. `ui/render/`：纯渲染组件（无业务）
2. `ui/state/`：会话状态与 UI 状态
3. `ui/terminal/`：终端能力探测、颜色深度、快捷键协议
4. `app/`：命令路由、模型流式输出、工具调用编排

## 5. Python 推荐库与映射
优先级建议（按可维护性）：
1. `Textual`：组件化强，适合复杂 TUI（推荐）
2. `prompt_toolkit`：输入体验优秀，适合 CLI + REPL 核心
3. `Rich`：展示能力强，可与上面两者结合

Gemini -> Python 的常见映射：
- Ink `Box/Text` -> Textual `Container/Static/Label` 或 Rich `Panel/Text`
- React hooks 状态 -> Python 状态对象 + 事件总线（或 Textual reactive）
- Gradient 渲染 -> Rich `Text` 分段上色
- Terminal resize hook -> SIGWINCH / Textual 尺寸事件
- Slash commands -> 命令解析器（前缀 `/`）

## 6. 大 Logo 在 Python 的实现建议
目标：实现与 Gemini 类似的“宽度自适应 + 渐变文字 + 可开关”。

最小实现步骤：
1. 准备三套 ASCII（long/short/tiny）
2. 每次渲染读取终端宽度
3. 选中可容纳的最大 logo
4. 按字符列计算渐变色并输出
5. 提供配置项 `hide_banner` / `hide_tips`

可选增强：
- 节日动画（雪花）作为独立 feature flag
- 首次若干次启动显示欢迎条，后续自动收敛

## 7. 推荐目录草案（Python）
```text
openjax/
  tui/
    app.py                  # 应用入口
    layout.py               # 顶层布局
    header.py               # logo + 版本 + tips 区
    banner.py               # 启动提示/警告条
    composer.py             # 输入框
    message_list.py         # 对话流
    state.py                # UIState / SessionState
    commands.py             # /help /theme /model ...
    terminal.py             # 宽度、颜色、键盘能力
    themes.py               # 主题与渐变策略
    ascii_art.py            # long/short/tiny logo
```

## 8. 迁移时避免的坑
1. 不要把业务逻辑写在渲染组件里（避免后续难测）
2. 不要把“logo/banner/tips”混为一个组件，需保持职责清晰
3. 不要忽略终端宽度变化事件（resize 后需重选 logo）
4. 不要先做复杂动画，优先稳定输入与消息渲染

## 9. 迭代路线（建议）
1. M1：静态 header（long/short/tiny + 渐变）
2. M2：消息列表 + 输入框 + 基础命令
3. M3：banner/tips/identity 等辅助信息
4. M4：流式响应、工具输出块、确认队列
5. M5：动画、主题切换、性能优化

## 10. 结论
可以明确参考 Gemini 的 `packages/cli/src/ui` 实现思路，尤其是 `Header + AsciiArt + AppHeader` 的组织方式；
但在 Python 项目里应按等价架构重建，而不是尝试直接复用 TypeScript/Ink 代码。
