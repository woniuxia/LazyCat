import { describe, expect, it } from "vitest";
import { buildJsonTree, encodeJsonTreePath } from "./jsonTreeView";
import {
  collectJsonTreeAncestorKeys,
  collectJsonTreeSearchMatches,
  jsonTreeSearchMatchId,
} from "./jsonTreeSearch";

describe("collectJsonTreeSearchMatches", () => {
  it("matches keys and scalar values case-insensitively in DFS document order", () => {
    const root = buildJsonTree({
      user: { name: "Alice", tags: ["Admin"] },
      admin: true,
    });

    const matches = collectJsonTreeSearchMatches(root, "ADMIN");

    expect(matches).toEqual([
      {
        key: encodeJsonTreePath(["user", "tags", 0]),
        path: ["user", "tags", 0],
        field: "value",
      },
      {
        key: encodeJsonTreePath(["admin"]),
        path: ["admin"],
        field: "key",
      },
    ]);
  });

  it("records two matches when both key and value of one node hit", () => {
    const root = buildJsonTree({ foo: "foo" });

    const matches = collectJsonTreeSearchMatches(root, "foo");

    expect(matches).toEqual([
      { key: encodeJsonTreePath(["foo"]), path: ["foo"], field: "key" },
      { key: encodeJsonTreePath(["foo"]), path: ["foo"], field: "value" },
    ]);
  });

  it("matches container nodes by label only, never by summary text", () => {
    const root = buildJsonTree({ items: [1, 2] });

    const matches = collectJsonTreeSearchMatches(root, "item");

    expect(matches).toEqual([
      { key: encodeJsonTreePath(["items"]), path: ["items"], field: "key" },
    ]);
  });

  it("matches the formatted value text of a scalar root", () => {
    const root = buildJsonTree("Hello world");

    const matches = collectJsonTreeSearchMatches(root, "hello");

    expect(matches).toEqual([{ key: "$", path: [], field: "value" }]);
  });

  it("matches numbers, booleans, and null by their formatted text", () => {
    const root = buildJsonTree({ count: 1024, flag: false, empty: null });

    expect(collectJsonTreeSearchMatches(root, "102")).toEqual([
      { key: encodeJsonTreePath(["count"]), path: ["count"], field: "value" },
    ]);
    expect(collectJsonTreeSearchMatches(root, "false")).toEqual([
      { key: encodeJsonTreePath(["flag"]), path: ["flag"], field: "value" },
    ]);
    expect(collectJsonTreeSearchMatches(root, "null")).toEqual([
      { key: encodeJsonTreePath(["empty"]), path: ["empty"], field: "value" },
    ]);
  });

  it("returns an empty list for an empty query", () => {
    const root = buildJsonTree({ user: "admin" });

    expect(collectJsonTreeSearchMatches(root, "")).toEqual([]);
  });
});

describe("jsonTreeSearchMatchId", () => {
  it("distinguishes key and value hits on the same node", () => {
    expect(jsonTreeSearchMatchId({ field: "key", key: "$" })).toBe("key:$");
    expect(jsonTreeSearchMatchId({ field: "value", key: "$/k:1:a" })).toBe("value:$/k:1:a");
  });
});

describe("collectJsonTreeAncestorKeys", () => {
  it("returns every ancestor key from root to parent", () => {
    expect(collectJsonTreeAncestorKeys(["a", 0, "b"])).toEqual([
      encodeJsonTreePath([]),
      encodeJsonTreePath(["a"]),
      encodeJsonTreePath(["a", 0]),
    ]);
  });

  it("returns an empty list for the root path", () => {
    expect(collectJsonTreeAncestorKeys([])).toEqual([]);
  });
});
