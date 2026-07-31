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
