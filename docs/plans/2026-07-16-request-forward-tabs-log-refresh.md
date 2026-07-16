# Request Forward Tabs and Log Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split the request-forward workbench into configuration and observability tabs, increase content density, and make visible forwarding logs refresh automatically without breaking pagination.

**Architecture:** Keep `RequestForwardPanel.vue` as the orchestration owner and reuse its existing serial 2-second polling loop. Add small pure helpers to `requestForward.ts` for continuous log-window sizing, use Element Plus tabs with mounted panes for UI state retention, and represent a blocked background refresh as one replaceable pending intent rather than another timer or queue.

**Tech Stack:** Vue 3 Composition API, TypeScript, Element Plus, Vitest, scoped CSS.

---

### Task 1: Continuous Background Log Window

**Files:**
- Modify: `apps/desktop/src/utils/requestForward.test.ts`
- Modify: `apps/desktop/src/utils/requestForward.ts`

**Step 1: Write the failing utility tests**

Import `getRequestForwardLogProbeLimit` and `getRequestForwardLogTargetCount`, then cover:

```ts
expect(getRequestForwardLogProbeLimit(30)).toBe(60);
expect(getRequestForwardLogProbeLimit(990)).toBe(1000);

expect(getRequestForwardLogTargetCount({
  loadedCount: 60,
  previousTotal: 100,
  nextTotal: 105,
})).toBe(65);
expect(getRequestForwardLogTargetCount({
  loadedCount: 60,
  previousTotal: 100,
  nextTotal: 200,
})).toBe(160);
expect(getRequestForwardLogTargetCount({
  loadedCount: 60,
  previousTotal: 100,
  nextTotal: 20,
})).toBe(20);
expect(getRequestForwardLogTargetCount({
  loadedCount: 990,
  previousTotal: 1000,
  nextTotal: 1000,
})).toBe(990);
```

**Step 2: Run the tests and verify failure**

Run: `pnpm test src/utils/requestForward.test.ts`

Expected: FAIL because the two helpers are not exported.

**Step 3: Implement the minimal pure helpers**

Add constants and functions to `requestForward.ts`:

```ts
const REQUEST_FORWARD_LOG_PAGE_SIZE = 30;
const REQUEST_FORWARD_LOG_LIMIT = 1000;

export function getRequestForwardLogProbeLimit(loadedCount: number): number {
  return Math.min(
    REQUEST_FORWARD_LOG_LIMIT,
    Math.max(REQUEST_FORWARD_LOG_PAGE_SIZE, loadedCount + REQUEST_FORWARD_LOG_PAGE_SIZE),
  );
}

export function getRequestForwardLogTargetCount(input: {
  loadedCount: number;
  previousTotal: number;
  nextTotal: number;
}): number {
  const added = Math.max(0, input.nextTotal - input.previousTotal);
  return Math.min(
    REQUEST_FORWARD_LOG_LIMIT,
    input.nextTotal,
    Math.max(REQUEST_FORWARD_LOG_PAGE_SIZE, input.loadedCount + added),
  );
}
```

Keep inputs internal and non-negative at the call site; do not create a generic pagination abstraction.

**Step 4: Run the tests and verify success**

Run: `pnpm test src/utils/requestForward.test.ts`

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src/utils/requestForward.ts apps/desktop/src/utils/requestForward.test.ts
git commit -m "test(request-forward): 覆盖日志刷新窗口"
```

### Task 2: Tab and Polling Structure Tests

**Files:**
- Modify: `apps/desktop/src/components/RequestForwardPanel.test.ts`

**Step 1: Add failing source contract tests**

Add focused assertions for the component integration points that are meaningful as source guards:

```ts
it("splits persisted rules into mounted config and observability tabs", () => {
  expect(source).toContain("activeWorkbenchTab");
  expect(source).toContain('<el-tabs');
  expect(source).toContain('label="规则配置"');
  expect(source).toContain('label="运行观测"');
  expect(source).not.toContain('<el-tab-pane lazy');
});

it("queues one background log refresh from the existing serial poll", () => {
  expect(source).toContain("pendingLogRefresh");
  expect(source).toContain("refreshLogsInBackground");
  expect(source).toContain("flushPendingLogRefresh");
  expect(source).toMatch(/await refreshRules\(\)[\s\S]*?refreshLogsInBackground/);
  expect(source).not.toContain("setInterval");
});

it("keeps background refresh errors non-blocking", () => {
  expect(source).toContain("logRefreshError");
  expect(source).toContain("日志自动刷新失败");
});
```

Also assert `createDraft()` assigns `activeWorkbenchTab.value = "config"` and the observability pane only exists for persisted rules.

**Step 2: Run the component test and verify failure**

Run: `pnpm test src/components/RequestForwardPanel.test.ts`

Expected: FAIL because the new state, tabs, and background refresh path do not exist.

**Step 3: Leave the failing tests in place for Task 3**

Do not weaken existing request intent, mutation, polling, or clear/reset assertions.

### Task 3: Implement Tabs and Automatic Log Refresh

**Files:**
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`

**Step 1: Add tab and refresh state**

Add:

```ts
type WorkbenchTab = "config" | "observability";
type PendingLogRefresh = {
  ruleId: number;
  intentToken: number;
  keyword: string;
  mode: "all" | RequestForwardLogOutcome;
};

const activeWorkbenchTab = ref<WorkbenchTab>("config");
const logRefreshError = ref("");
let pendingLogRefresh: PendingLogRefresh | null = null;
```

Import the two Task 1 helpers. Reset invalid pending intents when selection, draft, keyword, or mode changes. `createDraft()` must force `config`; ordinary rule selection must not change the tab.

**Step 2: Add a reusable log query snapshot**

Capture `ruleId`, `selectionIntentToken`, trimmed keyword, and mode before invoking `tool:request-forward:log-list`. Keep one current-context predicate and reuse it for initial, appended, and background responses so stale results cannot write.

**Step 3: Implement background refresh without flicker**

`refreshLogsInBackground()` must:

1. Return unless the active tab is `observability` and a persisted rule is selected.
2. If debounce, log request, load-more, or observability mutation is active, replace `pendingLogRefresh` with the current intent and return.
3. Query `offset: 0` using `getRequestForwardLogProbeLimit(logItems.value.length)`.
4. Compute `targetCount` using the old `logTotal` and probe `total`.
5. Re-query from offset zero with `limit: targetCount` only when the probe does not contain the full target.
6. Replace `logItems` with a continuous prefix and update `logTotal` only if the request token and full intent are still current.
7. Keep existing items on failure and set `logRefreshError`; clear it after any successful manual or background refresh.
8. In `finally`, release the request and call `flushPendingLogRefresh()`.

`flushPendingLogRefresh()` consumes at most one current pending intent and never starts the status poll.

**Step 4: Connect the existing poll and tab watcher**

After `await refreshRules()` in `runPoll()`, request one background refresh if the observability tab is active. Do this before deciding whether to schedule the next status poll so the terminal poll can refresh or queue the final logs.

Watch `activeWorkbenchTab`; entering `observability` immediately calls `reloadCurrentObservability()`. Keep the existing serial `setTimeout` and all generation guards.

**Step 5: Restructure the template with Element Plus tabs**

Wrap the workbench content in one `el-tabs` instance:

```vue
<el-tabs
  v-model="activeWorkbenchTab"
  class="workbench-tabs"
  :class="{ 'is-draft': draft }"
>
  <el-tab-pane label="规则配置" name="config">
    <!-- readonly banner, form scroll, config footer -->
  </el-tab-pane>
  <el-tab-pane v-if="!draft" label="运行观测" name="observability">
    <!-- observability scroll, warning, stats, filters, logs -->
  </el-tab-pane>
</el-tabs>
```

Hide the single draft tab header with scoped CSS. Do not use `lazy`; persisted panes must stay mounted. Render `logRefreshError` as a warning above `RequestForwardLogList`, while existing blocking `logError` remains the list prop.

**Step 6: Run focused tests**

Run: `pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts`

Expected: PASS.

**Step 7: Commit**

```powershell
git add apps/desktop/src/components/RequestForwardPanel.vue apps/desktop/src/components/RequestForwardPanel.test.ts
git commit -m "feat(request-forward): 拆分配置与观测工作台"
```

### Task 4: Increase Workbench Density

**Files:**
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardLogList.vue`

**Step 1: Add full-height tab layout styles**

Make `.workbench-tabs`, `.el-tabs__content`, active `.el-tab-pane`, and `.workbench-pane` flex children with `min-height: 0`. Keep one scroll container inside each pane. Use the existing Element Plus tab visuals and only adjust header padding/spacing to match the workbench.

**Step 2: Tighten the panel rhythm**

Use these bounds rather than redesigning the palette:

- Workbench scroll: approximately `14px 20px 18px`.
- Form card gap: `10px`; card padding: approximately `12px 14px 2px`.
- Stats gap: `7px`; card padding: approximately `9px 10px`.
- Log list gap: `8px`; row padding: approximately `10px 12px`.
- Preserve current readable font sizes, status colors, responsive two-column stats, and single-column form breakpoints.

**Step 3: Run focused tests and typecheck**

Run:

```powershell
pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts
pnpm typecheck
```

Expected: PASS.

**Step 4: Commit**

```powershell
git add apps/desktop/src/components/RequestForwardPanel.vue apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue apps/desktop/src/components/request-forward/RequestForwardLogList.vue
git commit -m "style(request-forward): 提升工作台信息密度"
```

### Task 5: Final Validation and Process Record

**Files:**
- Modify: `process.md`

**Step 1: Run the required validation**

```powershell
pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: all commands PASS.

**Step 2: Perform a minimal visual smoke check**

Using the repository's existing preview path, verify config, observability, draft, running-readonly, empty/error log, and narrow layouts. Confirm each tab keeps its own scroll position and Element Plus keyboard navigation works. Do not automatically start `pnpm dev`; if no existing preview can render the live component, report this runtime visual check as not run rather than presenting the static brainstorming mockup as product validation.

**Step 3: Record the reusable lesson**

Add a concise `process.md` entry covering:

- task-specific tabs should own independent scroll containers;
- background refresh must preserve a continuous offset-zero window;
- a terminal poll blocked by another request needs one replaceable pending intent;
- background refresh errors must not hide usable stale data.

**Step 4: Commit**

```powershell
git add process.md
git commit -m "docs: 记录工作台日志刷新经验"
```
