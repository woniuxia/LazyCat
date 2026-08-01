import { describe, expect, it } from "vitest";
import {
  createSearchField,
  matchPreparedQuery,
  matchScore,
  prepareSearchQuery,
} from "./fuzzy-match";

describe("fuzzy-match", () => {
  it("precomputes normalized, compact and pinyin field forms", () => {
    const field = createSearchField("数据-字典", 1);

    expect(field.normalizedText).toBe("数据 字典");
    expect(field.compactText).toBe("数据字典");
    expect(field.fullPinyin).toBe("shu ju zi dian");
    expect(field.compactPinyin).toBe("shujuzidian");
    expect(field.initials).toBe("sjzd");
  });

  it("treats common separators as equivalent", () => {
    const field = createSearchField("request_forward.rule", 1);

    expect(matchScore("request forward rule", [field])).toBeGreaterThan(0);
    expect(matchScore("request/forward-rule", [field])).toBeGreaterThan(0);
  });

  it("requires every query token and allows tokens to match across fields", () => {
    const fields = [createSearchField("LazyCat", 1), createSearchField("Spotlight 搜索", 1)];
    const query = prepareSearchQuery("lazy 搜索");

    expect(matchPreparedQuery(query, fields)).toBeGreaterThan(0);
    expect(matchScore("lazy missing", fields)).toBe(-1);
  });

  it("matches Chinese fields by full pinyin and initials", () => {
    const field = createSearchField("数据字典", 1);

    expect(matchScore("shujuzidian", [field])).toBeGreaterThan(0);
    expect(matchScore("sjzd", [field])).toBeGreaterThan(0);
  });

  it("penalizes wider subsequence spans", () => {
    const compact = matchScore("spt", [createSearchField("spot", 1)]);
    const scattered = matchScore("spt", [createSearchField("sxxxxpxxxxt", 1)]);

    expect(compact).toBeGreaterThan(scattered);
  });
});
