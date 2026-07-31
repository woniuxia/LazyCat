# 挂件样式优化：清新紫韵

## 概述

桌面挂件配色偏暗沉单调，优化为"清新紫韵"风格 -- 紫/青/蓝柔和渐变点缀，提升清新感和现代感。

## 约束

- 保持现有三段式布局（拖拽把手 → 待办列表 → 底部按钮栏）
- 仅改 CSS，不动模板/逻辑/事件/数据流
- 仅保留浅色壁纸主题，移除深色主题
- 不改后端

## 改动文件

| 文件                      | 改动                                      |
| ------------------------- | ----------------------------------------- |
| `WidgetCanvas.vue`        | CSS 变量体系重定义，移除 `.glass-on-dark` |
| `WidgetTodoList.vue`      | 复选框圆形化、逾期行左侧色条、行样式微调  |
| `WidgetExtensionSlot.vue` | 按钮胶囊化、固定按钮各自着色              |

## 配色变量对照

| 变量                 | 当前                    | 新值                              |
| -------------------- | ----------------------- | --------------------------------- |
| `--wc-text`          | `#1a1a1a`               | `#1e293b` (slate-800)             |
| `--wc-text-muted`    | `rgba(26,26,26,0.55)`   | `#94a3b8` (slate-400)             |
| `--wc-glass`         | `rgba(255,255,255,0.6)` | `rgba(255,255,255,0.75)` + 微渐变 |
| `--wc-block-bg`      | `rgba(0,0,0,0.04)`      | 按钮按色相各自着色                |
| `--wc-block-border`  | `rgba(0,0,0,0.08)`      | `rgba(0,0,0,0.05)`                |
| `--wc-row-hover`     | `rgba(0,0,0,0.04)`      | `rgba(99,102,241,0.04)`           |
| `--wc-accent`        | (无)                    | `#6366f1` (indigo-500)            |
| `--wc-accent-purple` | (无)                    | `#9333ea` (purple-600)            |
| `--wc-accent-teal`   | (无)                    | `#0d9488` (teal-600)              |

## 组件改动要点

### WidgetCanvas

- 背景渐变：`linear-gradient(135deg, #f8fafc, #f1f5f9)` 叠底半透明
- 圆角：12px
- 外阴影：`0 2px 12px rgba(0,0,0,0.04)`
- 删除 `.glass-on-dark` 类及对应 CSS 变量块

### WidgetTodoList

- 复选框：`border-radius: 50%`，hover 时浅色填充，选中时实心 + 白色对勾
- 逾期行：`border-left: 2.5px solid #ef4444` + `background: rgba(239,68,68,0.06)`
- 列表头部 "+ 新建"：渐变文字 `linear-gradient(135deg, #6366f1, #a855f7)`
- 行 hover：`rgba(99,102,241,0.04)`
- 行间距：padding 从 4px 0 增至 7px 8px

### WidgetExtensionSlot

- 按钮：`border-radius: 14px` 胶囊形
- 固定按钮着色：PM → indigo、待办 → purple、Inbox → teal
- 热门按钮：中性灰，hover 时微着色
- 分隔线透明度降低

## 验证

- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- 手动检查挂件在浅色壁纸下的显示效果
