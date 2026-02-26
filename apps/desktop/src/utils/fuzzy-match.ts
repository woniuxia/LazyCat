import { pinyin } from "pinyin-pro";

function normalizeWhitespace(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, " ");
}

function normalizeCompact(value: string): string {
  return normalizeWhitespace(value).replace(/\s+/g, "");
}

function hasCjk(value: string): boolean {
  return /[\u3400-\u9fff]/.test(value);
}

export function toPinyinInitials(value: string): string {
  if (!value || !hasCjk(value)) return "";
  try {
    const result = pinyin(value, {
      pattern: "first",
      toneType: "none",
      type: "array",
    }) as string[] | string;
    if (Array.isArray(result)) return normalizeCompact(result.join(""));
    return normalizeCompact(result);
  } catch {
    return "";
  }
}

function subsequenceScore(query: string, target: string): number {
  let qi = 0;
  let ti = 0;
  let contiguous = 0;
  let lastHit = -2;
  let firstHit = -1;

  while (qi < query.length && ti < target.length) {
    if (query[qi] === target[ti]) {
      if (firstHit < 0) firstHit = ti;
      if (ti === lastHit + 1) contiguous += 1;
      lastHit = ti;
      qi += 1;
    }
    ti += 1;
  }

  if (qi !== query.length) return -1;
  return 520 + contiguous * 6 - (firstHit < 0 ? 0 : firstHit);
}

function baseTextScore(query: string, target: string): number {
  if (!query || !target) return -1;
  if (target.startsWith(query)) return 1200 - Math.min(target.length - query.length, 200);
  const index = target.indexOf(query);
  if (index >= 0) return 980 - index * 2;
  return subsequenceScore(query, target);
}

export interface SearchField {
  text: string;
  initials: string;
  weight: number;
}

export function matchScore(queryRaw: string, fields: SearchField[]): number {
  const query = normalizeCompact(queryRaw);
  if (!query) return 0;

  let best = -1;
  for (const field of fields) {
    const text = normalizeCompact(field.text);
    const initials = normalizeCompact(field.initials);
    if (!text && !initials) continue;

    const textScore = baseTextScore(query, text);
    if (textScore > 0) {
      best = Math.max(best, Math.round(textScore * field.weight));
    }

    if (initials) {
      const pinyinScore = baseTextScore(query, initials);
      if (pinyinScore > 0) {
        // Slightly lower than direct text match.
        best = Math.max(best, Math.round(pinyinScore * field.weight * 0.88));
      }
    }
  }

  return best;
}
