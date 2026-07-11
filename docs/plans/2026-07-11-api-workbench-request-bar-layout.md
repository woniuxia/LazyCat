# API Workbench Request Bar Layout Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将请求配置和保存操作移入元信息行，使接口调试请求栏在中等及窄窗口保持“Method + URL + 发送”的稳定单行布局。

**Architecture:** 只调整 `ApiWorkbenchPanel.vue` 的模板分组和局部 CSS，不改动响应式状态、事件处理或业务数据。元信息行继续负责请求级元数据与管理动作，请求栏收敛为发送主路径；现有 `1180px` 断点仅堆叠元信息行，不再覆盖请求栏列定义。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Scoped CSS

---

### Task 1: 调整请求栏结构与响应式样式

**Files:**
- Modify: `apps/desktop/src/components/ApiWorkbenchPanel.vue:45-135`
- Modify: `apps/desktop/src/components/ApiWorkbenchPanel.vue:2059-2080`
- Modify: `apps/desktop/src/components/ApiWorkbenchPanel.vue:2650-2680`

**Step 1: 记录修改前的布局约束**

Run:

```powershell
rg -n "api-workbench-meta-row|api-workbench-request-bar|request-settings-button|save-request-button" apps/desktop/src/components/ApiWorkbenchPanel.vue
```

Expected: 请求配置和保存按钮位于 `.api-workbench-request-bar`，请求栏使用四段列定义，`1180px` 断点把请求栏改为单列。

**Step 2: 移动请求管理动作**

在 `.api-workbench-primary-actions` 内、环境选择器之后移动现有请求配置 Popover 和保存按钮，保持以下绑定原样：

```vue
<el-popover placement="bottom-end" :width="300" trigger="click">
  <!-- 保留现有 reference 按钮和 request-settings 内容 -->
</el-popover>
<el-button
  class="save-request-button"
  :icon="DocumentChecked"
  title="保存接口"
  aria-label="保存接口"
  @click="saveRequest"
/>
```

请求栏只保留 Method 选择器、URL 输入框、`ApiWorkbenchVariablePopover` 和发送按钮。

**Step 3: 收敛请求栏 Grid**

把请求栏列定义改为：

```css
.api-workbench-request-bar {
  display: grid;
  grid-template-columns: 104px minmax(0, 1fr) auto;
}
```

删除 `1380px` 断点内对请求栏的冗余列定义；在 `1180px` 断点中只保留 `.api-workbench-meta-row { grid-template-columns: 1fr; }`，不再把 `.api-workbench-request-bar` 改为单列。

**Step 4: 检查结构和 CSS 差异**

Run:

```powershell
git diff --check
git diff -- apps/desktop/src/components/ApiWorkbenchPanel.vue
```

Expected: 只有模板节点移动、请求栏三列定义和断点清理；没有业务逻辑或文案变化。

**Step 5: 执行类型检查**

Run:

```powershell
pnpm typecheck
```

Expected: PASS，无 TypeScript/Vue 类型错误。

**Step 6: 执行渲染层构建**

Run:

```powershell
pnpm --filter @lazycat/desktop build:web
```

Expected: PASS，Vite 构建成功。

**Step 7: 进行布局冒烟检查**

在 `1181px`、`1180px`、`821px`、`820px`、`819px` 和 `375px` 宽度检查：

- 请求栏保持 Method、URL、发送同一行，无横向滚动。
- 元信息行在桌面为双列，在 `1180px` 以下为单列，环境/配置/保存操作组不溢出。
- 配置 Popover、保存、发送、变量补全、`Ctrl+S` 和 `Ctrl+Enter` 可用。
- Tab 顺序为接口名称、环境、请求配置、保存、Method、URL、发送。

**Step 8: 提交实现**

```powershell
git add apps/desktop/src/components/ApiWorkbenchPanel.vue
git commit -m "fix(api-workbench): 优化请求栏单行布局"
```
