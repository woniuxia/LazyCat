# Browser Profiles Spotlight Alias Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make browser profile alias, hidden state, Edge path, and launch-stat changes refresh the Spotlight browser-profiles cache immediately.

**Architecture:** Add a small Tauri cross-window event wrapper for browser profile changes, plus a tested cache-write guard for the `browser-profiles` Spotlight provider. Wire successful browser profile mutations to broadcast the event, and wire `SpotlightPanel.vue` to refresh only the `browser-profiles` provider with latest-wins protection against both local refresh races and `prefetchAll()` races.

**Tech Stack:** Tauri 2 event API, Vue 3 `<script setup>`, TypeScript, Vitest, existing Spotlight provider registry.

---

### Task 1: Browser Profile Change Event Wrapper

**Files:**

- Create: `apps/desktop/src/spotlight/browser-profiles-events.ts`
- Create: `apps/desktop/src/spotlight/browser-profiles-events.test.ts`

**Step 1: Write the failing tests**

Create `apps/desktop/src/spotlight/browser-profiles-events.test.ts`.

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const emit = vi.fn();
const listen = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: unknown[]) => emit(...args),
  listen: (...args: unknown[]) => listen(...args),
}));

import {
  BROWSER_PROFILES_CHANGED_EVENT,
  listenBrowserProfilesChanged,
  notifyBrowserProfilesChanged,
} from "./browser-profiles-events";

beforeEach(() => {
  emit.mockReset();
  listen.mockReset();
});

describe("browser profile change events", () => {
  it("emits the cross-window browser profile changed event", async () => {
    await notifyBrowserProfilesChanged("alias");

    expect(emit).toHaveBeenCalledWith(BROWSER_PROFILES_CHANGED_EVENT, {
      reason: "alias",
    });
  });

  it("listens and forwards event payloads", async () => {
    const unlisten = vi.fn();
    const handler = vi.fn();
    listen.mockImplementation((_event, cb) => {
      cb({ payload: { reason: "hidden" } });
      return Promise.resolve(unlisten);
    });

    const got = await listenBrowserProfilesChanged(handler);

    expect(listen).toHaveBeenCalledWith(BROWSER_PROFILES_CHANGED_EVENT, expect.any(Function));
    expect(handler).toHaveBeenCalledWith({ reason: "hidden" });
    expect(got).toBe(unlisten);
  });
});
```

Run: `pnpm test src/spotlight/browser-profiles-events.test.ts`

Expected: FAIL because `browser-profiles-events.ts` does not exist.

**Step 2: Implement the event wrapper**

Create `apps/desktop/src/spotlight/browser-profiles-events.ts`.

```ts
import type { UnlistenFn } from "@tauri-apps/api/event";

export const BROWSER_PROFILES_CHANGED_EVENT = "browser-profiles-changed";

export type BrowserProfilesChangedReason = "alias" | "hidden" | "edge-path" | "launch";

export interface BrowserProfilesChangedPayload {
  reason: BrowserProfilesChangedReason;
}

export async function notifyBrowserProfilesChanged(
  reason: BrowserProfilesChangedReason,
): Promise<void> {
  const { emit } = await import("@tauri-apps/api/event");
  await emit(BROWSER_PROFILES_CHANGED_EVENT, { reason });
}

export async function listenBrowserProfilesChanged(
  handler: (payload: BrowserProfilesChangedPayload) => void | Promise<void>,
): Promise<UnlistenFn> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<BrowserProfilesChangedPayload>(
    BROWSER_PROFILES_CHANGED_EVENT,
    (event) => void handler(event.payload),
  );
}
```

**Step 3: Verify**

Run: `pnpm test src/spotlight/browser-profiles-events.test.ts`

Expected: PASS.

**Step 4: Commit**

```bash
git add apps/desktop/src/spotlight/browser-profiles-events.ts apps/desktop/src/spotlight/browser-profiles-events.test.ts
git commit -m "feat(spotlight): 添加浏览器身份变更事件"
```

---

### Task 2: Browser Profiles Spotlight Cache Guard

**Files:**

- Create: `apps/desktop/src/spotlight/browser-profiles-refresh.ts`
- Create: `apps/desktop/src/spotlight/browser-profiles-refresh.test.ts`

**Step 1: Write the failing tests**

Create `apps/desktop/src/spotlight/browser-profiles-refresh.test.ts`.

```ts
import { describe, expect, it } from "vitest";
import type { SpotlightItem, SpotlightProviderId } from "./types";
import {
  BROWSER_PROFILES_PROVIDER_ID,
  beginBrowserProfilesLocalRefresh,
  canWriteBrowserProfiles,
  captureBrowserProfilesPrefetchVersion,
  createBrowserProfilesRefreshGuard,
  replaceBrowserProfilesItems,
} from "./browser-profiles-refresh";

function item(title: string): SpotlightItem {
  return {
    providerId: BROWSER_PROFILES_PROVIDER_ID,
    itemId: `edge:${title}`,
    title,
    searchFields: [{ text: title, initials: "", weight: 1 }],
  };
}

describe("browser profile Spotlight refresh guard", () => {
  it("allows only the latest local refresh to write", () => {
    const guard = createBrowserProfilesRefreshGuard();
    const first = beginBrowserProfilesLocalRefresh(guard);
    const second = beginBrowserProfilesLocalRefresh(guard);

    expect(canWriteBrowserProfiles(guard, first)).toBe(false);
    expect(canWriteBrowserProfiles(guard, second)).toBe(true);
  });

  it("blocks an older prefetchAll write after a local refresh", () => {
    const guard = createBrowserProfilesRefreshGuard();
    const prefetchVersion = captureBrowserProfilesPrefetchVersion(guard);
    const localVersion = beginBrowserProfilesLocalRefresh(guard);

    expect(canWriteBrowserProfiles(guard, prefetchVersion)).toBe(false);
    expect(canWriteBrowserProfiles(guard, localVersion)).toBe(true);
  });

  it("replaces only browser profile provider items", () => {
    const current = new Map<SpotlightProviderId, SpotlightItem[]>([
      ["tool", [{ providerId: "tool", itemId: "json", title: "JSON", searchFields: [] }]],
      [BROWSER_PROFILES_PROVIDER_ID, [item("old-alias")]],
    ]);

    const next = replaceBrowserProfilesItems(current, [item("new-alias")]);

    expect(next).not.toBe(current);
    expect(next.get("tool")?.[0]?.title).toBe("JSON");
    expect(next.get(BROWSER_PROFILES_PROVIDER_ID)?.map((entry) => entry.title)).toEqual([
      "new-alias",
    ]);
  });
});
```

Run: `pnpm test src/spotlight/browser-profiles-refresh.test.ts`

Expected: FAIL because `browser-profiles-refresh.ts` does not exist.

**Step 2: Implement the refresh helper**

Create `apps/desktop/src/spotlight/browser-profiles-refresh.ts`.

```ts
import type { SpotlightItem, SpotlightProviderId } from "./types";

export const BROWSER_PROFILES_PROVIDER_ID = "browser-profiles" as const;

export interface BrowserProfilesRefreshGuard {
  writeVersion: number;
}

export function createBrowserProfilesRefreshGuard(): BrowserProfilesRefreshGuard {
  return { writeVersion: 0 };
}

export function beginBrowserProfilesLocalRefresh(guard: BrowserProfilesRefreshGuard): number {
  guard.writeVersion += 1;
  return guard.writeVersion;
}

export function captureBrowserProfilesPrefetchVersion(guard: BrowserProfilesRefreshGuard): number {
  return guard.writeVersion;
}

export function canWriteBrowserProfiles(
  guard: BrowserProfilesRefreshGuard,
  version: number,
): boolean {
  return guard.writeVersion === version;
}

export function replaceBrowserProfilesItems(
  current: Map<SpotlightProviderId, SpotlightItem[]>,
  items: SpotlightItem[],
): Map<SpotlightProviderId, SpotlightItem[]> {
  const next = new Map(current);
  next.set(BROWSER_PROFILES_PROVIDER_ID, items);
  return next;
}
```

**Step 3: Verify**

Run: `pnpm test src/spotlight/browser-profiles-refresh.test.ts`

Expected: PASS.

**Step 4: Commit**

```bash
git add apps/desktop/src/spotlight/browser-profiles-refresh.ts apps/desktop/src/spotlight/browser-profiles-refresh.test.ts
git commit -m "test(spotlight): 覆盖浏览器身份缓存刷新守卫"
```

---

### Task 3: Notify Successful Browser Profile Mutations

**Files:**

- Modify: `apps/desktop/src/components/BrowserProfilesPanel.vue`
- Modify: `apps/desktop/src/spotlight/providers/browser-profiles.ts`
- Modify: `apps/desktop/src/spotlight/providers/browser-profiles.test.ts`

**Step 1: Write failing provider tests**

In `apps/desktop/src/spotlight/providers/browser-profiles.test.ts`, mock the event wrapper before importing the provider.

Add near the existing mocks:

```ts
const notifyBrowserProfilesChanged = vi.fn();

vi.mock("../browser-profiles-events", () => ({
  notifyBrowserProfilesChanged: (...args: unknown[]) => notifyBrowserProfilesChanged(...args),
}));
```

Reset it in `beforeEach`:

```ts
notifyBrowserProfilesChanged.mockReset();
```

Update the existing default action test:

```ts
expect(notifyBrowserProfilesChanged).toHaveBeenCalledWith("launch");
```

Add a failure-path test:

```ts
it("does not notify when launch fails", async () => {
  invokeToolByChannel.mockRejectedValueOnce(new Error("boom"));
  const item = buildBrowserProfileSpotlightItem(
    profile({ profileDir: "Profile 2", alias: "管理员" }),
  );

  const result = await browserProfilesProvider.defaultAction(item, {} as never);

  expect(result.errorMessage).toBe("boom");
  expect(notifyBrowserProfilesChanged).not.toHaveBeenCalled();
});
```

Run: `pnpm test src/spotlight/providers/browser-profiles.test.ts`

Expected: FAIL because the provider does not call `notifyBrowserProfilesChanged("launch")`.

**Step 2: Notify from the Spotlight provider**

Modify `apps/desktop/src/spotlight/providers/browser-profiles.ts`.

Add:

```ts
import { notifyBrowserProfilesChanged } from "../browser-profiles-events";
```

Inside `launchProfile`, after the successful `tool:browser-profiles:launch` call and before returning success:

```ts
try {
  await notifyBrowserProfilesChanged("launch");
} catch {
  /* Spotlight cache refresh notification is best-effort. */
}
```

Do not add `edgePath` to the payload. The launch action must continue to pass only:

```ts
{
  browser: payload.browser,
  profileDir: payload.profileDir,
}
```

**Step 3: Notify from the browser profiles panel**

Modify `apps/desktop/src/components/BrowserProfilesPanel.vue`.

Add:

```ts
import {
  notifyBrowserProfilesChanged,
  type BrowserProfilesChangedReason,
} from "../spotlight/browser-profiles-events";
```

Add a small local helper:

```ts
function notifyProfilesChanged(reason: BrowserProfilesChangedReason) {
  void notifyBrowserProfilesChanged(reason).catch(() => undefined);
}
```

Call it only after successful backend operations:

```ts
// launchProfile success path
notifyProfilesChanged("launch");

// editAlias success path
notifyProfilesChanged("alias");

// setHidden success path
notifyProfilesChanged("hidden");

// chooseEdgePath success path
notifyProfilesChanged("edge-path");
```

Place each call after the corresponding `await invokeToolByChannel(...)` succeeds. It may be before or after `await loadProfiles()`, but it must not run in catch/cancel paths.

**Step 4: Verify provider tests**

Run: `pnpm test src/spotlight/providers/browser-profiles.test.ts`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/desktop/src/components/BrowserProfilesPanel.vue apps/desktop/src/spotlight/providers/browser-profiles.ts apps/desktop/src/spotlight/providers/browser-profiles.test.ts
git commit -m "feat(browser-profiles): 通知 Spotlight 刷新身份缓存"
```

---

### Task 4: Listen And Refresh Browser Profiles In Spotlight

**Files:**

- Modify: `apps/desktop/src/components/SpotlightPanel.vue`
- Test: `apps/desktop/src/spotlight/browser-profiles-refresh.test.ts`

**Step 1: Extend refresh helper tests for active index**

Append to `apps/desktop/src/spotlight/browser-profiles-refresh.test.ts`:

```ts
import { nextSpotlightActiveIndex } from "../utils/spotlight-active-index";

it("clamps active index after browser profile results shrink", () => {
  expect(
    nextSpotlightActiveIndex({
      currentIndex: 4,
      resultCount: 2,
      queryChanged: false,
    }),
  ).toBe(1);
});
```

Run: `pnpm test src/spotlight/browser-profiles-refresh.test.ts src/utils/spotlight-active-index.test.ts`

Expected: PASS. This test documents the existing clamp function used by `SpotlightPanel.vue`.

**Step 2: Import the provider, event wrapper, and refresh helper**

Modify `apps/desktop/src/components/SpotlightPanel.vue`.

Replace the side-effect import:

```ts
import "../spotlight/providers/browser-profiles";
```

with:

```ts
import { browserProfilesProvider } from "../spotlight/providers/browser-profiles";
import { listenBrowserProfilesChanged } from "../spotlight/browser-profiles-events";
import {
  BROWSER_PROFILES_PROVIDER_ID,
  beginBrowserProfilesLocalRefresh,
  canWriteBrowserProfiles,
  captureBrowserProfilesPrefetchVersion,
  createBrowserProfilesRefreshGuard,
  replaceBrowserProfilesItems,
} from "../spotlight/browser-profiles-refresh";
```

Importing `browserProfilesProvider` still executes provider registration because the provider module calls `registerProvider(...)`.

**Step 3: Add local state**

Near the existing `unlistenReset` / `unsubConfig` locals, add:

```ts
let unlistenBrowserProfilesChanged: UnlistenFn | null = null;
let browserProfilesListenerDisposed = false;
const browserProfilesRefreshGuard = createBrowserProfilesRefreshGuard();
```

**Step 4: Add a provider-local refresh function**

Add near `prefetchAll()`:

```ts
async function refreshBrowserProfilesProvider() {
  const version = beginBrowserProfilesLocalRefresh(browserProfilesRefreshGuard);
  try {
    const items = await browserProfilesProvider.prefetch();
    if (!canWriteBrowserProfiles(browserProfilesRefreshGuard, version)) return;
    itemsByProvider.value = replaceBrowserProfilesItems(itemsByProvider.value, items);
    activeIndex.value = nextSpotlightActiveIndex({
      currentIndex: activeIndex.value,
      resultCount: results.value.length,
      queryChanged: false,
    });
  } catch (err) {
    if (!canWriteBrowserProfiles(browserProfilesRefreshGuard, version)) return;
    console.warn("[Spotlight] refresh browser profiles failed:", err);
  }
}
```

**Step 5: Protect `prefetchAll()` writes for browser-profiles**

Inside `prefetchAll()`, before each provider `prefetch()` call, capture the version only for `browser-profiles`:

```ts
const browserProfilesPrefetchVersion =
  provider.id === BROWSER_PROFILES_PROVIDER_ID
    ? captureBrowserProfilesPrefetchVersion(browserProfilesRefreshGuard)
    : null;
```

Before writing successful items:

```ts
if (
  provider.id === BROWSER_PROFILES_PROVIDER_ID &&
  !canWriteBrowserProfiles(browserProfilesRefreshGuard, browserProfilesPrefetchVersion!)
) {
  return;
}
```

Use the same stale check in the catch block before writing `[]` for first-time failures:

```ts
if (
  provider.id === BROWSER_PROFILES_PROVIDER_ID &&
  !canWriteBrowserProfiles(browserProfilesRefreshGuard, browserProfilesPrefetchVersion!)
) {
  return;
}
```

Leave other providers unchanged.

**Step 6: Register the listener with lifecycle cleanup**

In `onMounted`, before `window.addEventListener("focus", onWindowFocus);`, add:

```ts
browserProfilesListenerDisposed = false;
void listenBrowserProfilesChanged(() => {
  void refreshBrowserProfilesProvider();
})
  .then((unlisten) => {
    if (browserProfilesListenerDisposed) {
      unlisten();
      return;
    }
    unlistenBrowserProfilesChanged = unlisten;
  })
  .catch(() => {
    /* Tauri event listener is best-effort; spotlight-reset remains the fallback. */
  });
```

In `onBeforeUnmount`, add:

```ts
browserProfilesListenerDisposed = true;
unlistenBrowserProfilesChanged?.();
unlistenBrowserProfilesChanged = null;
```

Do not clear `itemsByProvider` when the listener fires.

**Step 7: Verify focused tests**

Run:

```text
pnpm test src/spotlight/browser-profiles-refresh.test.ts src/spotlight/browser-profiles-events.test.ts src/spotlight/providers/browser-profiles.test.ts src/utils/spotlight-active-index.test.ts
```

Expected: PASS.

**Step 8: Commit**

```bash
git add apps/desktop/src/components/SpotlightPanel.vue apps/desktop/src/spotlight/browser-profiles-refresh.test.ts
git commit -m "feat(spotlight): 局部刷新浏览器身份结果"
```

---

### Task 5: Full Frontend Verification

**Files:**

- No code changes expected.

**Step 1: Run the browser profile and Spotlight tests**

Run:

```text
pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts src/spotlight/browser-profiles-events.test.ts src/spotlight/browser-profiles-refresh.test.ts src/spotlight/search.test.ts src/utils/spotlight-active-index.test.ts
```

Expected: PASS.

**Step 2: Run typecheck**

Run: `pnpm typecheck`

Expected: PASS.

**Step 3: Run renderer build**

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS.

**Step 4: Manual smoke**

Manual checks in the app:

1. Open Browser Profiles, rename a visible Profile from an old alias to a new alias.
2. Open Spotlight without restarting the app.
3. Search the new alias. Expected: the Profile appears and title uses the new alias.
4. Search the old alias. Expected: that Profile no longer appears via the old alias.
5. Search Edge display name or `Profile 2`. Expected: the Profile still appears.
6. Hide the Profile. Expected: Spotlight no longer shows it.
7. Restore the Profile. Expected: Spotlight can find it again.
8. Launch the Profile from Spotlight. Expected: launch succeeds and later empty Spotlight results reflect updated usage order.

**Step 5: Commit verification-only fixes if needed**

If verification requires small fixes, commit only those files:

```bash
git add <changed-files>
git commit -m "fix(spotlight): 补齐浏览器身份刷新边界"
```

If no fixes are needed, do not create an empty commit.

---

### Task 6: Record Process Note If Implementation Touches 3+ Files

**Files:**

- Modify: `process.md`

**Step 1: Add a short process note**

If the implementation changes three or more files, append a concise note to `process.md` after verification.

Suggested entry:

```md
## 2026-07-07: Spotlight 预取缓存变更用 provider 级事件失效

**场景**: 浏览器身份别名保存后，Spotlight 驻留窗口仍显示旧别名。

**经验**:

1. provider item 的搜索字段正确，不代表 Spotlight 驻留缓存会自动更新；跨窗口状态变更要有显式事件。
2. 局部刷新和全量 `prefetchAll()` 共享同一 provider 缓存时，要用统一版本号防止旧全量请求回写。
3. 会更新排序权重的默认动作也要触发刷新，否则从 Spotlight 启动后的使用统计不会反映到空输入结果。

**相关文件**:

- `apps/desktop/src/spotlight/browser-profiles-events.ts`
- `apps/desktop/src/spotlight/browser-profiles-refresh.ts`
- `apps/desktop/src/components/SpotlightPanel.vue`
- `apps/desktop/src/components/BrowserProfilesPanel.vue`
- `apps/desktop/src/spotlight/providers/browser-profiles.ts`
```

**Step 2: Commit process note**

Run:

```bash
git add process.md
git commit -m "docs(process): 记录 Spotlight 浏览器身份刷新经验"
```

Skip this task if implementation remains under three files.
