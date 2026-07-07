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
      [
        "tool",
        [{ providerId: "tool", itemId: "json", title: "JSON", searchFields: [] }],
      ],
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
