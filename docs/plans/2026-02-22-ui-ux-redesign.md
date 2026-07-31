# LazyCat UI/UX 现代化重设计

> 日期: 2026-02-22
> 方案: B -- 主题层重写
> 范围: 色彩体系 + 毛玻璃/微光效果 + 面板切换动画 + 侧边栏可拖拽 + 首页卡片重设计

---

## 1. 设计方向

**美学定位**: Arctic Depth -- 深海极地风格。以深邃的暗色为底，青蓝色光芒作为视觉引导，毛玻璃层叠营造空间纵深感。

**关键词**: 清冷、专业、通透、有纵深

**参考风格**: Vercel Dashboard / Linear App / Raycast

---

## 2. 色彩体系重设计

### 2.1 暗色主题 (默认)

```css
:root {
  /* 核心色板 -- 从纯黑到深蓝灰的 5 级梯度 */
  --lc-bg: #0a0e14;
  --lc-surface-0: #0f1319;
  --lc-surface-1: #151b26;
  --lc-surface-2: #1a2232;
  --lc-surface-3: #222c3d;

  /* 边框 -- 微妙的蓝调 */
  --lc-border: rgba(56, 189, 248, 0.06);
  --lc-border-subtle: rgba(56, 189, 248, 0.03);
  --lc-border-hover: rgba(56, 189, 248, 0.15);
  --lc-border-active: rgba(56, 189, 248, 0.35);

  /* 文字 -- 冷白色阶 */
  --lc-text: #e2e8f0;
  --lc-text-secondary: #8494a7;
  --lc-text-muted: #4a5567;

  /* 强调色 -- 青蓝 */
  --lc-accent: #38bdf8;
  --lc-accent-light: #67d4fc;
  --lc-accent-dim: rgba(56, 189, 248, 0.12);
  --lc-accent-glow: rgba(56, 189, 248, 0.08);

  /* 语义色 (保持不变) */
  --lc-success: #34d399;
  --lc-danger: #f87171;
  --lc-info: #60a5fa;
  --lc-warning: #fbbf24;

  /* 阴影 -- 加入青蓝辉光 */
  --lc-shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.4);
  --lc-shadow-md: 0 4px 16px rgba(0, 0, 0, 0.4);
  --lc-shadow-lg: 0 8px 32px rgba(0, 0, 0, 0.5);
  --lc-shadow-glow: 0 0 30px rgba(56, 189, 248, 0.08);
}
```

### 2.2 浅色主题

```css
html[data-theme="light"] {
  --lc-bg: #f0f4f8;
  --lc-surface-0: #ffffff;
  --lc-surface-1: #f6f8fb;
  --lc-surface-2: #ecf0f6;
  --lc-surface-3: #e0e5ee;

  --lc-border: rgba(56, 130, 220, 0.1);
  --lc-border-hover: rgba(56, 130, 220, 0.2);
  --lc-border-active: rgba(56, 189, 248, 0.4);

  --lc-text: #0f172a;
  --lc-text-secondary: #475569;
  --lc-text-muted: #94a3b8;

  --lc-accent: #0ea5e9;
  --lc-accent-light: #38bdf8;
  --lc-accent-dim: rgba(14, 165, 233, 0.08);
  --lc-accent-glow: rgba(14, 165, 233, 0.05);
}
```

### 2.3 Element Plus 变量同步

所有 `--el-color-primary*` 系列与新的 `--lc-accent` 对齐：

- `--el-color-primary: #38bdf8` (暗) / `#0ea5e9` (浅)
- 从 `light-3` 到 `light-9` 按 HSL 明度梯度插值
- `--el-color-primary-dark-2: #0284c7` 用于 active 状态

### 2.4 body 背景

暗色：双径向渐变（左上青蓝 + 右下深蓝），营造微妙的空间感：

```css
body {
  background:
    radial-gradient(ellipse at 8% 0%, rgba(56, 189, 248, 0.04) 0%, transparent 50%),
    radial-gradient(ellipse at 92% 100%, rgba(96, 165, 250, 0.03) 0%, transparent 50%), var(--lc-bg);
}
```

---

## 3. 毛玻璃与纵深效果

### 3.1 侧边栏毛玻璃

```css
.nav {
  background: rgba(15, 19, 25, 0.72);
  backdrop-filter: blur(24px) saturate(150%);
  -webkit-backdrop-filter: blur(24px) saturate(150%);
  border: 1px solid var(--lc-border);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.03),
    0 0 0 1px rgba(0, 0, 0, 0.2);
}
```

浅色主题：

```css
html[data-theme="light"] .nav {
  background: rgba(255, 255, 255, 0.78);
  backdrop-filter: blur(24px) saturate(140%);
}
```

### 3.2 内容区

```css
.content {
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.02),
    var(--lc-shadow-sm);
}
```

### 3.3 标签栏

```css
.tabbar {
  background: rgba(15, 19, 25, 0.5);
  backdrop-filter: blur(12px);
  border-bottom: 1px solid var(--lc-border);
}
```

### 3.4 弹出层/上下文菜单

```css
.tabbar-context-menu {
  background: rgba(21, 27, 38, 0.85);
  backdrop-filter: blur(20px) saturate(140%);
  border: 1px solid var(--lc-border-hover);
  box-shadow: var(--lc-shadow-lg);
}
```

---

## 4. 面板切换过渡动画

### 4.1 方案

使用 Vue `<Transition>` 组件包裹动态组件 `<component :is>`，实现淡入 + 轻微上滑效果。

### 4.2 模板改动 (App.vue)

```html
<!-- 替换原有的硬切换 -->
<HomePanel v-if="activeTool === HOME_ID" ... />

<Transition name="panel-switch" mode="out-in">
  <component
    v-if="activeTool !== HOME_ID && currentComponent"
    :is="currentComponent"
    :key="activeTool"
    v-bind="currentComponentProps"
  />
</Transition>
```

HomePanel 也用 Transition 包裹：

```html
<Transition name="panel-switch" mode="out-in">
  <HomePanel v-if="activeTool === HOME_ID" ... :key="'home'" />
  <component v-else-if="currentComponent" ... />
</Transition>
```

### 4.3 CSS 动画

```css
/* panels.css 或新建 transitions.css */
.panel-switch-enter-active {
  transition:
    opacity 180ms var(--lc-ease),
    transform 180ms var(--lc-ease);
}
.panel-switch-leave-active {
  transition: opacity 120ms var(--lc-ease);
}
.panel-switch-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.panel-switch-enter-to {
  opacity: 1;
  transform: translateY(0);
}
.panel-switch-leave-from {
  opacity: 1;
}
.panel-switch-leave-to {
  opacity: 0;
}
```

时长控制在 180ms（进入）/ 120ms（离开），避免影响操作节奏。`mode="out-in"` 确保旧面板完全离开后新面板才进入，避免布局抖动。

---

## 5. 侧边栏可拖拽调宽

### 5.1 交互规范

- 在侧边栏右边缘（`.nav` 的右侧 margin 区域）放置一个 4px 宽的拖拽手柄
- 鼠标悬浮时光标变为 `col-resize`，手柄高亮
- 拖拽时实时更新 CSS Grid 的 `grid-template-columns`
- 宽度范围：`200px ~ 400px`，默认 `260px`
- 双击手柄恢复默认宽度
- 宽度持久化到 SQLite (`sidebar_width` key)

### 5.2 实现方式

在 `App.vue` 的 `.shell` grid 中，侧边栏列宽改为响应式变量：

```html
<div class="shell" :style="{ gridTemplateColumns: sidebarWidth + 'px 1fr' }">
  <SidebarNav ... />
  <div class="resize-handle" @mousedown="startResize" />
  <main class="content">...</main>
</div>
```

拖拽逻辑（composable `useResizable` 或直接内联）：

- `mousedown` → 记录起始 X 和起始宽度
- `mousemove` (document) → 计算 delta，clamp 到 [200, 400]
- `mouseup` → 停止，持久化
- `dblclick` → 重置为 260px

### 5.3 拖拽手柄样式

```css
.resize-handle {
  position: absolute;
  top: 12px; /* 与 shell padding 对齐 */
  bottom: 12px;
  width: 4px;
  cursor: col-resize;
  z-index: 10;
  border-radius: 2px;
  transition: background 200ms ease;
  /* left 值动态设置为 sidebarWidth + 12px (padding) */
}
.resize-handle:hover,
.resize-handle.is-dragging {
  background: var(--lc-accent);
  opacity: 0.5;
}
```

---

## 6. 首页卡片重设计

### 6.1 设计目标

- 更丰富的悬浮效果：光标跟随的渐变光晕
- 卡片间距和内边距调整，呼吸感更强
- 微妙的入场交错动画增强
- 收藏/热度指标的视觉权重优化

### 6.2 卡片新样式

```css
.home-tool-card {
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-1);
  padding: 20px;
  cursor: pointer;
  position: relative;
  overflow: hidden;
  transition:
    border-color 300ms var(--lc-ease),
    box-shadow 300ms var(--lc-ease),
    transform 200ms var(--lc-ease);
}

/* 顶部装饰光条 */
.home-tool-card::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: linear-gradient(90deg, transparent 0%, var(--lc-accent) 50%, transparent 100%);
  opacity: 0;
  transition: opacity 300ms var(--lc-ease);
}

/* 光标跟随渐变 -- 通过 JS 设置 --mx, --my CSS 变量 */
.home-tool-card::after {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: radial-gradient(
    300px circle at var(--mx, 50%) var(--my, 50%),
    rgba(56, 189, 248, 0.06) 0%,
    transparent 70%
  );
  opacity: 0;
  transition: opacity 300ms var(--lc-ease);
  pointer-events: none;
}

.home-tool-card:hover {
  border-color: var(--lc-border-active);
  box-shadow:
    0 0 24px rgba(56, 189, 248, 0.06),
    0 8px 24px rgba(0, 0, 0, 0.2);
  transform: translateY(-2px);
}

.home-tool-card:hover::before {
  opacity: 1;
}

.home-tool-card:hover::after {
  opacity: 1;
}
```

### 6.3 光标跟随效果 (JS)

在 `HomePanel.vue` 中为卡片容器添加 `mousemove` 事件：

```ts
function onCardMouseMove(e: MouseEvent) {
  const card = e.currentTarget as HTMLElement;
  const rect = card.getBoundingClientRect();
  card.style.setProperty("--mx", `${e.clientX - rect.left}px`);
  card.style.setProperty("--my", `${e.clientY - rect.top}px`);
}
```

### 6.4 入场动画增强

```css
.home-tool-card {
  animation: cardReveal 0.4s var(--lc-ease-out) both;
  animation-delay: calc(var(--card-index, 0) * 50ms);
}

@keyframes cardReveal {
  from {
    opacity: 0;
    transform: translateY(12px) scale(0.97);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
```

通过 `:style="{ '--card-index': index }"` 传入索引实现交错。

### 6.5 区块标题样式

```css
.home-section-header h2 {
  font-family: var(--lc-font-display);
  font-size: 15px;
  font-weight: 600;
  color: var(--lc-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

/* 标题前的装饰短线 */
.home-section-header h2::before {
  content: "";
  display: inline-block;
  width: 3px;
  height: 14px;
  background: var(--lc-accent);
  border-radius: 2px;
  margin-right: 10px;
  vertical-align: middle;
}
```

---

## 7. 字体系统

### 7.1 字体选择

保持现有字体栈，但修正 fallback 顺序并新增字重使用规范：

```css
--lc-font-display: "Outfit", "PingFang SC", "Microsoft YaHei", sans-serif;
--lc-font-body: "DM Sans", "PingFang SC", "Microsoft YaHei", sans-serif;
--lc-font-mono: "JetBrains Mono", "Cascadia Code", "Consolas", monospace;
```

### 7.2 字重规范

| 用途        | 字重 | 场景                 |
| ----------- | ---- | -------------------- |
| 标题 (h1)   | 700  | 工具标题             |
| 小标题 (h2) | 600  | 区块标题、品牌名     |
| 正文        | 400  | 描述文字、输入内容   |
| 辅助        | 400  | 次要说明、时间标记   |
| 标签        | 500  | 按钮、菜单项、标签页 |

---

## 8. 需清理的遗留问题

### 8.1 重复样式文件

`apps/desktop/src/styles.css` 是模块化 CSS 文件的合并版本（约 1500 行），与 `styles/` 目录完全重复。需确认是否有引用，如无则删除。

### 8.2 注释修正

`tokens.css` 中 `/* Accent (warm amber) */` 注释与实际的绿色/即将变更的青蓝色不符，需更新为 `/* Accent (arctic cyan) */`。

### 8.3 硬编码颜色值

多个 CSS 文件中存在硬编码的旧强调色 `rgba(10, 79, 65, ...)`:

- `sidebar.css` (.nav 背景)
- `panels.css` (.calc-row-history:focus, .calc-row-active)
- `home.css` (卡片 hover shadow, ::after 渐变)
- `reset.css` (body 背景渐变)

全部需替换为新的 `--lc-accent*` 变量引用或新的 `rgba(56, 189, 248, ...)` 值。

---

## 9. 实施分批计划

### 批次 1: 色彩体系与基础视觉

**涉及文件**:

- `styles/tokens.css` -- 重写所有 Design Token 和 Element Plus 变量
- `styles/theme-light.css` -- 重写浅色主题
- `styles/reset.css` -- 更新 body 背景渐变和滚动条
- `styles/element-overrides.css` -- 同步 Element Plus 组件覆盖色值

**验证**: `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web`

### 批次 2: 毛玻璃与纵深效果

**涉及文件**:

- `styles/sidebar.css` -- 侧边栏毛玻璃背景、hover 效果
- `styles/tabbar.css` -- 标签栏毛玻璃
- `styles/layout.css` -- 内容区阴影和纵深
- `styles/home.css` -- 首页区块背景

**验证**: 视觉回归检查（暗色 + 浅色两个主题）

### 批次 3: 面板切换过渡动画

**涉及文件**:

- `App.vue` -- 模板包裹 `<Transition>`
- `styles/panels.css` (或新建 `styles/transitions.css`) -- 过渡动画 CSS
- `styles/index.css` -- 如新建文件需添加 @import

**验证**: 切换不同工具面板，确认动画流畅无布局抖动

### 批次 4: 侧边栏可拖拽调宽

**涉及文件**:

- `App.vue` -- 添加拖拽手柄 DOM + 拖拽逻辑
- `styles/layout.css` -- 拖拽手柄样式
- `styles/responsive.css` -- 确保响应式断点兼容
- `composables/useSettings.ts` -- 宽度持久化

**验证**: 拖拽、双击重置、刷新后宽度恢复

### 批次 5: 首页卡片重设计

**涉及文件**:

- `styles/home.css` -- 卡片新样式、入场动画、区块标题
- `components/HomePanel.vue` -- 光标跟随 JS 逻辑、`--card-index` 传入

**验证**: 首页视觉效果、卡片交互、入场动画、浅色/暗色主题

### 批次 6: 清理与收尾

**涉及文件**:

- 删除 `styles.css`（如确认无引用）
- 全局搜索替换所有硬编码旧色值
- 修正注释
- 运行完整构建验证

**验证**: `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web` + `pnpm test`

---

## 10. 风险与缓解

| 风险                                         | 缓解措施                                                                            |
| -------------------------------------------- | ----------------------------------------------------------------------------------- |
| Element Plus 组件样式回归                    | 每批次完成后切换暗色/浅色主题逐一检查                                               |
| 毛玻璃 `backdrop-filter` 性能                | Tauri WebView2 基于 Chromium，`backdrop-filter` 硬件加速良好；blur 值控制在 24px 内 |
| 面板切换动画影响操作速度感                   | enter 180ms / leave 120ms，总计 < 300ms，使用 `mode="out-in"`                       |
| 拖拽调宽时内容区 Monaco 编辑器 resize 不跟随 | 拖拽结束后触发 window resize 事件，Monaco 自动响应                                  |
| 光标跟随渐变性能                             | 仅在 hover 状态下更新 CSS 变量，非 hover 时不监听 mousemove                         |
