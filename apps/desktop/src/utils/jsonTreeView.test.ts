import { describe, expect, it } from "vitest";
import {
  buildJsonTree,
  collectExpandableKeys,
  collectExpandedKeysByDepth,
  formatJsonForCopy,
  isJsonTreeExpandable,
  summarizeJsonNode,
  toJsonPath,
} from "./jsonTreeView";

function collectSummaries(node: ReturnType<typeof buildJsonTree>): string[] {
  return [node.summary, ...node.children.flatMap(collectSummaries)];
}

describe("jsonTreeView", () => {
  it("builds object, array, and scalar nodes", () => {
    const root = buildJsonTree({
      user: { name: "张三" },
      roles: ["admin"],
      active: true,
      none: null,
    });

    expect(root.key).toBe("$");
    expect(root.depth).toBe(0);
    expect(root.path).toEqual([]);
    expect(root.valueType).toBe("object");
    expect(root.childCount).toBe(4);
    expect(root.summary).toBe("4 fields");
    expect(root.children.map((child) => child.label)).toEqual([
      '"user"',
      '"roles"',
      '"active"',
      '"none"',
    ]);

    const roles = root.children.find((child) => child.label === '"roles"');
    expect(roles?.valueType).toBe("array");
    expect(roles?.summary).toBe("1 item");
    expect(roles?.children[0].label).toBe("[0]");
    expect(roles?.children[0].summary).toBe('"admin"');

    const scalar = buildJsonTree("hello\nworld");
    expect(scalar.valueType).toBe("string");
    expect(scalar.childCount).toBe(0);
    expect(scalar.children).toEqual([]);
    expect(summarizeJsonNode(scalar)).toBe('"hello\\nworld"');
  });

  it("uses typed stable path keys without dot or index collisions", () => {
    const root = buildJsonTree({
      "0": "field-zero",
      "a.b": 1,
      "a\\b": 2,
      "arr[0]": 3,
      list: ["index-zero"],
    });

    const objectZero = root.children.find((child) => child.label === '"0"');
    const dotted = root.children.find((child) => child.label === '"a.b"');
    const slashed = root.children.find((child) => child.label === '"a\\\\b"');
    const bracketed = root.children.find((child) => child.label === '"arr[0]"');
    const list = root.children.find((child) => child.label === '"list"');
    const arrayZero = list?.children[0];
    const keys = [objectZero?.key, dotted?.key, slashed?.key, bracketed?.key, arrayZero?.key];

    expect(keys.every(Boolean)).toBe(true);
    expect(new Set(keys).size).toBe(keys.length);
    expect(objectZero?.path).toEqual(["0"]);
    expect(arrayZero?.path).toEqual(["list", 0]);
    expect(objectZero?.key).not.toBe(arrayZero?.key);
  });

  it("collects expandable keys and depth-limited expanded keys", () => {
    const root = buildJsonTree({
      user: {
        name: "张三",
        profile: { city: "杭州" },
      },
      roles: ["admin", "owner"],
      emptyObject: {},
      emptyArray: [],
    });
    const user = root.children.find((child) => child.label === '"user"')!;
    const profile = user.children.find((child) => child.label === '"profile"')!;
    const roles = root.children.find((child) => child.label === '"roles"')!;
    const emptyObject = root.children.find((child) => child.label === '"emptyObject"')!;
    const emptyArray = root.children.find((child) => child.label === '"emptyArray"')!;

    expect(isJsonTreeExpandable(root)).toBe(true);
    expect(isJsonTreeExpandable(emptyObject)).toBe(false);
    expect(isJsonTreeExpandable(emptyArray)).toBe(false);
    expect(emptyObject.summary).toBe("empty object");
    expect(emptyArray.summary).toBe("empty array");

    expect(collectExpandableKeys(root)).toEqual(new Set([root.key, user.key, profile.key, roles.key]));
    expect(collectExpandedKeysByDepth(root, "all")).toEqual(collectExpandableKeys(root));
    expect(collectExpandedKeysByDepth(root, 2)).toEqual(new Set([root.key, user.key, roles.key]));
  });

  it("renders circular references without recursive traversal", () => {
    const input: Record<string, unknown> = { name: "root" };
    input.self = input;

    const root = buildJsonTree(input);
    const self = root.children.find((child) => child.label === '"self"');

    expect(self?.summary).toBe("[Circular]");
    expect(self?.children).toEqual([]);
  });

  it("adds a max-depth guard node for deeply nested inputs", () => {
    let value: Record<string, unknown> = { leaf: "end" };
    for (let index = 0; index < 105; index += 1) {
      value = { next: value };
    }

    const root = buildJsonTree(value);

    expect(collectSummaries(root)).toContain("[Max depth reached]");
  });

  it("formats copy text safely for circular and non-JSON values", () => {
    const circular: Record<string, unknown> = { ok: true };
    circular.self = circular;
    const parsed = JSON.parse(
      formatJsonForCopy({
        circular,
        fn: () => "x",
        missing: undefined,
        nan: Number.NaN,
        symbol: Symbol("id"),
      }),
    );

    expect(parsed.circular.self).toBe("[Circular]");
    expect(parsed.fn).toContain("=>");
    expect(parsed.missing).toBe("undefined");
    expect(parsed.nan).toBe("NaN");
    expect(parsed.symbol).toBe("Symbol(id)");
  });

  it("renders JSONPath with dot access for identifier-safe field names", () => {
    expect(toJsonPath([])).toBe("$");
    expect(toJsonPath(["a", 0, "b"])).toBe("$.a[0].b");
    expect(toJsonPath(["$var", "_x", "y9"])).toBe("$.$var._x.y9");
  });

  it("renders JSONPath with escaped bracket access for unsafe field names", () => {
    expect(toJsonPath(["a.b"])).toBe('$["a.b"]');
    expect(toJsonPath(['he"llo'])).toBe('$["he\\"llo"]');
    expect(toJsonPath(["a\\b"])).toBe('$["a\\\\b"]');
    expect(toJsonPath(["0abc"])).toBe('$["0abc"]');
    expect(toJsonPath([""])).toBe('$[""]');
    expect(toJsonPath(["中文键", 2])).toBe('$["中文键"][2]');
  });
});
