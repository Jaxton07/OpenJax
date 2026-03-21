# `src/components/composer` 索引

该子模块封装聊天输入区的全部 UI 逻辑，包括按钮栏与输入框，是后续扩展（斜杠命令、模型切换、上下文用量显示）的统一入口。

## 目录职责

- 对外暴露 `Composer` 组件，供 `App.tsx` 直接使用
- 内部按视觉区域拆分为独立子组件，各自可独立修改与扩展
- CSS 完全归属本模块，不依赖全局 `styles/` 目录

## 关键文件

- `index.tsx`：对外导出入口，持有 `input` state、`submit` 逻辑、`textareaRef`，组合子组件。
- `ComposerActions.tsx`：按钮栏（新建对话），无状态，纯展示。后续扩展点：斜杠命令触发入口、其他快捷操作。
- `ComposerInput.tsx`：输入区（textarea + 发送按钮），无状态。后续扩展点：上下文用量显示、模型切换按钮。
- `composer.css`：输入区全部样式，仅通过 `index.tsx` 的 `import` 加载。

## 对外接口

```ts
interface ComposerProps {
  disabled?: boolean;
  onSend: (content: string) => Promise<void> | void;
  onNewChat: () => void;
}
```

## 后续扩展路径

| 功能 | 扩展位置 |
|------|----------|
| 斜杠命令菜单 | 新增 `SlashCommandMenu.tsx`，在 `index.tsx` 中条件渲染 |
| 移除压缩按钮 | `ComposerActions.tsx` |
| 模型切换 | 新增 `ModelSwitcher.tsx`，在 `index.tsx` 中组合 |
| 上下文用量显示 | `ComposerInput.tsx` 内扩展 |
| 悬浮布局调整 | `composer.css` |

## 上层文档

- 返回组件层索引：[src/components/README.md](../README.md)
- 返回 Web 模块总文档：[ui/web/README.md](../../../README.md)
