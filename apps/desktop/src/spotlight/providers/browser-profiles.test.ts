import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeToolByChannel = vi.fn();
const invoke = vi.fn();

vi.mock("../../bridge/tauri", () => ({
  invokeToolByChannel: (...args: unknown[]) => invokeToolByChannel(...args),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import {
  browserProfilesProvider,
  buildBrowserProfileSpotlightItem,
} from "./browser-profiles";
import type { BrowserProfileItem } from "../../types/browser-profiles";

function profile(overrides: Partial<BrowserProfileItem>): BrowserProfileItem {
  return {
    browser: "edge",
    profileDir: "Default",
    edgeDisplayName: "个人",
    alias: "",
    hidden: false,
    launchCount: 0,
    lastLaunchedAt: null,
    ...overrides,
  };
}

beforeEach(() => {
  invokeToolByChannel.mockReset();
  invoke.mockReset();
});

describe("buildBrowserProfileSpotlightItem", () => {
  it("maps alias display name and searchable fields", () => {
    const item = buildBrowserProfileSpotlightItem(
      profile({
        profileDir: "Profile 2",
        alias: "管理员",
        edgeDisplayName: "测试账号",
        launchCount: 8,
      }),
    );

    expect(item.providerId).toBe("browser-profiles");
    expect(item.itemId).toBe("edge:Profile 2");
    expect(item.title).toBe("管理员");
    expect(item.subtitle).toContain("测试账号");
    expect(item.subtitle).toContain("Profile 2");
    expect(item.badge).toEqual({ short: "Edge", tone: "primary" });
    expect(item.searchFields.map((field) => field.text)).toEqual([
      "管理员",
      "测试账号",
      "Profile 2",
    ]);
    expect(item.weight).toBeGreaterThan(1.08);
  });
});

describe("browserProfilesProvider", () => {
  it("prefetches only visible profiles", async () => {
    invokeToolByChannel.mockResolvedValue({
      profiles: [
        profile({ profileDir: "Default" }),
        profile({ profileDir: "Profile 2", hidden: true }),
      ],
    });

    const items = await browserProfilesProvider.prefetch();

    expect(invokeToolByChannel).toHaveBeenCalledWith(
      "tool:browser-profiles:list",
      {},
    );
    expect(items.map((item) => item.itemId)).toEqual(["edge:Default"]);
  });

  it("launches selected profile as default action", async () => {
    const item = buildBrowserProfileSpotlightItem(
      profile({
        profileDir: "Profile 2",
        alias: "管理员",
      }),
    );

    const result = await browserProfilesProvider.defaultAction(item, {} as never);

    expect(invokeToolByChannel).toHaveBeenCalledWith(
      "tool:browser-profiles:launch",
      {
        browser: "edge",
        profileDir: "Profile 2",
      },
    );
    expect(result).toEqual({
      closeSpotlight: true,
      toast: { message: "已打开 Edge：管理员", type: "success" },
    });
  });

  it("opens browser profiles tool from action menu", async () => {
    const item = buildBrowserProfileSpotlightItem(
      profile({ profileDir: "Default" }),
    );

    const result = await browserProfilesProvider.executeAction?.(
      item,
      "open_tool",
      {} as never,
    );

    expect(invoke).toHaveBeenCalledWith("spotlight_pick", {
      target: "browser-profiles",
    });
    expect(result).toEqual({ closeSpotlight: true });
  });

  it("returns explicit error for malformed payload", async () => {
    const result = await browserProfilesProvider.defaultAction(
      {
        providerId: "browser-profiles",
        itemId: "bad",
        title: "bad",
        searchFields: [],
        payload: {},
      },
      {} as never,
    );

    expect(result.errorMessage).toContain("无效");
  });
});
