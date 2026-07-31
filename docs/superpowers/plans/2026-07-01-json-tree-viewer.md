# JSON Tree Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable read-only JSON tree viewer and use it in the data dictionary record detail JSON area.

**Architecture:** Keep JSON tree construction, key encoding, expansion collection, summaries, and copy formatting in `apps/desktop/src/utils/jsonTreeView.ts`. `JsonTreeViewer.vue` owns only rendering, local expansion state, toolbar actions, and copy feedback. `DataDictionaryPanel.vue` passes `recordDetail.record.rawJson` plus the existing `selectedJson` copy text and no longer owns the raw JSON copy button.

**Tech Stack:** Vue 3, TypeScript, Vitest, Element Plus, `@element-plus/icons-vue`.

## Global Constraints

- Do not add a third-party JSON viewer dependency.
- Do not modify data dictionary backend, IPC, database schema, `recordDetail` requests, relation rendering, or `rawJson` semantics.
- Default expansion is `"all"`; value/default depth changes rebuild instance expansion state.
- Only non-empty objects and arrays are expandable.
- Path keys must distinguish object field `"0"` from array index `0` and avoid collisions for dots, backslashes, and brackets.
- Tree building and copy formatting must handle circular references and stop tree recursion at depth `100`.
- Data dictionary detail order remains summary tags, JSON area, relation groups.

---

## File Structure

- Create `apps/desktop/src/utils/jsonTreeView.ts`: pure JSON tree model, stable key encoding, summaries, expansion collectors, safe copy formatter.
- Create `apps/desktop/src/utils/jsonTreeView.test.ts`: Vitest coverage for tree construction, key stability, summaries, expansion depths, circular references, max depth, and safe copy formatting.
- Create `apps/desktop/src/components/common/JsonTreeViewer.vue`: reusable read-only viewer with toolbar, local expansion state, and copy feedback.
- Modify `apps/desktop/src/components/DataDictionaryPanel.vue`: replace legacy `<pre>` raw JSON block and hover copy button with `JsonTreeViewer`.
- Modify `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`: update source structure assertions for the new viewer.
- Modify `process.md`: record the reusable JSON viewer pattern after implementation because this touches more than three files.

---

### Task 1: Pure Function Red Tests

**Files:**

- Create: `apps/desktop/src/utils/jsonTreeView.test.ts`

**Interfaces:**

- Consumes: none.
- Produces expected API for Task 2:
  - `buildJsonTree(value: unknown): JsonTreeNode`
  - `formatJsonForCopy(value: unknown): string`
  - `summarizeJsonNode(node: JsonTreeNode): string`
  - `collectExpandableKeys(root: JsonTreeNode): Set<string>`
  - `collectExpandedKeysByDepth(root: JsonTreeNode, depth: number | "all"): Set<string>`
  - `isJsonTreeExpandable(node: JsonTreeNode): boolean`

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src/utils/jsonTreeView.test.ts` with tests for:

- object, array, and scalar root node construction
- object key labels using JSON string syntax and array labels using `[0]`
- stable non-colliding path keys for `"0"`, `0`, dots, backslashes, and brackets
- `object · N` and `array · N` summaries
- `"all"` expansion and depth `2` expansion semantics
- empty object and array are not expandable
- circular references render `[Circular]` without recursion
- max depth renders `[Max depth reached]`
- copy formatting handles circular references and non-JSON values

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test src/utils/jsonTreeView.test.ts`

Expected: FAIL because `./jsonTreeView` does not exist.

---

### Task 2: Pure Function Implementation

**Files:**

- Create: `apps/desktop/src/utils/jsonTreeView.ts`
- Test: `apps/desktop/src/utils/jsonTreeView.test.ts`

**Interfaces:**

- Consumes: tests from Task 1.
- Produces: stable JSON tree utilities for `JsonTreeViewer.vue`.

- [ ] **Step 1: Implement minimal pure functions**

Implement:

- `JsonTreeValueType` and `JsonTreeNode`
- path encoding with typed path segments
- safe recursive `buildJsonTree` with max depth `100`
- primitive formatting for strings, numbers, booleans, `null`, and unknown values
- summary and expandable helpers
- expansion key collectors
- safe `formatJsonForCopy`

- [ ] **Step 2: Run pure function tests**

Run: `pnpm test src/utils/jsonTreeView.test.ts`

Expected: PASS.

---

### Task 3: JsonTreeViewer Component

**Files:**

- Create: `apps/desktop/src/components/common/JsonTreeViewer.vue`

**Interfaces:**

- Consumes: `jsonTreeView.ts` exports from Task 2.
- Produces component props:
  - `value: unknown`
  - `defaultExpandDepth?: number | "all"`
  - `showToolbar?: boolean`
  - `copyText?: string`
  - `ariaLabel?: string`

- [ ] **Step 1: Create component**

Implement a toolbar with Chinese text buttons for copy, expand all, collapse all, and fold to two levels. Use existing Element Plus icon components where useful. Render objects and arrays recursively, hide expansion-only actions for non-expandable roots, and keep copy independent of the current folded state.

- [ ] **Step 2: Run typecheck after component creation**

Run: `pnpm typecheck`

Expected: no TypeScript errors from the new component.

---

### Task 4: Data Dictionary Integration

**Files:**

- Modify: `apps/desktop/src/components/DataDictionaryPanel.vue`
- Modify: `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`

**Interfaces:**

- Consumes: `JsonTreeViewer.vue`.
- Produces: data dictionary detail JSON rendered through the reusable viewer.

- [ ] **Step 1: Replace legacy JSON block**

In `DataDictionaryPanel.vue`:

- import `JsonTreeViewer`
- remove `CopyDocument` if no longer used
- replace the old `dd-json-shell` copy button and `<pre class="dd-json-view">` with:

```vue
<JsonTreeViewer
  v-if="recordDetail"
  class="dd-json-view"
  :value="recordDetail.record.rawJson"
  :copy-text="selectedJson"
  default-expand-depth="all"
/>
```

- delete `copySelectedJson`
- delete `.dd-json-copy-btn` styles
- keep `.dd-json-shell` and `.dd-json-view` as the data dictionary sizing/visual shell

- [ ] **Step 2: Update source structure test**

In `DataDictionaryPanel.context-menu.test.ts`, replace the legacy raw JSON copy test with assertions for:

- `import JsonTreeViewer from "./common/JsonTreeViewer.vue"`
- `<JsonTreeViewer`
- `:value="recordDetail.record.rawJson"`
- `:copy-text="selectedJson"`
- absence of `copySelectedJson`
- relation groups still appear after the JSON area

- [ ] **Step 3: Run data dictionary source tests**

Run: `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts`

Expected: PASS.

---

### Task 5: Final Verification And Process Note

**Files:**

- Modify: `process.md`

**Interfaces:**

- Consumes: completed implementation.
- Produces: project process note and final verification evidence.

- [ ] **Step 1: Record process note**

Add a new top entry to `process.md` describing:

- reusable JSON tree viewers should keep traversal and expansion rules in pure utilities
- business panels should pass structured value and copy text only
- source structure tests need updating when hidden hover actions become explicit toolbar actions

- [ ] **Step 2: Run required verification**

Run:

```powershell
pnpm test src/utils/jsonTreeView.test.ts src/components/DataDictionaryPanel.context-menu.test.ts
pnpm test src/utils/dataDictionary.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: all commands exit 0.
