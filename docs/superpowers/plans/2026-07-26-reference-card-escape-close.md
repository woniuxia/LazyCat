# 置顶参考卡 Esc 关闭实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让当前获得焦点的置顶参考卡按一次 `Esc` 立即关闭，即使 Monaco 正在显示查找框或自动补全列表。

**Architecture:** 在 `ReferenceCard.vue` 的浏览器窗口捕获阶段监听 `keydown`，在 Monaco 处理事件前拦截 `Escape` 并复用现有 `closeCard()`。组件卸载时移除监听，关闭失败继续使用现有卡片内错误提示。

**Tech Stack:** Vue 3、TypeScript、Tauri 2 WebviewWindow、Vitest

---

## 文件结构

- Modify: `apps/desktop/src/components/ReferenceCard.contract.test.ts`
  通过源码契约守卫 Escape 捕获监听、事件阻断、关闭调用和卸载清理。
- Modify: `apps/desktop/src/components/ReferenceCard.vue`
  注册当前参考卡窗口的捕获阶段键盘监听，并复用现有关闭链路。

不修改 Monaco 公共 API、Rust、IPC、设置、快捷键或持久化逻辑。

### Task 1: 当前参考卡按 Esc 关闭

**Files:**
- Modify: `apps/desktop/src/components/ReferenceCard.contract.test.ts`
- Modify: `apps/desktop/src/components/ReferenceCard.vue`
- Test: `apps/desktop/src/components/ReferenceCard.contract.test.ts`

- [ ] **Step 1: 写入失败的 Escape 关闭契约测试**

在 `describe("ReferenceCard window wiring", ...)` 中加入：

```typescript
it("closes the focused card on Escape before Monaco handles it", () => {
  expect(component).toContain(
    'window.addEventListener("keydown", onWindowKeydown, true)',
  );
  expect(component).toContain(
    'window.removeEventListener("keydown", onWindowKeydown, true)',
  );
  expect(component).toContain('if (event.key !== "Escape") return;');
  expect(component).toContain("event.preventDefault();");
  expect(component).toContain("event.stopPropagation();");
  expect(component).toContain("void closeCard();");
});
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts
```

Expected: FAIL，组件尚未注册 `onWindowKeydown` 捕获监听；其余 8 个现有测试继续通过。

- [ ] **Step 3: 写入最小 Escape 关闭实现**

在 `onMounted` 开头注册监听：

```typescript
onMounted(async () => {
  window.addEventListener("keydown", onWindowKeydown, true);
  try {
```

将现有单行卸载清理改为：

```typescript
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onWindowKeydown, true);
  unlistenInit?.();
});
```

在 `closeCard()` 前加入：

```typescript
function onWindowKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  event.preventDefault();
  event.stopPropagation();
  void closeCard();
}
```

捕获阶段和 `stopPropagation()` 保证 Monaco 查找框、自动补全列表或正文编辑器不会先消费 Escape。非 Escape 按键直接返回。

- [ ] **Step 4: 运行定向测试并确认 GREEN**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts
```

Expected: 9 tests PASS，无 Vitest warning 或未处理异常。

- [ ] **Step 5: 运行类型检查和渲染层构建**

Run:

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: 两条命令 exit code 0；无 TypeScript 错误，Vite 完成构建。

- [ ] **Step 6: 检查差异并提交**

Run:

```powershell
git diff --check
git status --short
```

Expected: 仅 `ReferenceCard.vue` 和对应契约测试存在任务改动，`git diff --check` 无输出。

Commit:

```powershell
git add apps/desktop/src/components/ReferenceCard.vue apps/desktop/src/components/ReferenceCard.contract.test.ts
git commit -m "feat(reference-card): 支持 Esc 关闭当前卡片"
```

### Task 2: 完成前验证

**Files:**
- Verify: `apps/desktop/src/components/ReferenceCard.vue`
- Verify: `apps/desktop/src/components/ReferenceCard.contract.test.ts`

- [ ] **Step 1: 重跑行为测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts
```

Expected: 9 tests PASS。

- [ ] **Step 2: 核对需求边界**

Run:

```powershell
rg -n "addEventListener|removeEventListener|event.key|preventDefault|stopPropagation|closeCard" apps/desktop/src/components/ReferenceCard.vue
git diff 929fa70..HEAD --stat
git status --short
```

Expected:

- Escape 监听只存在于参考卡组件。
- 监听在捕获阶段注册和移除。
- 关闭继续复用 `closeCard()`。
- 变更不涉及 Monaco 公共 API、Rust、IPC、设置或持久化。
- 工作区无未提交任务改动。

- [ ] **Step 3: 执行完成前验证审查**

使用 `superpowers:verification-before-completion`，根据本轮实际命令输出再声明完成。
