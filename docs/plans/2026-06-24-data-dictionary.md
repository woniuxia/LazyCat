# Data Dictionary Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the data dictionary tool with JSON array import, nested field metadata, per-dictionary search, and cross-dictionary global search.

**Architecture:** Store dictionaries, field metadata, and raw JSON records in SQLite. Derive searchable text per record, use SQLite FTS5 when available, and always use normalized LIKE as the correctness fallback. The Vue panel is a three-column operational UI registered as a standard LazyCat tool.

**Tech Stack:** Tauri 2, Rust, rusqlite bundled SQLite, Vue 3, TypeScript, Element Plus, Vitest.

---

### Task 1: Backend Tests And Core Logic

**Files:**
- Create: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

**Step 1:** Write Rust tests for JSON import validation, nested dot-path flattening, path escaping, search text building, LIKE escaping, and cross-dictionary search request validation.

**Step 2:** Run `cargo test data_dictionary` and verify the tests fail because the module is not registered or logic is missing.

**Step 3:** Implement the minimal pure functions and action skeleton needed to pass the tests.

**Step 4:** Run `cargo test data_dictionary` and verify the tests pass.

### Task 2: Backend Persistence And IPC

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1:** Add DB tests for create/list/get/search/update-fields/replace/delete where practical.

**Step 2:** Run `cargo test data_dictionary` and verify DB tests fail.

**Step 3:** Add SQLite schema, FTS setup, action routing, and channel mapping.

**Step 4:** Run `cargo test data_dictionary` and verify tests pass.

### Task 3: Frontend Types And Utilities

**Files:**
- Create: `apps/desktop/src/types/data-dictionary.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Create: `apps/desktop/src/utils/dataDictionary.ts`
- Create: `apps/desktop/src/utils/dataDictionary.test.ts`

**Step 1:** Write Vitest tests for visible field summaries, dictionary source labels, match labels, and JSON formatting.

**Step 2:** Run `pnpm test src/utils/dataDictionary.test.ts` and verify tests fail.

**Step 3:** Implement the utility functions and exported types.

**Step 4:** Run `pnpm test src/utils/dataDictionary.test.ts` and verify tests pass.

### Task 4: Vue Tool Panel

**Files:**
- Create: `apps/desktop/src/components/DataDictionaryPanel.vue`
- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/tool-registry.ts`

**Step 1:** Register the tool entry and async component.

**Step 2:** Build the panel with dictionary list, import dialog, field config drawer, scope toggle, search results, and JSON detail.

**Step 3:** Run `pnpm typecheck` and fix type errors.

### Task 5: Final Verification

**Files:**
- All changed files.

**Step 1:** Run `cargo test data_dictionary`.

**Step 2:** Run `pnpm test src/utils/dataDictionary.test.ts`.

**Step 3:** Run `pnpm typecheck`.

**Step 4:** Run `pnpm --filter @lazycat/desktop build:web`.

**Step 5:** Commit with `feat(data-dictionary): 添加数据字典工具`.
