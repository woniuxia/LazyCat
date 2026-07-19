import { describe, expect, it } from "vitest";
import {
  BROWSER_PROFILE_BADGE_COLOR_COUNT,
  buildBrowserProfileMetaSegments,
  buildBrowserProfileSearchFields,
  filterBrowserProfiles,
  formatBrowserProfileLastLaunchedAt,
  formatBrowserProfileLastLaunchedAtCompact,
  getBrowserProfileBadgeColorIndex,
  getBrowserProfileBadgeInitial,
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

  it("filters profiles by alias, Edge display name, profile dir and pinyin initials", () => {
    const input = [
      profile({
        profileDir: "Default",
        alias: "管理员",
        edgeDisplayName: "个人",
      }),
      profile({
        profileDir: "Profile 2",
        alias: "",
        edgeDisplayName: "测试账号",
      }),
      profile({
        profileDir: "Profile 9",
        alias: "运维",
        edgeDisplayName: "Ops",
      }),
    ];

    expect(filterBrowserProfiles(input, "gly").map((item) => item.profileDir)).toEqual([
      "Default",
    ]);
    expect(filterBrowserProfiles(input, "测试").map((item) => item.profileDir)).toEqual([
      "Profile 2",
    ]);
    expect(
      filterBrowserProfiles(input, "profile 9").map((item) => item.profileDir),
    ).toEqual(["Profile 9"]);
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

  it("formats compact last launched time relative to now", () => {
    const now = new Date(2026, 6, 3, 15, 0);

    expect(formatBrowserProfileLastLaunchedAtCompact(null, now)).toBe("未启动过");
    expect(formatBrowserProfileLastLaunchedAtCompact("not-a-date", now)).toBe(
      "未启动过",
    );
    expect(
      formatBrowserProfileLastLaunchedAtCompact("2026-07-03T09:05:00", now),
    ).toBe("今天 09:05");
    expect(
      formatBrowserProfileLastLaunchedAtCompact("2026-07-02T23:59:00", now),
    ).toBe("昨天 23:59");
    expect(
      formatBrowserProfileLastLaunchedAtCompact("2026-03-18T10:00:00", now),
    ).toBe("3月18日");
    expect(
      formatBrowserProfileLastLaunchedAtCompact("2025-12-01T10:00:00", now),
    ).toBe("2025/12/1");
  });

  it("builds badge initial from display name with fallback", () => {
    expect(
      getBrowserProfileBadgeInitial(profile({ alias: "管理员" })),
    ).toBe("管");
    expect(
      getBrowserProfileBadgeInitial(profile({ profileDir: "profile 3" })),
    ).toBe("P");
    expect(
      getBrowserProfileBadgeInitial(
        profile({ alias: "", edgeDisplayName: "", profileDir: "  " }),
      ),
    ).toBe("?");
  });

  it("assigns stable badge color index within palette range", () => {
    const first = profile({ profileDir: "Profile 1" });
    const aliased = profile({ profileDir: "Profile 1", alias: "改名后" });

    expect(getBrowserProfileBadgeColorIndex(first)).toBe(
      getBrowserProfileBadgeColorIndex(aliased),
    );
    for (const dir of ["Default", "Profile 1", "Profile 2", "Profile 10"]) {
      const index = getBrowserProfileBadgeColorIndex(profile({ profileDir: dir }));
      expect(index).toBeGreaterThanOrEqual(0);
      expect(index).toBeLessThan(BROWSER_PROFILE_BADGE_COLOR_COUNT);
    }
  });

  it("builds meta segments without duplicating the display name", () => {
    const now = new Date(2026, 6, 3, 15, 0);

    expect(
      buildBrowserProfileMetaSegments(
        profile({
          alias: "工作",
          edgeDisplayName: "张三",
          profileDir: "Profile 3",
          launchCount: 12,
          lastLaunchedAt: "2026-07-02T09:30:00",
        }),
        now,
      ),
    ).toEqual(["Profile 3", "Edge：张三", "12 次", "昨天 09:30"]);

    expect(
      buildBrowserProfileMetaSegments(
        profile({ profileDir: "Default", launchCount: 0 }),
        now,
      ),
    ).toEqual(["未启动过"]);

    expect(
      buildBrowserProfileMetaSegments(
        profile({
          edgeDisplayName: "个人",
          profileDir: "Profile 2",
          launchCount: 3,
          lastLaunchedAt: "2026-07-03T08:00:00",
        }),
        now,
      ),
    ).toEqual(["Profile 2", "3 次", "今天 08:00"]);
  });

  it("labels Chrome profile metadata with the correct browser", () => {
    expect(
      buildBrowserProfileMetaSegments(
        profile({
          browser: "chrome",
          alias: "工作",
          edgeDisplayName: "公司账号",
          profileDir: "Profile 6",
        }),
      ),
    ).toEqual(["Profile 6", "Chrome：公司账号", "未启动过"]);
  });
});
