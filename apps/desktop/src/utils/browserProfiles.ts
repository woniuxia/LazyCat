import type { BrowserProfileItem } from "../types/browser-profiles";
import type { SearchField } from "./fuzzy-match";
import { matchScore, toPinyinInitials } from "./fuzzy-match";

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

export function filterBrowserProfiles(
  profiles: readonly BrowserProfileItem[],
  queryRaw: string,
): BrowserProfileItem[] {
  const query = queryRaw.trim();
  if (!query) return [...profiles];

  return profiles
    .map((profile, index) => ({
      profile,
      index,
      score: matchScore(query, buildBrowserProfileSearchFields(profile)),
    }))
    .filter((item) => item.score > 0)
    .sort((left, right) => right.score - left.score || left.index - right.index)
    .map((item) => item.profile);
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

export function formatBrowserProfileLastLaunchedAtCompact(
  value: string | null,
  now: Date = new Date(),
): string {
  if (!value) return "未启动过";
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "未启动过";

  const time = new Date(timestamp);
  const dayStart = (date: Date) =>
    new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const dayDiff = Math.round((dayStart(now) - dayStart(time)) / 86_400_000);
  const clock = `${String(time.getHours()).padStart(2, "0")}:${String(
    time.getMinutes(),
  ).padStart(2, "0")}`;

  if (dayDiff === 0) return `今天 ${clock}`;
  if (dayDiff === 1) return `昨天 ${clock}`;
  if (time.getFullYear() === now.getFullYear() && dayDiff > 0) {
    return `${time.getMonth() + 1}月${time.getDate()}日`;
  }
  return `${time.getFullYear()}/${time.getMonth() + 1}/${time.getDate()}`;
}

export const BROWSER_PROFILE_BADGE_COLOR_COUNT = 8;

export function getBrowserProfileBadgeInitial(profile: BrowserProfileItem): string {
  const name = getBrowserProfileDisplayName(profile);
  const first = [...name][0];
  if (!first) return "?";
  return first.toLocaleUpperCase("zh-CN");
}

export function getBrowserProfileBadgeColorIndex(profile: BrowserProfileItem): number {
  const key = `${profile.browser}:${profile.profileDir}`;
  let hash = 5381;
  for (let index = 0; index < key.length; index++) {
    hash = ((hash << 5) + hash + key.charCodeAt(index)) >>> 0;
  }
  return hash % BROWSER_PROFILE_BADGE_COLOR_COUNT;
}

export function buildBrowserProfileMetaSegments(
  profile: BrowserProfileItem,
  now: Date = new Date(),
): string[] {
  const segments: string[] = [];
  const displayName = getBrowserProfileDisplayName(profile);
  const profileDir = profile.profileDir.trim();
  if (profileDir && profileDir !== displayName) segments.push(profileDir);

  const browserName = profile.edgeDisplayName.trim();
  const browserLabel = profile.browser === "chrome" ? "Chrome" : "Edge";
  if (browserName && browserName !== displayName) segments.push(`${browserLabel}：${browserName}`);

  if (profile.launchCount > 0) segments.push(`${profile.launchCount} 次`);
  segments.push(formatBrowserProfileLastLaunchedAtCompact(profile.lastLaunchedAt, now));
  return segments;
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
