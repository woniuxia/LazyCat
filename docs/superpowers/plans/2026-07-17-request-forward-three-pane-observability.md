# Request Forward Three-Pane Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Replace the request-forward config/observability tabs with a dense three-pane log workspace, modal rule editing, and a persistent resizable log inspector.

**Architecture:** Keep RequestForwardPanel.vue as the async orchestration owner for rules, runtime state, polling, mutations, and settings. Move modal form presentation and log-detail presentation into focused child components, make the rule list and log list emit explicit user intents, and keep width/selection boundary logic as tested pure functions in requestForward.ts.

**Tech Stack:** Vue 3 script setup, TypeScript, Element Plus, Vitest, existing useToolInvoke and useSettings APIs, scoped CSS.

## Global Constraints

- Runtime assets must remain fully local; add no CDN or external resource.
- Keep the existing clean light visual direction and existing Element Plus theme variables.
- Do not modify Rust IPC, request-forward database tables, log contracts, polling cadence, pagination order, or retention limits.
- Use the existing user_settings API with key request-forward:inspector-width; add no migration.
- Inspector default width is 420px, minimum is 320px, and desktop maximum is 50% of its available workspace.
- Left click selects a rule, inline controls start or stop it, right click opens edit/delete, and new/edit share one dialog.
- On narrow layouts the inspector becomes an overlay drawer and must not overwrite the saved desktop width.
- Preserve current stale-response guards, continuous log refresh window, dirty-form rules, readonly running-rule behavior, and explicit errors.
- Update process.md because the implementation touches more than three files.

---

### Task 1: Inspector Width And Log Selection Helpers

**Files:**

- Modify: apps/desktop/src/utils/requestForward.ts
- Test: apps/desktop/src/utils/requestForward.test.ts

**Interfaces:**

- Produces: DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH = 420
- Produces: MIN_REQUEST_FORWARD_INSPECTOR_WIDTH = 320
- Produces: clampRequestForwardInspectorWidth(preferred: unknown, availableWidth: number): number
- Produces: retainRequestForwardSelectedLogId(selectedId: number | null, items: Array<{ id: number }>): number | null

- [ ] **Step 1: Write failing pure-function tests**

Add imports and these cases to requestForward.test.ts:

```typescript
expect(clampRequestForwardInspectorWidth(undefined, 1200)).toBe(420);
expect(clampRequestForwardInspectorWidth("oops", 1200)).toBe(420);
expect(clampRequestForwardInspectorWidth(200, 1200)).toBe(320);
expect(clampRequestForwardInspectorWidth(900, 1200)).toBe(600);
expect(clampRequestForwardInspectorWidth(480, 800)).toBe(400);

expect(retainRequestForwardSelectedLogId(7, [{ id: 9 }, { id: 7 }])).toBe(7);
expect(retainRequestForwardSelectedLogId(7, [{ id: 9 }])).toBeNull();
expect(retainRequestForwardSelectedLogId(null, [{ id: 9 }])).toBeNull();
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

    pnpm test src/utils/requestForward.test.ts

Expected: FAIL because clampRequestForwardInspectorWidth and retainRequestForwardSelectedLogId are not exported.

- [ ] **Step 3: Implement the helpers**

Add to requestForward.ts:

```typescript
export const DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH = 420;
export const MIN_REQUEST_FORWARD_INSPECTOR_WIDTH = 320;

export function clampRequestForwardInspectorWidth(
  preferred: unknown,
  availableWidth: number,
): number {
  const parsed = typeof preferred === "number" ? preferred : Number(preferred);
  const width = Number.isFinite(parsed) ? parsed : DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH;
  const safeAvailable = Number.isFinite(availableWidth)
    ? Math.max(0, availableWidth)
    : DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH * 2;
  const maximum = Math.max(MIN_REQUEST_FORWARD_INSPECTOR_WIDTH, Math.floor(safeAvailable * 0.5));
  return Math.min(maximum, Math.max(MIN_REQUEST_FORWARD_INSPECTOR_WIDTH, Math.round(width)));
}

export function retainRequestForwardSelectedLogId(
  selectedId: number | null,
  items: Array<{ id: number }>,
): number | null {
  if (selectedId == null) return null;
  return items.some((item) => item.id === selectedId) ? selectedId : null;
}
```

- [ ] **Step 4: Run the tests and verify GREEN**

Run:

    pnpm test src/utils/requestForward.test.ts

Expected: all requestForward utility tests pass.

- [ ] **Step 5: Commit**

  git add apps/desktop/src/utils/requestForward.ts apps/desktop/src/utils/requestForward.test.ts
  git commit -m "test(request-forward): 覆盖详情栏宽度边界"

---

### Task 2: Compact Rule Navigation And Context Menu

**Files:**

- Modify: apps/desktop/src/components/request-forward/RequestForwardRuleList.vue
- Test: apps/desktop/src/components/RequestForwardPanel.test.ts

**Interfaces:**

- Consumes: RequestForwardRule and RequestForwardRuntimeStatus
- Produces events: add, select(id), start(id), stop(id), edit(id), delete(id), start-all, stop-all
- Produces behavior: right-clicking a rule opens an Element Plus context menu without selecting that rule

- [ ] **Step 1: Write failing source-structure tests**

Extend RequestForwardPanel.test.ts:

```typescript
it("uses a compact rule navigation with context editing", () => {
  expect(listSource).toContain('trigger="contextmenu"');
  expect(listSource).toMatch(/edit: \[id: number\]/);
  expect(listSource).toMatch(/delete: \[id: number\]/);
  expect(listSource).toContain('command="edit"');
  expect(listSource).toContain('command="delete"');
  expect(listSource).toContain("MoreFilled");
  expect(listSource).toContain('class="rule-row"');
  expect(listSource).not.toContain('class="rule-card"');
});

it("keeps inline start and stop controls in the rule navigation", () => {
  expect(listSource).toMatch(/emit\("start", rule\.id\)/);
  expect(listSource).toMatch(/emit\("stop", rule\.id\)/);
});
```

- [ ] **Step 2: Run the component test and verify RED**

Run:

    pnpm test src/components/RequestForwardPanel.test.ts

Expected: FAIL because edit/delete events, context menu, and compact rule rows do not exist.

- [ ] **Step 3: Implement explicit context commands**

In RequestForwardRuleList.vue:

```typescript
import { Delete, Edit, MoreFilled, Plus } from "@element-plus/icons-vue";
import type { DropdownInstance } from "element-plus";

const menuRefs = new Map<number, DropdownInstance>();

function setMenuRef(ruleId: number, value: unknown) {
  if (value) menuRefs.set(ruleId, value as DropdownInstance);
  else menuRefs.delete(ruleId);
}

function handleCommand(command: "edit" | "delete", ruleId: number) {
  emit(command, ruleId);
}
```

Wrap each compact rule row in el-dropdown with trigger=contextmenu, bind command to the current rule id, provide Edit/Delete dropdown items, and use a MoreFilled icon button calling menuRefs.get(rule.id)?.handleOpen() for keyboard-accessible access. Keep the selection button separate from inline start/stop and menu buttons.

- [ ] **Step 4: Replace card styling with dense navigation rows**

Use stable row dimensions:

```css
.rule-list {
  width: 220px;
  gap: 8px;
  padding: 12px 10px;
}
.rule-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 48px;
  border-bottom: 1px solid #e2e7ec;
  background: transparent;
}
.rule-row.is-selected {
  background: #eaf2f7;
  box-shadow: inset 3px 0 0 var(--el-color-primary, #409eff);
}
```

Keep visible focus, hover, running/failed status text, search, batch actions, counts, and empty states.

- [ ] **Step 5: Run the test and verify GREEN**

Run:

    pnpm test src/components/RequestForwardPanel.test.ts

Expected: the new navigation tests pass; unrelated existing tests may still fail only where later tasks intentionally remove tabs.

- [ ] **Step 6: Commit**

  git add apps/desktop/src/components/request-forward/RequestForwardRuleList.vue apps/desktop/src/components/RequestForwardPanel.test.ts
  git commit -m "feat(request-forward): 改造紧凑规则导航"

---

### Task 3: Shared Rule Configuration Dialog

**Files:**

- Create: apps/desktop/src/components/request-forward/RequestForwardRuleDialog.vue
- Modify: apps/desktop/src/components/RequestForwardPanel.vue
- Test: apps/desktop/src/components/RequestForwardPanel.test.ts

**Interfaces:**

- RequestForwardRuleDialog props: visible, form, errors, readonly, persisted, disabled, saving, operating
- RequestForwardRuleDialog events: update:form, request-close, save, save-and-start, stop-and-edit, delete
- RequestForwardPanel state: editorMode: "create" | "edit" | null, editorRuleId: number | null, editorIntentToken: number

- [ ] **Step 1: Write failing dialog and panel tests**

Add dialog source loading to RequestForwardPanel.test.ts and assert:

```typescript
it("moves create and edit into one controlled rule dialog", () => {
  expect(source).toContain("RequestForwardRuleDialog");
  expect(source).toContain('editorMode = ref<"create" | "edit" | null>');
  expect(source).toContain("function openCreateDialog()");
  expect(source).toContain("function openEditDialog(id: number)");
  expect(source).toContain('@edit="openEditDialog"');
  expect(source).not.toContain("<el-tabs");
  expect(source).not.toContain("<el-tab-pane");
  expect(dialogSource).toContain("<el-dialog");
  expect(dialogSource).toContain("RequestForwardRuleFormEditor");
  expect(dialogSource).toContain("停止并编辑");
  expect(dialogSource).toContain("仅保存");
  expect(dialogSource).toContain("保存并启动");
});

it("does not replace the selected log context when editing another rule", () => {
  expect(source).toContain("editorRuleId");
  expect(source).toContain("currentEditorIntent");
  expect(source).toMatch(/function openEditDialog\(id: number\)[\s\S]*?editorRuleId\.value = id/);
  expect(source).not.toMatch(/function openEditDialog\(id: number\)[\s\S]*?selectedId\.value = id/);
});
```

- [ ] **Step 2: Run the component test and verify RED**

Run:

    pnpm test src/components/RequestForwardPanel.test.ts

Expected: FAIL because RequestForwardRuleDialog and editor-specific state do not exist.

- [ ] **Step 3: Build the presentational dialog**

RequestForwardRuleDialog.vue owns only layout. It renders RequestForwardRuleFormEditor in an el-dialog with width=min(760px, 92vw), disables modal click close, emits request-close from before-close/cancel, and exposes the existing readonly banner and footer actions through explicit events. It must not invoke IPC or mutate rules.

- [ ] **Step 4: Separate editor state from selected observability state**

In RequestForwardPanel.vue:

```typescript
const editorMode = ref<"create" | "edit" | null>(null);
const editorRuleId = ref<number | null>(null);
let editorIntentToken = 0;

const editorRule = computed(
  () => rules.value.find((rule) => rule.id === editorRuleId.value) ?? null,
);
const editorStatus = computed(
  () => statuses.value.find((status) => status.ruleId === editorRuleId.value) ?? null,
);
const editorReadonly = computed(() =>
  editorRule.value ? isRequestForwardRuleReadonly(editorStatus.value?.state ?? "stopped") : false,
);

function openCreateDialog() {
  if (interactionBusy.value) return;
  editorIntentToken += 1;
  editorMode.value = "create";
  editorRuleId.value = null;
  form.value = getDefaultRequestForwardForm();
  formDirty.value = false;
  fieldErrors.value = {};
}

function openEditDialog(id: number) {
  if (interactionBusy.value) return;
  const rule = rules.value.find((item) => item.id === id);
  if (!rule) return;
  editorIntentToken += 1;
  editorMode.value = "edit";
  editorRuleId.value = id;
  form.value = { ...rule };
  formDirty.value = false;
  fieldErrors.value = {};
}
```

Use a separate currentEditorIntent for save/delete/stop-and-edit. Opening or closing a dialog must not set selectedId, clear logs, stop polling, or invalidate log-query context. After a successful create, select the new rule; after edit, retain the existing selected rule.

- [ ] **Step 5: Preserve dirty-close and readonly behavior**

Add requestEditorClose that confirms “关闭后将丢失未保存的修改” only when formDirty is true. Adapt saveRule, saveAndStart, delete, and stop-and-edit to editorRuleId/editorMode. Keep errors visible and keep the dialog open after failed validation or failed save.

- [ ] **Step 6: Run tests and verify GREEN**

Run:

    pnpm test src/components/RequestForwardPanel.test.ts src/utils/requestForward.test.ts

Expected: dialog/editor tests and all mutation intent tests pass after updating obsolete tab assertions.

- [ ] **Step 7: Commit**

  git add apps/desktop/src/components/RequestForwardPanel.vue apps/desktop/src/components/request-forward/RequestForwardRuleDialog.vue apps/desktop/src/components/RequestForwardPanel.test.ts
  git commit -m "feat(request-forward): 使用弹窗维护转发规则"

---

### Task 4: Dense Log Table And Detail Inspector

**Files:**

- Modify: apps/desktop/src/components/request-forward/RequestForwardLogList.vue
- Create: apps/desktop/src/components/request-forward/RequestForwardLogInspector.vue
- Modify: apps/desktop/src/components/RequestForwardPanel.vue
- Test: apps/desktop/src/components/RequestForwardPanel.test.ts

**Interfaces:**

- RequestForwardLogList props add selectedId: number | null
- RequestForwardLogList events add select(id: number)
- RequestForwardLogInspector props: log: RequestForwardLogRow | null
- RequestForwardLogInspector event: close
- RequestForwardPanel state: selectedLogId: number | null; selectedLog derived from logItems

- [ ] **Step 1: Write failing log presentation tests**

Add inspector source loading and assert:

```typescript
it("renders selectable dense log rows and a separate inspector", () => {
  expect(logListSource).toContain("selectedId: number | null");
  expect(logListSource).toMatch(/select: \[id: number\]/);
  expect(logListSource).toContain('class="log-table"');
  expect(logListSource).toContain('class="log-table__row"');
  expect(logListSource).not.toContain('class="http-details"');
  expect(source).toContain("selectedLogId");
  expect(source).toContain("RequestForwardLogInspector");
  expect(inspectorSource).toContain("请求头");
  expect(inspectorSource).toContain("响应头");
  expect(inspectorSource).toContain("请求体预览");
  expect(inspectorSource).toContain("响应体预览");
});
```

- [ ] **Step 2: Run the test and verify RED**

Run:

    pnpm test src/components/RequestForwardPanel.test.ts

Expected: FAIL because the list is card-based and no inspector exists.

- [ ] **Step 3: Convert the list to a semantic dense table**

Use a fixed grid for desktop columns and button-like selectable rows. Preserve loading, retry, empty, error, load-more, byte formatting, timestamps, HTTP method/path, status/error outcome, and TCP/UDP summaries. Emit select(log.id) without expanding details in place.

Desktop row columns:

    60px | minmax(180px, 1.5fr) | minmax(120px, 1fr) |
    minmax(120px, 1fr) | 72px | 72px | 70px | 92px

At narrower widths hide target, upload, and download before hiding result/request/duration/time.

- [ ] **Step 4: Implement the focused inspector**

RequestForwardLogInspector.vue:

- Shows an empty prompt when log is null.
- Has a close icon button with tooltip.
- Shows result, protocol, request title, client, target, bytes, duration, and timestamp.
- For HTTP only, renders masked request/response headers and body previews with truncation labels.
- For TCP/UDP never renders payload/body sections.
- Uses scrollable monospace content with overflow wrapping and no nested cards.

- [ ] **Step 5: Wire stable log selection**

In RequestForwardPanel.vue:

```typescript
const selectedLogId = ref<number | null>(null);
const selectedLog = computed(
  () => logItems.value.find((item) => item.id === selectedLogId.value) ?? null,
);

function selectLog(id: number) {
  selectedLogId.value = id;
}
```

Clear selectedLogId on rule change, manual filter changes, and successful clear. After every initial load or background refresh assignment, call retainRequestForwardSelectedLogId against the new items so selection survives by id only while the row remains present.

- [ ] **Step 6: Run tests and verify GREEN**

Run:

    pnpm test src/components/RequestForwardPanel.test.ts src/utils/requestForward.test.ts

Expected: log table, inspector, selection retention, and existing refresh tests pass.

- [ ] **Step 7: Commit**

  git add apps/desktop/src/components/RequestForwardPanel.vue apps/desktop/src/components/request-forward/RequestForwardLogList.vue apps/desktop/src/components/request-forward/RequestForwardLogInspector.vue apps/desktop/src/components/RequestForwardPanel.test.ts
  git commit -m "feat(request-forward): 添加高密度日志检查器"

---

### Task 5: Three-Pane Layout And Persistent Resizer

**Files:**

- Modify: apps/desktop/src/components/RequestForwardPanel.vue
- Modify: apps/desktop/src/components/RequestForwardPanel.test.ts

**Interfaces:**

- Consumes: getSetting and setSetting from useSettings
- Consumes: clampRequestForwardInspectorWidth and width constants from requestForward.ts
- Persists: request-forward:inspector-width
- Produces: pointer and keyboard accessible inspector separator

- [ ] **Step 1: Write failing persistence and layout tests**

Add:

```typescript
it("uses a three-pane workspace with a persistent resizer", () => {
  expect(source).toContain('class="request-forward-workspace"');
  expect(source).toContain('class="inspector-resizer"');
  expect(source).toContain('role="separator"');
  expect(source).toContain('aria-orientation="vertical"');
  expect(source).toContain("request-forward:inspector-width");
  expect(source).toContain("getSetting");
  expect(source).toContain("setSetting");
  expect(source).toContain("@pointerdown");
  expect(source).toContain("@keydown.left");
  expect(source).toContain("@keydown.right");
  expect(source).toContain("ResizeObserver");
});

it("keeps the inspector as an overlay on narrow layouts", () => {
  expect(source).toContain("is-inspector-open");
  expect(source).toMatch(/@media \(max-width: 1100px\)/);
  expect(source).toContain("position: absolute");
});
```

- [ ] **Step 2: Run the test and verify RED**

Run:

    pnpm test src/components/RequestForwardPanel.test.ts

Expected: FAIL because the workspace is still two-column and has no resizer/settings.

- [ ] **Step 3: Build the three-pane template**

Keep RequestForwardRuleList as the first column. The second column contains current-rule toolbar, observability warning, horizontal metrics strip, filter toolbar, refresh warning, and RequestForwardLogList. The third column is RequestForwardLogInspector with a separator between middle and right.

When no rule is selected, keep the navigation visible and show the existing empty action in the middle workspace. Do not render decorative nested cards around metrics, table, or inspector.

- [ ] **Step 4: Restore and clamp the preferred width**

Use:

```typescript
const INSPECTOR_WIDTH_SETTING = "request-forward:inspector-width";
const workspaceRef = ref<HTMLElement | null>(null);
const workspaceWidth = ref(0);
const preferredInspectorWidth = ref(
  Number(getSetting(INSPECTOR_WIDTH_SETTING)) || DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH,
);
const inspectorWidth = computed(() =>
  clampRequestForwardInspectorWidth(preferredInspectorWidth.value, workspaceWidth.value),
);
```

Observe workspaceRef with ResizeObserver. The computed width clamps rendering only and never writes during window resize.

- [ ] **Step 5: Implement pointer and keyboard resizing**

On pointerdown capture startX/startWidth, add pointermove and pointerup listeners to window, update preferredInspectorWidth during movement, and call setSetting only on pointerup. Remove listeners on pointerup and onUnmounted.

The separator is focusable. ArrowLeft increases inspector width by 16px and ArrowRight decreases it by 16px because the inspector is on the right; clamp and persist after each keyboard action. Expose aria-valuenow, aria-valuemin=320, and the current calculated maximum.

- [ ] **Step 6: Add responsive overlay behavior**

At max-width 1100px:

- Remove the inspector from the grid track.
- Position it absolute on the right below the main toolbar.
- Use width min(92%, 520px), transform it off-canvas when no log is selected, and add class is-inspector-open when selectedLog is non-null.
- Hide the resizer.
- Keep the desktop preferred width untouched.

At max-width 780px retain the existing stacked rule-navigation strategy and reduce table columns without page-level horizontal overflow.

- [ ] **Step 7: Run targeted tests and typecheck**

Run:

    pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts
    pnpm typecheck

Expected: all targeted tests pass and typecheck exits 0.

- [ ] **Step 8: Commit**

  git add apps/desktop/src/components/RequestForwardPanel.vue apps/desktop/src/components/RequestForwardPanel.test.ts
  git commit -m "feat(request-forward): 完成可调三栏观测布局"

---

### Task 6: Regression Verification And Process Record

**Files:**

- Modify: process.md
- Verify: apps/desktop/src/components/RequestForwardPanel.vue
- Verify: apps/desktop/src/components/request-forward/RequestForwardRuleList.vue
- Verify: apps/desktop/src/components/request-forward/RequestForwardRuleDialog.vue
- Verify: apps/desktop/src/components/request-forward/RequestForwardLogList.vue
- Verify: apps/desktop/src/components/request-forward/RequestForwardLogInspector.vue

**Interfaces:**

- No new runtime interface
- Produces: repository experience record and final verification evidence

- [ ] **Step 1: Add the process record**

Add a new latest entry documenting:

- Config dialogs must have their own target id and intent token; right-click editing must not reuse the selected observability rule.
- Persistent splitter state separates preferred width from current clamped render width.
- Background log replacement retains detail selection only by stable id.
- Context-menu refs stay in a plain Map to avoid reactive writes during render.

- [ ] **Step 2: Run focused unit tests**

Run:

    pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts

Expected: all tests pass with zero failures.

- [ ] **Step 3: Run the full frontend test suite**

Run:

    pnpm test

Expected: all workspace unit tests pass.

- [ ] **Step 4: Run static verification**

Run:

    pnpm typecheck
    pnpm --filter @lazycat/desktop build:web

Expected: both commands exit 0. If build:web fails with spawn EPERM, retry once, then request elevation only if it repeats.

- [ ] **Step 5: Inspect the final diff**

Run:

    git diff --check
    git status --short

Confirm there is no whitespace error, no unrelated file change, no Element Plus global override change, and no backend/bridge/schema change.

- [ ] **Step 6: Commit the process record and any final test-only correction**

  git add process.md
  git commit -m "docs(process): 记录三栏日志工作台实践"
