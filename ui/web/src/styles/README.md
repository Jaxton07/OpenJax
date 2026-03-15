# `src/styles` 索引

该目录存放 Web 前端样式文件，采用“按模块拆分 + 入口聚合”的方式组织。

## 目录职责

- 页面基础样式与布局样式
- 聊天区、输入区、登录页样式
- 设置弹窗样式（tokens/shell/general/provider/controls）
- 响应式样式

## 样式入口

- `app.css`：应用级样式入口，聚合基础布局与页面样式。
- `settings.css`：设置弹窗样式入口，聚合 `settings.*.css` 子模块。

## 关键文件

- `base.css`：全局基础规则。
- `layout.css`：主布局与侧栏布局。
- `messages.css`：消息区与 reasoning/tool steps 展示样式。
- `composer.css`：输入区样式。
- `login.css`：登录页样式。
- `responsive.css`：响应式覆盖规则。

### 设置页样式子模块

- `settings.tokens.css`：设置页 token（颜色、动效参数、状态变量）。
- `settings.shell.css`：弹窗外壳与左侧导航布局。
- `settings.general.css`：通用设置面板样式。
- `settings.provider.css`：Provider 列表与编辑卡片样式、动效与状态。
- `settings.controls.css`：按钮、状态提示等控件样式。

## 维护建议

- 新增设置页视觉参数优先放 `settings.tokens.css`，避免散落硬编码。
- 业务模块样式尽量只改对应文件，减少跨文件副作用。

## 上层文档

- 返回 Web 模块总文档：[ui/web/README.md](../../README.md)
