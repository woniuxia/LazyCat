import { describe, expect, it } from "vitest";
import { canEnterJsonTree, JSON_TREE_TEXT_LIMIT } from "./jsonProcessTree";

describe("canEnterJsonTree", () => {
  it("passes valid JSON within the size limit", () => {
    const result = canEnterJsonTree('{"a":1}');
    expect(result).toEqual({ ok: true, value: { a: 1 } });

    const boundary = `"${"a".repeat(JSON_TREE_TEXT_LIMIT - 2)}"`;
    expect(boundary.length).toBe(JSON_TREE_TEXT_LIMIT);
    expect(canEnterJsonTree(boundary).ok).toBe(true);
  });

  it("rejects oversized content before parsing", () => {
    const oversized = `"${"a".repeat(JSON_TREE_TEXT_LIMIT - 1)}"`;
    expect(oversized.length).toBe(JSON_TREE_TEXT_LIMIT + 1);
    const result = canEnterJsonTree(oversized);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toContain("超过");
  });

  it("rejects invalid JSON with the parser message", () => {
    const result = canEnterJsonTree("{a:1}");
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toContain("JSON 解析失败");
      expect(result.reason.length).toBeGreaterThan("JSON 解析失败: ".length);
    }
    expect(canEnterJsonTree("").ok).toBe(false);
  });
});
