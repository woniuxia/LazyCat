import type { UnlistenFn } from "@tauri-apps/api/event";

export const BROWSER_PROFILES_CHANGED_EVENT = "browser-profiles-changed";

export type BrowserProfilesChangedReason =
  | "alias"
  | "hidden"
  | "edge-path"
  | "launch";

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
