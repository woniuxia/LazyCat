import { pinyin } from "pinyin-pro";

const SEARCH_SEPARATOR_PATTERN = /[\s\\/_.\-:：@|,，;；·]+/gu;

export interface SearchField {
  text: string;
  initials: string;
  weight: number;
  normalizedText?: string;
  compactText?: string;
  fullPinyin?: string;
  compactPinyin?: string;
}

export interface PreparedSearchQuery {
  normalized: string;
  compact: string;
  tokens: string[];
}

export function normalizeSearchText(value: string): string {
  return value
    .normalize("NFKC")
    .toLocaleLowerCase("zh-CN")
    .replace(SEARCH_SEPARATOR_PATTERN, " ")
    .trim()
    .replace(/\s+/g, " ");
}

function compactSearchText(value: string): string {
  return normalizeSearchText(value).replace(/\s+/g, "");
}

function hasCjk(value: string): boolean {
  return /[\u3400-\u9fff]/u.test(value);
}

function toPinyin(value: string, pattern: "pinyin" | "first"): string {
  if (!value || !hasCjk(value)) return "";
  try {
    return pinyin(value, {
      pattern,
      toneType: "none",
      type: "string",
      separator: pattern === "first" ? "" : " ",
      nonZh: "consecutive",
      v: true,
    });
  } catch {
    return "";
  }
}

export function toPinyinInitials(value: string): string {
  return compactSearchText(toPinyin(value, "first"));
}

export function toFullPinyin(value: string): string {
  return normalizeSearchText(toPinyin(value, "pinyin"));
}

export function createSearchField(text: string, weight: number): SearchField {
  const cleaned = text.trim();
  const normalizedText = normalizeSearchText(cleaned);
  const fullPinyin = toFullPinyin(cleaned);
  return {
    text: cleaned,
    initials: toPinyinInitials(cleaned),
    weight,
    normalizedText,
    compactText: normalizedText.replace(/\s+/g, ""),
    fullPinyin,
    compactPinyin: fullPinyin.replace(/\s+/g, ""),
  };
}

export function prepareSearchQuery(query: string): PreparedSearchQuery {
  const normalized = normalizeSearchText(query);
  return {
    normalized,
    compact: normalized.replace(/\s+/g, ""),
    tokens: normalized ? normalized.split(" ") : [],
  };
}

function subsequenceScore(query: string, target: string): number {
  let queryIndex = 0;
  let targetIndex = 0;
  let firstHit = -1;
  let lastHit = -1;
  let contiguousPairs = 0;

  while (queryIndex < query.length && targetIndex < target.length) {
    if (query[queryIndex] === target[targetIndex]) {
      if (firstHit < 0) firstHit = targetIndex;
      if (lastHit + 1 === targetIndex) contiguousPairs += 1;
      lastHit = targetIndex;
      queryIndex += 1;
    }
    targetIndex += 1;
  }

  if (queryIndex !== query.length) return -1;
  const span = lastHit - firstHit + 1;
  const gaps = Math.max(0, span - query.length);
  const tail = Math.max(0, target.length - lastHit - 1);
  return Math.max(1, 620 + contiguousPairs * 18 - gaps * 14 - firstHit * 3 - tail * 0.5);
}

function baseTextScore(query: string, target: string): number {
  if (!query || !target) return -1;
  if (target === query) return 1400;
  if (target.startsWith(query)) return 1260 - Math.min(target.length - query.length, 220);
  const index = target.indexOf(query);
  if (index >= 0) return 1040 - Math.min(index * 3, 300);
  return subsequenceScore(query, target);
}

function indexedField(field: SearchField): Required<SearchField> {
  if (
    field.normalizedText !== undefined &&
    field.compactText !== undefined &&
    field.fullPinyin !== undefined &&
    field.compactPinyin !== undefined
  ) {
    return field as Required<SearchField>;
  }
  const indexed = createSearchField(field.text, field.weight);
  return {
    ...indexed,
    initials: field.initials ? compactSearchText(field.initials) : indexed.initials,
    normalizedText: field.normalizedText ?? indexed.normalizedText,
    compactText: field.compactText ?? indexed.compactText,
    fullPinyin: field.fullPinyin ?? indexed.fullPinyin,
    compactPinyin: field.compactPinyin ?? indexed.compactPinyin,
  } as Required<SearchField>;
}

function fieldTokenScore(token: string, field: Required<SearchField>): number {
  const variants = [
    { value: field.normalizedText, factor: 1 },
    { value: field.compactText, factor: 0.98 },
    { value: field.fullPinyin, factor: 0.9 },
    { value: field.compactPinyin, factor: 0.9 },
    { value: field.initials, factor: 0.86 },
  ];
  let best = -1;
  for (const variant of variants) {
    const score = baseTextScore(token, variant.value);
    if (score > 0) best = Math.max(best, score * variant.factor * field.weight);
  }
  return best;
}

function phraseScore(query: PreparedSearchQuery, field: Required<SearchField>): number {
  if (query.tokens.length < 2) return -1;
  const variants = [
    { value: field.compactText, factor: 1 },
    { value: field.compactPinyin, factor: 0.9 },
  ];
  let best = -1;
  for (const variant of variants) {
    const score = baseTextScore(query.compact, variant.value);
    if (score > 0) best = Math.max(best, score * variant.factor * field.weight);
  }
  return best;
}

export function matchPreparedQuery(
  query: PreparedSearchQuery,
  fields: SearchField[],
): number {
  if (query.tokens.length === 0) return 0;
  const indexed = fields.map(indexedField);
  if (indexed.length === 0) return -1;

  let tokenTotal = 0;
  for (const token of query.tokens) {
    let best = -1;
    for (const field of indexed) best = Math.max(best, fieldTokenScore(token, field));
    if (best <= 0) return -1;
    tokenTotal += best;
  }

  const tokenScore = tokenTotal / query.tokens.length + Math.min(72, query.tokens.length * 12);
  let bestPhrase = -1;
  for (const field of indexed) bestPhrase = Math.max(bestPhrase, phraseScore(query, field));
  return Math.round(Math.max(tokenScore, bestPhrase));
}

export function matchScore(queryRaw: string, fields: SearchField[]): number {
  return matchPreparedQuery(prepareSearchQuery(queryRaw), fields);
}
