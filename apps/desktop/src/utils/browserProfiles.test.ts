import { describe, expect, it } from "vitest";
import {
  buildBrowserProfileSearchFields,
  formatBrowserProfileLastLaunchedAt,
  getBrowserProfileDisplayName,
  getBrowserProfileSpotlightWeight,
  sortBrowserProfiles,
  splitBrowserProfilesByHidden,
} from "./browserProfiles";
import type { BrowserProfileItem } from "../types/browser-profiles";

function profile(overrides: Partial<BrowserProfileItem>): BrowserProfileItem {
  return {
    browser: "edge",
    profileDir: "Default",
    edgeDisplayName: "",
    alias: "",
    hidden: false,
    launchCount: 0,
    lastLaunchedAt: null,
    ...overrides,
  };
}

describe("browserProfiles utils", () => {
  it("uses alias, then Edge display name, then profile dir as display name", () => {
    expect(
      getBrowserProfileDisplayName(
        profile({ alias: "管理员", edgeDisplayName: "个人" }),
      ),
    ).toBe("管理员");
    expect(
      getBrowserProfileDisplayName(
        profile({ alias: "", edgeDisplayName: "个人" }),
      ),
    ).toBe("个人");
    expect(
      getBrowserProfileDisplayName(
        profile({
          alias: "",
          edgeDisplayName: "",
          profileDir: "Profile 2",
        }),
      ),
    ).toBe("Profile 2");
  });

  it("sorts visible profiles before hidden profiles by usage and display name", () => {
    const sorted = sortBrowserProfiles([
      profile({
        profileDir: "Profile 4",
        edgeDisplayName: "Hidden",
        hidden: true,
        launchCount: 99,
      }),
      profile({
        profileDir: "Profile 2",
        alias: "管理员",
        launchCount: 3,
        lastLaunchedAt: "2026-07-01T09:00:00+08:00",
      }),
      profile({
        profileDir: "Default",
        edgeDisplayName: "Alpha",
        launchCount: 3,
        lastLaunchedAt: "2026-07-02T09:00:00+08:00",
      }),
      profile({
        profileDir: "Profile 3",
        edgeDisplayName: "Beta",
        launchCount: 2,
      }),
    ]);

    expect(sorted.map((item) => item.profileDir)).toEqual([
      "Default",
      "Profile 2",
      "Profile 3",
      "Profile 4",
    ]);
  });

  it("splits visible and hidden profiles without mutating input", () => {
    const input = [
      profile({ profileDir: "Default" }),
      profile({ profileDir: "Profile 2", hidden: true }),
    ];
    const grouped = splitBrowserProfilesByHidden(input);

    expect(grouped.visible.map((item) => item.profileDir)).toEqual(["Default"]);
    expect(grouped.hidden.map((item) => item.profileDir)).toEqual([
      "Profile 2",
    ]);
    expect(input[0].hidden).toBe(false);
  });

  it("builds Spotlight search fields from alias, display name and profile dir", () => {
    const fields = buildBrowserProfileSearchFields(
      profile({
        alias: "管理员",
        edgeDisplayName: "测试账号",
        profileDir: "Profile 2",
      }),
    );

    expect(fields.map((field) => field.text)).toEqual([
      "管理员",
      "测试账号",
      "Profile 2",
    ]);
    expect(fields[0].weight).toBeGreaterThan(fields[1].weight);
  });

  it("increases Spotlight item weight with launch count but caps growth", () => {
    expect(getBrowserProfileSpotlightWeight(profile({ launchCount: 0 }))).toBeGreaterThan(
      1,
    );
    expect(
      getBrowserProfileSpotlightWeight(profile({ launchCount: 30 })),
    ).toBeGreaterThan(
      getBrowserProfileSpotlightWeight(profile({ launchCount: 1 })),
    );
    expect(
      getBrowserProfileSpotlightWeight(profile({ launchCount: 999 })),
    ).toBe(getBrowserProfileSpotlightWeight(profile({ launchCount: 50 })));
  });

  it("formats empty last launched time as never used", () => {
    expect(formatBrowserProfileLastLaunchedAt(null)).toBe("未启动过");
  });
});
