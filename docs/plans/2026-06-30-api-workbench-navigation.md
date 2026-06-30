# 接口调试左侧导航管理 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 完善接口调试工具左侧集合和接口导航，支持多级文件夹树、右键菜单、移动、排序和删除保留接口。

**Architecture:** Rust `api_workbench` 继续作为集合、文件夹、接口和排序的单一持久化真源，新增移动和重排 action，并强化删除文件夹行为。Vue 前端拆出 sidebar、context menu 和树形纯函数，`ApiWorkbenchPanel.vue` 只保留页面编排和 IPC 动作执行。

**Tech Stack:** Tauri 2, Vue 3, TypeScript, Element Plus, Vitest, Rust, rusqlite, serde_json.

---

## Execution Notes

- Start by running `git status --short --untracked-files=all`. The workspace may already contain unrelated API workbench edits; do not revert them.
- Do not implement drag-and-drop, search, copy, batch operations, or new database tables.
- Prefer small commits only when working in a clean or dedicated worktree. If unrelated dirty files exist, skip commit steps and report that commits were skipped to protect user changes.
- Keep request sending, environment variables, history, and Markdown export behavior unchanged.

## Reference Files

| Path | Purpose |
|---|---|
| `docs/superpowers/specs/2026-06-30-api-workbench-navigation-design.md` | Approved design |
| `apps/desktop/src/components/ApiWorkbenchPanel.vue` | Current single-file API workbench UI |
| `apps/desktop/src/types/api-workbench.ts` | API workbench frontend types |
| `apps/desktop/src/utils/apiWorkbench.ts` | Existing frontend pure helpers |
| `apps/desktop/src/utils/apiWorkbench.test.ts` | Existing helper tests |
| `apps/desktop/src/bridge/tauri.ts` | `tool:api-workbench:*` channel mapping |
| `apps/desktop/src-tauri/src/tools/api_workbench.rs` | Rust persistence, actions, request execution, tests |
| `apps/desktop/src/utils/contextMenu.ts` | Shared context menu viewport clamping |
| `apps/desktop/src/components/PmContextMenu.vue` | Existing Teleport context menu pattern |

## Task 1: Frontend Tree Utility Tests

**Files:**

- Create: `apps/desktop/src/utils/apiWorkbenchTree.test.ts`
- Later modify: `apps/desktop/src/utils/apiWorkbenchTree.ts`
- Later modify: `apps/desktop/src/types/api-workbench.ts`

**Step 1: Write failing tests for tree construction and movement helpers**

Create `apps/desktop/src/utils/apiWorkbenchTree.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { ApiWorkbenchCollection } from "../types/api-workbench";
import {
  buildApiWorkbenchTree,
  buildApiWorkbenchFolderMoveTargets,
  buildApiWorkbenchRequestMoveTargets,
  getApiWorkbenchFolderAncestorIds,
  moveApiWorkbenchOrderedId,
} from "./apiWorkbenchTree";

const collection: ApiWorkbenchCollection = {
  id: 1,
  name: "Demo",
  description: "",
  activeEnvironmentId: 10,
  folders: [
    { id: 2, collectionId: 1, parentId: null, name: "Admin", sortOrder: 1 },
    { id: 1, collectionId: 1, parentId: null, name: "Users", sortOrder: 0 },
    { id: 3, collectionId: 1, parentId: 1, name: "Profile", sortOrder: 0 },
  ],
  requests: [
    { id: 11, collectionId: 1, folderId: 1, name: "List users", method: "GET", url: "/users", sortOrder: 0 },
    { id: 10, collectionId: 1, folderId: null, name: "Health", method: "GET", url: "/health", sortOrder: 0 },
    { id: 12, collectionId: 1, folderId: 3, name: "Get profile", method: "GET", url: "/profile", sortOrder: 0 },
  ],
};

describe("apiWorkbenchTree", () => {
  it("builds roots, child folders, folder requests, and unassigned requests", () => {
    const tree = buildApiWorkbenchTree(collection);

    expect(tree.unassigned.requests.map((item) => item.name)).toEqual(["Health"]);
    expect(tree.roots.map((item) => item.name)).toEqual(["Users", "Admin"]);
    expect(tree.roots[0].children.map((item) => item.name)).toEqual(["Profile"]);
    expect(tree.roots[0].requests.map((item) => item.name)).toEqual(["List users"]);
    expect(tree.roots[0].children[0].requests.map((item) => item.name)).toEqual(["Get profile"]);
  });

  it("finds folder ancestors from root to parent", () => {
    expect(getApiWorkbenchFolderAncestorIds(collection.folders, 3)).toEqual([1]);
  });

  it("filters invalid folder move targets", () => {
    const targets = buildApiWorkbenchFolderMoveTargets(collection, 1);
    expect(targets.map((item) => item.folderId)).toEqual([null, 2]);
  });

  it("returns all request move targets including unassigned", () => {
    const targets = buildApiWorkbenchRequestMoveTargets(collection);
    expect(targets.map((item) => item.folderId)).toEqual([null, 1, 3, 2]);
  });

  it("moves ids up and down while preserving complete sibling order", () => {
    expect(moveApiWorkbenchOrderedId([1, 2, 3], 2, "up")).toEqual([2, 1, 3]);
    expect(moveApiWorkbenchOrderedId([1, 2, 3], 2, "down")).toEqual([1, 3, 2]);
    expect(moveApiWorkbenchOrderedId([1, 2, 3], 1, "up")).toEqual([1, 2, 3]);
  });
});
```

**Step 2: Run tests and confirm failure**

Run: `pnpm test src/utils/apiWorkbenchTree.test.ts`

Expected: FAIL because `./apiWorkbenchTree` does not exist.

**Step 3: Commit when isolated**

If working in a clean/dedicated worktree:

```powershell
git add apps/desktop/src/utils/apiWorkbenchTree.test.ts
git commit -m "test(api-workbench): 覆盖接口树工具行为"
```

If unrelated dirty files exist, skip the commit and continue.

## Task 2: Frontend Tree Types And Utilities

**Files:**

- Create: `apps/desktop/src/utils/apiWorkbenchTree.ts`
- Modify: `apps/desktop/src/types/api-workbench.ts`
- Test: `apps/desktop/src/utils/apiWorkbenchTree.test.ts`

**Step 1: Add tree and menu types**

Append these interfaces to `apps/desktop/src/types/api-workbench.ts`:

```ts
export interface ApiWorkbenchTreeRequestNode extends ApiWorkbenchRequestSummary {}

export interface ApiWorkbenchTreeFolderNode extends ApiWorkbenchFolder {
  children: ApiWorkbenchTreeFolderNode[];
  requests: ApiWorkbenchTreeRequestNode[];
}

export interface ApiWorkbenchTree {
  collectionId: number;
  roots: ApiWorkbenchTreeFolderNode[];
  unassigned: {
    folderId: null;
    name: string;
    requests: ApiWorkbenchTreeRequestNode[];
  };
  foldersById: Map<number, ApiWorkbenchTreeFolderNode>;
  requestsById: Map<number, ApiWorkbenchTreeRequestNode>;
}

export interface ApiWorkbenchMoveTarget {
  folderId: number | null;
  label: string;
  depth: number;
}

export type ApiWorkbenchOrderDirection = "up" | "down";

export interface ApiWorkbenchMenuItem {
  key: string;
  label: string;
  danger?: boolean;
  disabled?: boolean;
  divider?: boolean;
}
```

**Step 2: Implement the tree utility module**

Create `apps/desktop/src/utils/apiWorkbenchTree.ts`:

```ts
import type {
  ApiWorkbenchCollection,
  ApiWorkbenchFolder,
  ApiWorkbenchMoveTarget,
  ApiWorkbenchOrderDirection,
  ApiWorkbenchRequestSummary,
  ApiWorkbenchTree,
  ApiWorkbenchTreeFolderNode,
} from "../types/api-workbench";

function bySortThenId<T extends { sortOrder: number; id: number }>(a: T, b: T): number {
  return a.sortOrder - b.sortOrder || a.id - b.id;
}

function indentLabel(name: string, depth: number): string {
  return `${"  ".repeat(depth)}${name}`;
}

export function buildApiWorkbenchTree(collection: ApiWorkbenchCollection): ApiWorkbenchTree {
  const foldersById = new Map<number, ApiWorkbenchTreeFolderNode>();
  const requestsById = new Map<number, ApiWorkbenchRequestSummary>();
  const roots: ApiWorkbenchTreeFolderNode[] = [];

  for (const folder of [...collection.folders].sort(bySortThenId)) {
    foldersById.set(folder.id, { ...folder, children: [], requests: [] });
  }

  for (const folder of [...foldersById.values()].sort(bySortThenId)) {
    if (folder.parentId === null) {
      roots.push(folder);
      continue;
    }
    const parent = foldersById.get(folder.parentId);
    if (parent) parent.children.push(folder);
    else roots.push(folder);
  }

  const unassigned = { folderId: null as const, name: "未分组", requests: [] as ApiWorkbenchRequestSummary[] };
  for (const request of [...collection.requests].sort(bySortThenId)) {
    requestsById.set(request.id, request);
    if (request.folderId === null) {
      unassigned.requests.push(request);
      continue;
    }
    const folder = foldersById.get(request.folderId);
    if (folder) folder.requests.push(request);
    else unassigned.requests.push(request);
  }

  return { collectionId: collection.id, roots, unassigned, foldersById, requestsById };
}

export function getApiWorkbenchFolderAncestorIds(
  folders: ApiWorkbenchFolder[],
  folderId: number | null,
): number[] {
  if (folderId === null) return [];
  const byId = new Map(folders.map((folder) => [folder.id, folder]));
  const out: number[] = [];
  let current = byId.get(folderId)?.parentId ?? null;
  const seen = new Set<number>();
  while (current !== null && !seen.has(current)) {
    seen.add(current);
    out.unshift(current);
    current = byId.get(current)?.parentId ?? null;
  }
  return out;
}

function collectFolderTargets(
  node: ApiWorkbenchTreeFolderNode,
  depth: number,
  out: ApiWorkbenchMoveTarget[],
  blocked: Set<number>,
) {
  if (!blocked.has(node.id)) {
    out.push({ folderId: node.id, label: indentLabel(node.name, depth), depth });
  }
  for (const child of node.children) collectFolderTargets(child, depth + 1, out, blocked);
}

export function buildApiWorkbenchFolderMoveTargets(
  collection: ApiWorkbenchCollection,
  movingFolderId: number,
): ApiWorkbenchMoveTarget[] {
  const tree = buildApiWorkbenchTree(collection);
  const blocked = new Set<number>([movingFolderId]);
  const moving = tree.foldersById.get(movingFolderId);
  const markDescendants = (node: ApiWorkbenchTreeFolderNode) => {
    for (const child of node.children) {
      blocked.add(child.id);
      markDescendants(child);
    }
  };
  if (moving) markDescendants(moving);

  const out: ApiWorkbenchMoveTarget[] = [{ folderId: null, label: "根级", depth: 0 }];
  for (const root of tree.roots) collectFolderTargets(root, 0, out, blocked);
  return out;
}

export function buildApiWorkbenchRequestMoveTargets(
  collection: ApiWorkbenchCollection,
): ApiWorkbenchMoveTarget[] {
  const tree = buildApiWorkbenchTree(collection);
  const out: ApiWorkbenchMoveTarget[] = [{ folderId: null, label: "未分组", depth: 0 }];
  for (const root of tree.roots) collectFolderTargets(root, 0, out, new Set());
  return out;
}

export function moveApiWorkbenchOrderedId(
  ids: number[],
  id: number,
  direction: ApiWorkbenchOrderDirection,
): number[] {
  const index = ids.indexOf(id);
  if (index < 0) return ids;
  const target = direction === "up" ? index - 1 : index + 1;
  if (target < 0 || target >= ids.length) return ids;
  const next = [...ids];
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}
```

**Step 3: Run tests**

Run: `pnpm test src/utils/apiWorkbenchTree.test.ts`

Expected: PASS.

**Step 4: Run existing helper tests**

Run: `pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchTree.test.ts`

Expected: PASS.

**Step 5: Commit when isolated**

```powershell
git add apps/desktop/src/types/api-workbench.ts apps/desktop/src/utils/apiWorkbenchTree.ts apps/desktop/src/utils/apiWorkbenchTree.test.ts
git commit -m "feat(api-workbench): 添加接口树工具"
```

Skip commit if unrelated dirty files exist.

## Task 3: Backend Folder Delete Safety

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

**Step 1: Add tests for folder delete behavior**

In `#[cfg(test)] mod tests` in `api_workbench.rs`, add:

```rust
#[test]
fn folder_delete_preserves_descendant_requests_as_unassigned() {
    let conn = test_conn();
    let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
    let collection_id = c["id"].as_i64().unwrap();
    let parent = folder_create_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "name": "Parent" }),
    )
    .expect("parent");
    let parent_id = parent["id"].as_i64().unwrap();
    let child = folder_create_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "parentId": parent_id, "name": "Child" }),
    )
    .expect("child");
    let child_id = child["id"].as_i64().unwrap();

    let saved = request_save_with_conn(
        &conn,
        &json!({
            "collectionId": collection_id,
            "folderId": child_id,
            "name": "Child request",
            "draft": {
                "method": "GET",
                "url": "/x",
                "query": [],
                "headers": [],
                "bodyType": "none",
                "body": "",
                "form": [],
                "timeoutMs": 10000
            }
        }),
    )
    .expect("request");
    let request_id = saved["id"].as_i64().unwrap();

    folder_delete_with_conn(&conn, &json!({ "id": parent_id })).expect("delete");

    let folder_id: Option<i64> = conn
        .query_row(
            "SELECT folder_id FROM api_workbench_requests WHERE id=?1",
            [request_id],
            |row| row.get(0),
        )
        .expect("request remains");
    assert_eq!(folder_id, None);
}

#[test]
fn folder_delete_reports_missing_folder() {
    let conn = test_conn();
    let err = folder_delete_with_conn(&conn, &json!({ "id": 999 })).expect_err("missing");
    assert!(err.contains("文件夹不存在"));
}
```

**Step 2: Run tests and confirm failure or regression coverage**

Run: `cargo test api_workbench -- --nocapture`

Expected: `folder_delete_reports_missing_folder` FAILS with current implementation because deleting a missing folder returns ok. The preserve test may already pass due the existing `ON DELETE SET NULL` foreign key; keep it as regression coverage.

**Step 3: Implement explicit delete safety**

Modify `folder_delete_with_conn`:

```rust
fn folder_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM api_workbench_folders WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check folder failed: {e}"))?;
    if exists == 0 {
        return Err("文件夹不存在".to_string());
    }
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("delete folder begin: {e}"))?;
    tx.execute(
        "WITH RECURSIVE descendants(id) AS (
            SELECT id FROM api_workbench_folders WHERE id=?1
            UNION ALL
            SELECT f.id FROM api_workbench_folders f
            JOIN descendants d ON f.parent_id=d.id
        )
        UPDATE api_workbench_requests
        SET folder_id=NULL, updated_at=CURRENT_TIMESTAMP
        WHERE folder_id IN (SELECT id FROM descendants)",
        [id],
    )
    .map_err(|e| format!("unassign folder requests failed: {e}"))?;
    tx.execute("DELETE FROM api_workbench_folders WHERE id=?1", [id])
        .map_err(|e| format!("delete folder failed: {e}"))?;
    tx.commit()
        .map_err(|e| format!("delete folder commit: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

If `unchecked_transaction()` is unavailable in the current rusqlite version, change only this new block to a safe local alternative that preserves atomicity; do not refactor unrelated functions.

**Step 4: Run backend tests**

Run: `cargo test api_workbench -- --nocapture`

Expected: PASS.

**Step 5: Commit when isolated**

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "fix(api-workbench): 删除文件夹时保留接口"
```

Skip commit if unrelated dirty files exist.

## Task 4: Backend Move Actions

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

**Step 1: Add failing move tests**

In `#[cfg(test)] mod tests`, add:

```rust
#[test]
fn request_move_moves_between_folder_and_unassigned() {
    let conn = test_conn();
    let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
    let collection_id = c["id"].as_i64().unwrap();
    let folder = folder_create_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "name": "Users" }),
    )
    .expect("folder");
    let folder_id = folder["id"].as_i64().unwrap();
    let saved = request_save_with_conn(
        &conn,
        &json!({
            "collectionId": collection_id,
            "folderId": null,
            "name": "Health",
            "draft": {
                "method": "GET",
                "url": "/health",
                "query": [],
                "headers": [],
                "bodyType": "none",
                "body": "",
                "form": [],
                "timeoutMs": 10000
            }
        }),
    )
    .expect("request");
    let request_id = saved["id"].as_i64().unwrap();

    request_move_with_conn(&conn, &json!({ "id": request_id, "targetFolderId": folder_id }))
        .expect("move to folder");
    let in_folder: Option<i64> = conn
        .query_row("SELECT folder_id FROM api_workbench_requests WHERE id=?1", [request_id], |row| row.get(0))
        .expect("folder id");
    assert_eq!(in_folder, Some(folder_id));

    request_move_with_conn(&conn, &json!({ "id": request_id, "targetFolderId": null }))
        .expect("move to unassigned");
    let unassigned: Option<i64> = conn
        .query_row("SELECT folder_id FROM api_workbench_requests WHERE id=?1", [request_id], |row| row.get(0))
        .expect("folder id");
    assert_eq!(unassigned, None);
}

#[test]
fn folder_move_rejects_self_and_descendant_targets() {
    let conn = test_conn();
    let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
    let collection_id = c["id"].as_i64().unwrap();
    let parent = folder_create_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "name": "Parent" }),
    )
    .expect("parent");
    let parent_id = parent["id"].as_i64().unwrap();
    let child = folder_create_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "parentId": parent_id, "name": "Child" }),
    )
    .expect("child");
    let child_id = child["id"].as_i64().unwrap();

    let err = folder_move_with_conn(&conn, &json!({ "id": parent_id, "targetParentId": parent_id }))
        .expect_err("self");
    assert!(err.contains("自己"));

    let err = folder_move_with_conn(&conn, &json!({ "id": parent_id, "targetParentId": child_id }))
        .expect_err("descendant");
    assert!(err.contains("子文件夹"));
}
```

**Step 2: Run backend tests and confirm failure**

Run: `cargo test api_workbench -- --nocapture`

Expected: FAIL because `request_move_with_conn` and `folder_move_with_conn` are not defined.

**Step 3: Implement move helpers**

Add helpers near the existing folder/request functions:

```rust
fn next_folder_sort_order(
    conn: &Connection,
    collection_id: i64,
    parent_id: Option<i64>,
) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1
         FROM api_workbench_folders
         WHERE collection_id=?1 AND parent_id IS ?2",
        params![collection_id, parent_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("query next folder order failed: {e}"))
}

fn next_request_sort_order(
    conn: &Connection,
    collection_id: i64,
    folder_id: Option<i64>,
) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1
         FROM api_workbench_requests
         WHERE collection_id=?1 AND folder_id IS ?2",
        params![collection_id, folder_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("query next request order failed: {e}"))
}

fn folder_is_descendant(conn: &Connection, folder_id: i64, possible_descendant_id: i64) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "WITH RECURSIVE descendants(id) AS (
                SELECT id FROM api_workbench_folders WHERE parent_id=?1
                UNION ALL
                SELECT f.id FROM api_workbench_folders f
                JOIN descendants d ON f.parent_id=d.id
            )
            SELECT COUNT(*) FROM descendants WHERE id=?2",
            params![folder_id, possible_descendant_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check descendants failed: {e}"))?;
    Ok(count > 0)
}
```

**Step 4: Implement `request_move_with_conn`**

```rust
fn request_move_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let target_folder_id = payload["targetFolderId"].as_i64();
    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_requests WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "接口不存在".to_string())?;
    if let Some(folder_id) = target_folder_id {
        let owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
                [folder_id],
                |row| row.get(0),
            )
            .map_err(|_| "目标文件夹不存在".to_string())?;
        if owner != collection_id {
            return Err("目标文件夹不属于当前集合".to_string());
        }
    }
    let next_order = next_request_sort_order(conn, collection_id, target_folder_id)?;
    conn.execute(
        "UPDATE api_workbench_requests
         SET folder_id=?1, sort_order=?2, updated_at=CURRENT_TIMESTAMP
         WHERE id=?3",
        params![target_folder_id, next_order, id],
    )
    .map_err(|e| format!("move request failed: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

**Step 5: Implement `folder_move_with_conn`**

```rust
fn folder_move_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let target_parent_id = payload["targetParentId"].as_i64();
    if target_parent_id == Some(id) {
        return Err("不能移动到自己".to_string());
    }
    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "文件夹不存在".to_string())?;
    if let Some(parent_id) = target_parent_id {
        let owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
                [parent_id],
                |row| row.get(0),
            )
            .map_err(|_| "目标文件夹不存在".to_string())?;
        if owner != collection_id {
            return Err("目标文件夹不属于当前集合".to_string());
        }
        if folder_is_descendant(conn, id, parent_id)? {
            return Err("不能移动到自己的子文件夹".to_string());
        }
    }
    let next_order = next_folder_sort_order(conn, collection_id, target_parent_id)?;
    conn.execute(
        "UPDATE api_workbench_folders
         SET parent_id=?1, sort_order=?2, updated_at=CURRENT_TIMESTAMP
         WHERE id=?3",
        params![target_parent_id, next_order, id],
    )
    .map_err(|e| format!("move folder failed: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

**Step 6: Add action dispatch**

In `execute`, add:

```rust
"folder_move" => folder_move_with_conn(&conn, payload),
"request_move" => request_move_with_conn(&conn, payload),
```

**Step 7: Run tests**

Run: `cargo test api_workbench -- --nocapture`

Expected: PASS.

**Step 8: Commit when isolated**

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "feat(api-workbench): 支持接口和文件夹移动"
```

Skip commit if unrelated dirty files exist.

## Task 5: Backend Reorder Actions And Bridge Channels

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: Add failing reorder tests**

Add tests to `api_workbench.rs`:

```rust
#[test]
fn folder_reorder_requires_complete_sibling_ids() {
    let conn = test_conn();
    let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
    let collection_id = c["id"].as_i64().unwrap();
    let a = folder_create_with_conn(&conn, &json!({ "collectionId": collection_id, "name": "A" })).expect("a");
    let b = folder_create_with_conn(&conn, &json!({ "collectionId": collection_id, "name": "B" })).expect("b");
    let a_id = a["id"].as_i64().unwrap();
    let b_id = b["id"].as_i64().unwrap();

    let err = folder_reorder_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "parentId": null, "orderedIds": [b_id] }),
    )
    .expect_err("incomplete");
    assert!(err.contains("不完整"));

    folder_reorder_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "parentId": null, "orderedIds": [b_id, a_id] }),
    )
    .expect("reorder");
    let names: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT name FROM api_workbench_folders WHERE collection_id=?1 AND parent_id IS NULL ORDER BY sort_order ASC")
            .unwrap();
        stmt.query_map([collection_id], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert_eq!(names, vec!["B", "A"]);
}

#[test]
fn request_reorder_rejects_duplicate_ids() {
    let conn = test_conn();
    let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
    let collection_id = c["id"].as_i64().unwrap();
    let first = request_save_with_conn(&conn, &json!({
        "collectionId": collection_id,
        "folderId": null,
        "name": "First",
        "draft": { "method": "GET", "url": "/1", "query": [], "headers": [], "bodyType": "none", "body": "", "form": [], "timeoutMs": 10000 }
    })).expect("first");
    let second = request_save_with_conn(&conn, &json!({
        "collectionId": collection_id,
        "folderId": null,
        "name": "Second",
        "draft": { "method": "GET", "url": "/2", "query": [], "headers": [], "bodyType": "none", "body": "", "form": [], "timeoutMs": 10000 }
    })).expect("second");
    let first_id = first["id"].as_i64().unwrap();
    let second_id = second["id"].as_i64().unwrap();

    let err = request_reorder_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "folderId": null, "orderedIds": [first_id, first_id] }),
    )
    .expect_err("duplicate");
    assert!(err.contains("重复"));

    request_reorder_with_conn(
        &conn,
        &json!({ "collectionId": collection_id, "folderId": null, "orderedIds": [second_id, first_id] }),
    )
    .expect("reorder");
}
```

**Step 2: Run tests and confirm failure**

Run: `cargo test api_workbench -- --nocapture`

Expected: FAIL because reorder functions are not defined.

**Step 3: Implement id parsing and validation helper**

Add near `parse_i64`:

```rust
fn parse_ordered_ids(payload: &Value) -> Result<Vec<i64>, String> {
    let arr = payload["orderedIds"]
        .as_array()
        .ok_or_else(|| "orderedIds must be an array".to_string())?;
    let mut ids = Vec::with_capacity(arr.len());
    let mut seen = HashSet::new();
    for item in arr {
        let id = item
            .as_i64()
            .ok_or_else(|| "orderedIds must contain integers".to_string())?;
        if !seen.insert(id) {
            return Err("排序列表包含重复项".to_string());
        }
        ids.push(id);
    }
    Ok(ids)
}
```

Use the existing `HashSet` import at the top of the file.

**Step 4: Implement `folder_reorder_with_conn`**

```rust
fn folder_reorder_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let parent_id = payload["parentId"].as_i64();
    let ordered_ids = parse_ordered_ids(payload)?;
    let existing: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM api_workbench_folders
                 WHERE collection_id=?1 AND parent_id IS ?2
                 ORDER BY sort_order ASC, id ASC",
            )
            .map_err(|e| format!("prepare folder reorder failed: {e}"))?;
        stmt.query_map(params![collection_id, parent_id], |row| row.get(0))
            .map_err(|e| format!("query folder reorder failed: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("collect folder reorder failed: {e}"))?
    };
    let expected: HashSet<i64> = existing.iter().copied().collect();
    let actual: HashSet<i64> = ordered_ids.iter().copied().collect();
    if expected != actual || existing.len() != ordered_ids.len() {
        return Err("排序列表不完整".to_string());
    }
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("folder reorder begin: {e}"))?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE api_workbench_folders SET sort_order=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![idx as i64, id],
        )
        .map_err(|e| format!("update folder order failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("folder reorder commit: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

**Step 5: Implement `request_reorder_with_conn`**

Use the same structure with `folderId` and `api_workbench_requests`:

```rust
fn request_reorder_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let folder_id = payload["folderId"].as_i64();
    let ordered_ids = parse_ordered_ids(payload)?;
    let existing: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM api_workbench_requests
                 WHERE collection_id=?1 AND folder_id IS ?2
                 ORDER BY sort_order ASC, id ASC",
            )
            .map_err(|e| format!("prepare request reorder failed: {e}"))?;
        stmt.query_map(params![collection_id, folder_id], |row| row.get(0))
            .map_err(|e| format!("query request reorder failed: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("collect request reorder failed: {e}"))?
    };
    let expected: HashSet<i64> = existing.iter().copied().collect();
    let actual: HashSet<i64> = ordered_ids.iter().copied().collect();
    if expected != actual || existing.len() != ordered_ids.len() {
        return Err("排序列表不完整".to_string());
    }
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("request reorder begin: {e}"))?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE api_workbench_requests SET sort_order=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![idx as i64, id],
        )
        .map_err(|e| format!("update request order failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("request reorder commit: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

**Step 6: Add action dispatch and bridge channels**

In `execute`:

```rust
"folder_reorder" => folder_reorder_with_conn(&conn, payload),
"request_reorder" => request_reorder_with_conn(&conn, payload),
```

In `apps/desktop/src/bridge/tauri.ts` near other API workbench channels:

```ts
"tool:api-workbench:folder-move": { domain: "api_workbench", action: "folder_move" },
"tool:api-workbench:request-move": { domain: "api_workbench", action: "request_move" },
"tool:api-workbench:folder-reorder": { domain: "api_workbench", action: "folder_reorder" },
"tool:api-workbench:request-reorder": { domain: "api_workbench", action: "request_reorder" },
```

**Step 7: Run tests**

Run: `cargo test api_workbench -- --nocapture`

Expected: PASS.

**Step 8: Run typecheck smoke for channel string changes**

Run: `pnpm typecheck`

Expected: PASS.

**Step 9: Commit when isolated**

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs apps/desktop/src/bridge/tauri.ts
git commit -m "feat(api-workbench): 添加导航排序接口"
```

Skip commit if unrelated dirty files exist.

## Task 6: Context Menu Component

**Files:**

- Create: `apps/desktop/src/components/ApiWorkbenchContextMenu.vue`

**Step 1: Create the component**

Use `PmContextMenu.vue` as the local pattern, but keep class names API-workbench-specific:

```vue
<template>
  <Teleport to="body">
    <Transition name="api-workbench-menu-fade">
      <div
        v-if="visible"
        ref="menuRef"
        class="api-workbench-context-menu"
        :style="{ left: pos.x + 'px', top: pos.y + 'px' }"
        role="menu"
        @click.stop
        @contextmenu.prevent.stop
      >
        <template v-for="item in items" :key="item.key">
          <div v-if="item.divider" class="api-workbench-context-menu-divider" />
          <button
            v-else
            type="button"
            class="api-workbench-context-menu-item"
            :class="{ 'is-danger': item.danger }"
            :disabled="item.disabled"
            role="menuitem"
            @click="select(item)"
          >
            {{ item.label }}
          </button>
        </template>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { ApiWorkbenchMenuItem } from "../types/api-workbench";
import { clampContextMenuPosition } from "../utils/contextMenu";

const props = defineProps<{
  visible: boolean;
  x: number;
  y: number;
  items: ApiWorkbenchMenuItem[];
}>();

const emit = defineEmits<{
  close: [];
  select: [item: ApiWorkbenchMenuItem];
}>();

const menuRef = ref<HTMLElement | null>(null);
const pos = ref({ x: props.x, y: props.y });

function reposition() {
  const menu = menuRef.value;
  if (!menu) return;
  pos.value = clampContextMenuPosition({
    anchorX: props.x,
    anchorY: props.y,
    menuWidth: menu.offsetWidth,
    menuHeight: menu.offsetHeight,
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
  });
}

function closeFromOutside(event: Event) {
  const target = event.target;
  if (target instanceof Node && menuRef.value?.contains(target)) return;
  emit("close");
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

function addListeners() {
  document.addEventListener("pointerdown", closeFromOutside);
  document.addEventListener("contextmenu", closeFromOutside);
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", closeFromOutside);
  document.addEventListener("scroll", closeFromOutside, true);
}

function removeListeners() {
  document.removeEventListener("pointerdown", closeFromOutside);
  document.removeEventListener("contextmenu", closeFromOutside);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", closeFromOutside);
  document.removeEventListener("scroll", closeFromOutside, true);
}

watch(
  () => props.visible,
  (visible) => {
    if (visible) {
      pos.value = { x: props.x, y: props.y };
      nextTick(reposition);
      addListeners();
    } else {
      removeListeners();
    }
  },
);

onBeforeUnmount(removeListeners);

function select(item: ApiWorkbenchMenuItem) {
  if (item.disabled || item.divider) return;
  emit("select", item);
}
</script>
```

Add compact global style in the same file, not scoped, because Teleport content should not depend on parent scoped selectors.

**Step 2: Run typecheck**

Run: `pnpm typecheck`

Expected: PASS.

**Step 3: Commit when isolated**

```powershell
git add apps/desktop/src/components/ApiWorkbenchContextMenu.vue
git commit -m "feat(api-workbench): 添加导航右键菜单组件"
```

Skip commit if unrelated dirty files exist.

## Task 7: Sidebar Component

**Files:**

- Create: `apps/desktop/src/components/ApiWorkbenchSidebar.vue`
- Modify: `apps/desktop/src/types/api-workbench.ts`
- Uses: `apps/desktop/src/components/ApiWorkbenchContextMenu.vue`
- Uses: `apps/desktop/src/utils/apiWorkbenchTree.ts`

**Step 1: Add target and command types**

Append to `types/api-workbench.ts`:

```ts
export type ApiWorkbenchNavTarget =
  | { type: "blank" }
  | { type: "collection"; collectionId: number }
  | { type: "folder"; collectionId: number; folderId: number }
  | { type: "request"; collectionId: number; requestId: number; folderId: number | null };

export type ApiWorkbenchNavCommand =
  | "collection:create"
  | "collection:select"
  | "collection:rename"
  | "collection:delete"
  | "collection:export"
  | "folder:create-root"
  | "folder:create-child"
  | "folder:rename"
  | "folder:delete"
  | "folder:move"
  | "folder:up"
  | "folder:down"
  | "request:open"
  | "request:rename"
  | "request:delete"
  | "request:move"
  | "request:up"
  | "request:down";
```

**Step 2: Create `ApiWorkbenchSidebar.vue`**

The component should:

- Props: `collections`, `selectedCollectionId`, `selectedRequestId`, `loading`.
- Emits:
  - `selectCollection(collectionId)`
  - `openRequest(requestId)`
  - `command(command, target)`
- Internal state: `expandedFolderKeys: Set<string>`, menu position, menu target, menu items.
- Compute `selectedCollection` and `tree` from `buildApiWorkbenchTree`.
- Render collection list and current collection tree.
- Use `@contextmenu.prevent.stop` on blank area, collection rows, folder rows, request rows.

Use explicit button elements. Do not use nested cards.

**Step 3: Implement menu item generation**

Inside sidebar:

```ts
function menuItemsForTarget(target: ApiWorkbenchNavTarget): ApiWorkbenchMenuItem[] {
  if (target.type === "blank") {
    return [
      { key: "collection:create", label: "新建集合" },
      { key: "folder:create-root", label: "新建根文件夹", disabled: !selectedCollection.value },
    ];
  }
  if (target.type === "collection") {
    return [
      { key: "collection:select", label: "选择集合" },
      { key: "folder:create-root", label: "新建文件夹" },
      { key: "collection:rename", label: "重命名" },
      { key: "collection:export", label: "导出 Markdown" },
      { key: "collection:delete", label: "删除", danger: true },
    ];
  }
  if (target.type === "folder") {
    return [
      { key: "folder:create-child", label: "新建子文件夹" },
      { key: "folder:rename", label: "重命名" },
      { key: "folder:move", label: "移动到" },
      { key: "folder:up", label: "上移" },
      { key: "folder:down", label: "下移" },
      { key: "folder:delete", label: "删除", danger: true },
    ];
  }
  return [
    { key: "request:open", label: "打开" },
    { key: "request:rename", label: "重命名" },
    { key: "request:move", label: "移动到" },
    { key: "request:up", label: "上移" },
    { key: "request:down", label: "下移" },
    { key: "request:delete", label: "删除", danger: true },
  ];
}
```

**Step 4: Handle expand after open**

Expose a method with `defineExpose`:

```ts
defineExpose({
  expandFolder(folderId: number | null) {
    const ids = getApiWorkbenchFolderAncestorIds(selectedCollection.value?.folders ?? [], folderId);
    for (const id of ids) expandedFolderKeys.value.add(`${selectedCollectionId.value}:${id}`);
    if (folderId !== null) expandedFolderKeys.value.add(`${selectedCollectionId.value}:${folderId}`);
  },
});
```

If using a `Set` in a ref, assign a new `Set` after mutation to trigger Vue updates.

**Step 5: Validate by typecheck**

Run: `pnpm typecheck`

Expected: PASS.

**Step 6: Commit when isolated**

```powershell
git add apps/desktop/src/components/ApiWorkbenchSidebar.vue apps/desktop/src/types/api-workbench.ts
git commit -m "feat(api-workbench): 添加集合接口树侧栏"
```

Skip commit if unrelated dirty files exist.

## Task 8: Panel Integration And Management Actions

**Files:**

- Modify: `apps/desktop/src/components/ApiWorkbenchPanel.vue`
- Modify: `apps/desktop/src/types/api-workbench.ts`
- Uses: `apps/desktop/src/components/ApiWorkbenchSidebar.vue`
- Uses: `apps/desktop/src/utils/apiWorkbenchTree.ts`

**Step 1: Replace inline sidebar markup**

In `ApiWorkbenchPanel.vue` template, replace the current sidebar block that contains the collection toolbar and `request-list` with:

```vue
<ApiWorkbenchSidebar
  ref="sidebarRef"
  :collections="collections"
  :selected-collection-id="selectedCollectionId"
  :selected-request-id="selectedRequestId"
  :loading="loading"
  @select-collection="selectCollection"
  @open-request="loadRequest"
  @command="handleSidebarCommand"
/>
```

Import `ApiWorkbenchSidebar` and relevant types in the script section.

**Step 2: Track save target folder**

Add:

```ts
const selectedRequestFolderId = ref<number | null>(null);
```

When loading a request:

```ts
selectedRequestFolderId.value = detail.folderId;
sidebarRef.value?.expandFolder(detail.folderId);
```

When switching collections or creating a blank draft, reset `selectedRequestFolderId` to `null`.

When saving:

```ts
folderId: selectedRequestFolderId.value,
```

This keeps existing "save current request" behavior while preserving the loaded request folder.

**Step 3: Add IPC action helpers**

Add action helpers with these exact responsibilities:

- `createFolder(parentId: number | null)`: prompt for folder name, call `tool:api-workbench:folder-create` with `{ collectionId: selectedCollectionId.value, parentId, name }`, reload, then expand the parent folder when `parentId` is not null.
- `renameCollection(collectionId: number)`: find the collection, prompt with its current name, call `tool:api-workbench:collection-update` with unchanged `description`, then reload.
- `deleteCollection(collectionId: number)`: confirm with the collection name, call `tool:api-workbench:collection-delete`, clear selected request/response if the deleted collection was active, then reload and select the first remaining collection if any.
- `renameFolder(folderId: number)`: find the folder in the selected collection, prompt with its current name, call `tool:api-workbench:folder-update`, then reload.
- `deleteFolder(folderId: number)`: confirm that contained interfaces will move to unassigned, call `tool:api-workbench:folder-delete`, clear `selectedRequestFolderId` if the open request was inside the deleted folder, then reload.
- `renameRequest(requestId: number)`: load request detail if necessary, prompt with current name, call `tool:api-workbench:request-save` with the existing draft, description, collection, and folder, then reload.
- `deleteRequest(requestId: number)`: confirm with the request name, call `tool:api-workbench:request-delete`, clear editor state if the deleted request is currently open, then reload.
- `moveFolder(folderId: number)`: build targets with `buildApiWorkbenchFolderMoveTargets`, let the user choose a target, call `tool:api-workbench:folder-move` with `{ id: folderId, targetParentId }`, then reload.
- `moveRequest(requestId: number)`: build targets with `buildApiWorkbenchRequestMoveTargets`, let the user choose a target, call `tool:api-workbench:request-move` with `{ id: requestId, targetFolderId }`, update `selectedRequestFolderId` if this request is open, then reload.
- `reorderFolder(folderId: number, direction: ApiWorkbenchOrderDirection)`: compute sibling folder ids for the folder parent, call `moveApiWorkbenchOrderedId`, then call `tool:api-workbench:folder-reorder` with `{ collectionId, parentId, orderedIds }`.
- `reorderRequest(requestId: number, direction: ApiWorkbenchOrderDirection)`: compute sibling request ids for the request folder, call `moveApiWorkbenchOrderedId`, then call `tool:api-workbench:request-reorder` with `{ collectionId, folderId, orderedIds }`.

Use `ElMessageBox.prompt` for naming and `ElMessageBox.confirm` for deletion. For movement, use a small local `ElDialog` chooser if `ElMessageBox` cannot cleanly render the target list; keep that dialog state inside `ApiWorkbenchPanel.vue` for this iteration.

**Step 4: Implement command dispatcher**

```ts
async function handleSidebarCommand(command: ApiWorkbenchNavCommand, target: ApiWorkbenchNavTarget) {
  if (command === "collection:create") return createCollection();
  if (command === "folder:create-root") return createFolder(null);
  if (target.type === "collection") {
    if (command === "collection:select") return selectCollection(target.collectionId);
    if (command === "collection:rename") return renameCollection(target.collectionId);
    if (command === "collection:delete") return deleteCollection(target.collectionId);
    if (command === "collection:export") return exportMarkdownForCollection(target.collectionId);
  }
  if (target.type === "folder") {
    if (command === "folder:create-child") return createFolder(target.folderId);
    if (command === "folder:rename") return renameFolder(target.folderId);
    if (command === "folder:delete") return deleteFolder(target.folderId);
    if (command === "folder:move") return moveFolder(target.folderId);
    if (command === "folder:up") return reorderFolder(target.folderId, "up");
    if (command === "folder:down") return reorderFolder(target.folderId, "down");
  }
  if (target.type === "request") {
    if (command === "request:open") return loadRequest(target.requestId);
    if (command === "request:rename") return renameRequest(target.requestId);
    if (command === "request:delete") return deleteRequest(target.requestId);
    if (command === "request:move") return moveRequest(target.requestId);
    if (command === "request:up") return reorderRequest(target.requestId, "up");
    if (command === "request:down") return reorderRequest(target.requestId, "down");
  }
}
```

**Step 5: Refresh from backend after every mutation**

Every create/update/delete/move/reorder action should:

1. Execute IPC.
2. `await loadAll()`.
3. Restore or update selection where appropriate.
4. Show success message.

Do not locally mutate `collections` as the final truth.

**Step 6: Remove old sidebar CSS that no longer applies**

Move sidebar-specific styles into `ApiWorkbenchSidebar.vue`. Keep only layout grid styles in `ApiWorkbenchPanel.vue`.

**Step 7: Validate**

Run: `pnpm typecheck`

Expected: PASS.

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS.

**Step 8: Commit when isolated**

```powershell
git add apps/desktop/src/components/ApiWorkbenchPanel.vue apps/desktop/src/types/api-workbench.ts
git commit -m "feat(api-workbench): 接入接口树管理动作"
```

Skip commit if unrelated dirty files exist.

## Task 9: Final Verification And Process Note

**Files:**

- Modify if needed: `process.md`

**Step 1: Run backend verification**

Run: `cargo test api_workbench -- --nocapture`

Expected: PASS. Confirm the output includes the new move/reorder/delete tests.

**Step 2: Run frontend unit tests**

Run: `pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchTree.test.ts`

Expected: PASS.

**Step 3: Run typecheck**

Run: `pnpm typecheck`

Expected: PASS.

**Step 4: Run web build**

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS.

**Step 5: Update `process.md`**

Because implementation touches more than three files, add a short entry at the top of `process.md` after `<!-- 新记录添加在此处，最新的在最上面 -->`:

```md
## 2026-06-30: 接口调试导航树管理要以后端排序为真源

**场景**: 完善接口调试左侧集合、文件夹和接口树管理，支持右键菜单、移动和排序。
**使用次数**: 0
**问题**:
1. 前端如果只本地调整树顺序，刷新后会回到数据库顺序。
2. 多级文件夹移动如果不校验后代关系，会产生循环树。
3. 删除文件夹需要保留接口，避免组织结构管理误删接口定义。
**解决**:
1. 后端新增 move/reorder action，排序提交同级完整 id 列表，事务内写入 gapless `sort_order`。
2. 文件夹移动校验同集合、不能移动到自己或后代。
3. 删除文件夹前把后代文件夹内接口统一移到未分组。
**验证**:
- `cargo test api_workbench -- --nocapture`
- `pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchTree.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
```

**Step 6: Final status**

Run: `git status --short --untracked-files=all`

Expected: only intended files are modified. Report any unrelated existing dirty files separately.

**Step 7: Final commit when isolated**

If commits were skipped earlier due dirty worktree, do not force a broad commit. If the worktree is isolated and only intended files are changed:

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs apps/desktop/src/bridge/tauri.ts apps/desktop/src/types/api-workbench.ts apps/desktop/src/utils/apiWorkbenchTree.ts apps/desktop/src/utils/apiWorkbenchTree.test.ts apps/desktop/src/components/ApiWorkbenchContextMenu.vue apps/desktop/src/components/ApiWorkbenchSidebar.vue apps/desktop/src/components/ApiWorkbenchPanel.vue process.md
git commit -m "feat(api-workbench): 完善接口导航管理"
```
