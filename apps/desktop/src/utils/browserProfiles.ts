import type { BrowserProfileItem } from "../types/browser-profiles";
import type { SearchField } from "./fuzzy-match";
import { toPinyinInitials } from "./fuzzy-match";

export interface BrowserProfileGroups {
  visible: BrowserProfileItem[];
  hidden: BrowserProfileItem[];
}

export function getBrowserProfileDisplayName(profile: BrowserProfileItem): string {
  for (const value of [profile.alias, profile.edgeDisplayName, profile.profileDir]) {
    const trimmed = value.trim();
    if (trimmed) return trimmed;
  }
  return "";
}

export function sortBrowserProfiles(
  profiles: readonly BrowserProfileItem[],
): BrowserProfileItem[] {
  return [...profiles].sort((left, right) => {
    if (left.hidden !== right.hidden) return left.hidden ? 1 : -1;
    if (left.launchCount !== right.launchCount) {
      return right.launchCount - left.launchCount;
    }

    const leftTime = parseComparableTime(left.lastLaunchedAt);
    const rightTime = parseComparableTime(right.lastLaunchedAt);
    if (leftTime !== rightTime) return rightTime - leftTime;

    const displayCompare = getBrowserProfileDisplayName(left).localeCompare(
      getBrowserProfileDisplayName(right),
      "zh-CN",
      { sensitivity: "base" },
    );
    if (displayCompare !== 0) return displayCompare;

    return left.profileDir.localeCompare(right.profileDir, "zh-CN", {
      sensitivity: "base",
    });
  });
}

export function splitBrowserProfilesByHidden(
  profiles: readonly BrowserProfileItem[],
): BrowserProfileGroups {
  return {
    visible: profiles.filter((profile) => !profile.hidden),
    hidden: profiles.filter((profile) => profile.hidden),
  };
}

export function formatBrowserProfileLastLaunchedAt(value: string | null): string {
  if (!value) return "未启动过";
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "未启动过";
  return new Date(timestamp).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function buildBrowserProfileSearchFields(profile: BrowserProfileItem): SearchField[] {
  const fields: SearchField[] = [];
  const seen = new Set<string>();

  for (const candidate of [
    { text: profile.alias, weight: 1.4 },
    { text: profile.edgeDisplayName, weight: 1 },
    { text: profile.profileDir, weight: 0.8 },
  ]) {
    const text = candidate.text.trim();
    const key = text.toLocaleLowerCase("zh-CN");
    if (!text || seen.has(key)) continue;
    seen.add(key);
    fields.push({
      text,
      initials: toPinyinInitials(text),
      weight: candidate.weight,
    });
  }

  return fields;
}

export function getBrowserProfileSpotlightWeight(profile: BrowserProfileItem): number {
  return 1.08 + Math.min(Math.max(profile.launchCount, 0), 50) * 0.012;
}

function parseComparableTime(value: string | null): number {
  if (!value) return Number.NEGATIVE_INFINITY;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? timestamp : Number.NEGATIVE_INFINITY;
}
