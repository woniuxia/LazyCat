import type { SpotlightItem, SpotlightProviderId } from "./types";

export const BROWSER_PROFILES_PROVIDER_ID = "browser-profiles" as const;

export interface BrowserProfilesRefreshGuard {
  writeVersion: number;
}

export function createBrowserProfilesRefreshGuard(): BrowserProfilesRefreshGuard {
  return { writeVersion: 0 };
}

export function beginBrowserProfilesLocalRefresh(
  guard: BrowserProfilesRefreshGuard,
): number {
  guard.writeVersion += 1;
  return guard.writeVersion;
}

export function captureBrowserProfilesPrefetchVersion(
  guard: BrowserProfilesRefreshGuard,
): number {
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
