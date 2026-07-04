import { describe, expect, it } from "vitest";
import { encodeJsonTreePath, formatJsonForCopy } from "./jsonTreeView";
import {
  applyJsonTreeEdit,
  defaultJsonValueForType,
  migrateExpandedKeys,
  parseLooseJsonInput,
} from "./jsonTreeEdit";
import type { JsonTreeEditOp } from "./jsonTreeEdit";

const enc = encodeJsonTreePath;

function expectOk(result: ReturnType<typeof applyJsonTreeEdit>): unknown {
  expect(result.ok).toBe(true);
  return result.ok ? result.value : undefined;
}

function expectFail(
  root: unknown,
  op: JsonTreeEditOp,
): string {
  const snapshot = formatJsonForCopy(root);
  const result = applyJsonTreeEdit(root, op);
  expect(result.ok).toBe(false);
  expect(formatJsonForCopy(root)).toBe(snapshot);
  return result.ok ? "" : result.reason;
}

describe("applyJsonTreeEdit set-value", () => {
  it("replaces nested values immutably with structural sharing", () => {
    const root = { a: { x: 1 }, b: { y: [1, 2] }, c: [{ z: 1 }] };
    const next = expectOk(
      applyJsonTreeEdit(root, { type: "set-value", path: ["b", "y", 0], value: 9 }),
    ) as typeof root;

    expect(next).toEqual({ a: { x: 1 }, b: { y: [9, 2] }, c: [{ z: 1 }] });
    expect(next).not.toBe(root);
    expect(next.b).not.toBe(root.b);
    expect(next.b.y).not.toBe(root.b.y);
    expect(next.a).toBe(root.a);
    expect(next.c).toBe(root.c);
    expect(root.b.y[0]).toBe(1);
  });

  it("replaces a scalar root entirely", () => {
    expect(expectOk(applyJsonTreeEdit("hi", { type: "set-value", path: [], value: 42 }))).toBe(42);
    expect(
      expectOk(applyJsonTreeEdit(null, { type: "set-value", path: [], value: { a: 1 } })),
    ).toEqual({ a: 1 });
  });

  it("fails on missing or type-mismatched paths without touching the document", () => {
    const root = { a: 1, list: [1] };
    expect(expectFail(root, { type: "set-value", path: ["missing", "x"], value: 1 })).toContain(
      "路径",
    );
    expect(expectFail(root, { type: "set-value", path: ["a", 0], value: 1 })).toContain("路径");
    expect(expectFail(root, { type: "set-value", path: ["list", 5], value: 1 })).toContain("路径");
    expect(expectFail(root, { type: "set-value", path: ["list", "0"], value: 1 })).toContain(
      "路径",
    );
  });

  it("fails on circular placeholder paths", () => {
    const root: Record<string, unknown> = { name: "r" };
    root.self = root;
    expect(expectFail(root, { type: "set-value", path: ["self"], value: 1 })).toContain("循环");
    expect(expectFail(root, { type: "set-value", path: ["self", "name"], value: 1 })).toContain(
      "循环",
    );
  });
});

describe("applyJsonTreeEdit rename-key", () => {
  it("renames an object field keeping key order and subtree reference", () => {
    const root = { first: { deep: true }, second: 2, third: 3 };
    const next = expectOk(
      applyJsonTreeEdit(root, { type: "rename-key", path: ["first"], newKey: "renamed" }),
    ) as Record<string, unknown>;

    expect(Object.keys(next)).toEqual(["renamed", "second", "third"]);
    expect(next.renamed).toBe(root.first);
  });

  it("fails when the new key already exists at the same level", () => {
    const root = { first: 1, second: 2 };
    expect(expectFail(root, { type: "rename-key", path: ["first"], newKey: "second" })).toContain(
      "已存在",
    );
  });

  it("fails for the root, array elements, and unchanged names", () => {
    const root = { list: [1], first: 1 };
    expect(expectFail(root, { type: "rename-key", path: [], newKey: "x" })).toBeTruthy();
    expect(expectFail(root, { type: "rename-key", path: ["list", 0], newKey: "x" })).toBeTruthy();
    expect(
      expectFail(root, { type: "rename-key", path: ["first"], newKey: "first" }),
    ).toBeTruthy();
  });
});

describe("applyJsonTreeEdit insert", () => {
  it("appends object fields at the end and inserts array items at an index", () => {
    const objRoot = { a: 1 };
    const objNext = expectOk(
      applyJsonTreeEdit(objRoot, { type: "insert", parentPath: [], key: "b", value: 2 }),
    ) as Record<string, unknown>;
    expect(Object.keys(objNext)).toEqual(["a", "b"]);

    const arrRoot = { list: [1, 3] };
    const arrNext = expectOk(
      applyJsonTreeEdit(arrRoot, { type: "insert", parentPath: ["list"], index: 1, value: 2 }),
    ) as typeof arrRoot;
    expect(arrNext.list).toEqual([1, 2, 3]);

    const appendNext = expectOk(
      applyJsonTreeEdit(arrRoot, { type: "insert", parentPath: ["list"], index: 2, value: 9 }),
    ) as typeof arrRoot;
    expect(appendNext.list).toEqual([1, 3, 9]);
  });

  it("allows inserting an empty-string key into an object without one", () => {
    const next = expectOk(
      applyJsonTreeEdit({}, { type: "insert", parentPath: [], key: "", value: null }),
    ) as Record<string, unknown>;
    expect(Object.prototype.hasOwnProperty.call(next, "")).toBe(true);
  });

  it("fails on duplicate keys including an existing empty-string key", () => {
    expect(
      expectFail({ a: 1 }, { type: "insert", parentPath: [], key: "a", value: 2 }),
    ).toContain("已存在");
    expect(
      expectFail({ "": 1 }, { type: "insert", parentPath: [], key: "", value: 2 }),
    ).toContain("已存在");
  });

  it("fails on missing key or index, out-of-range index, and non-container targets", () => {
    expect(expectFail({ a: 1 }, { type: "insert", parentPath: [], value: 2 })).toContain(
      "字段名",
    );
    expect(expectFail({ list: [1] }, { type: "insert", parentPath: ["list"], value: 2 })).toContain(
      "下标",
    );
    expect(
      expectFail({ list: [1] }, { type: "insert", parentPath: ["list"], index: 5, value: 2 }),
    ).toContain("越界");
    expect(
      expectFail({ list: [1] }, { type: "insert", parentPath: ["list"], index: -1, value: 2 }),
    ).toContain("越界");
    expect(
      expectFail({ a: 1 }, { type: "insert", parentPath: ["a"], key: "x", value: 2 }),
    ).toBeTruthy();
  });
});

describe("applyJsonTreeEdit remove", () => {
  it("removes object fields and array items immutably", () => {
    const root = { a: 1, b: 2, list: [1, 2, 3] };
    const objNext = expectOk(applyJsonTreeEdit(root, { type: "remove", path: ["a"] })) as Record<
      string,
      unknown
    >;
    expect(objNext).toEqual({ b: 2, list: [1, 2, 3] });
    expect(objNext.list).toBe(root.list);

    const arrNext = expectOk(
      applyJsonTreeEdit(root, { type: "remove", path: ["list", 1] }),
    ) as typeof root;
    expect(arrNext.list).toEqual([1, 3]);
  });

  it("fails for the root node", () => {
    expect(expectFail({ a: 1 }, { type: "remove", path: [] })).toContain("根");
  });
});

describe("applyJsonTreeEdit move", () => {
  it("swaps adjacent array items", () => {
    const root = { list: ["a", "b", "c"] };
    const next = expectOk(
      applyJsonTreeEdit(root, { type: "move", path: ["list", 0], offset: 1 }),
    ) as typeof root;
    expect(next.list).toEqual(["b", "a", "c"]);
  });

  it("rebuilds object key order when moving fields", () => {
    const root = { a: 1, b: 2, c: 3 };
    const next = expectOk(
      applyJsonTreeEdit(root, { type: "move", path: ["b"], offset: -1 }),
    ) as Record<string, unknown>;
    expect(Object.keys(next)).toEqual(["b", "a", "c"]);
  });

  it("fails at boundaries and for the root node", () => {
    const root = { list: [1, 2], a: 1 };
    expect(expectFail(root, { type: "move", path: ["list", 0], offset: -1 })).toContain("边界");
    expect(expectFail(root, { type: "move", path: ["list", 1], offset: 1 })).toContain("边界");
    expect(expectFail(root, { type: "move", path: ["list"], offset: -1 })).toContain("边界");
    expect(expectFail(root, { type: "move", path: ["a"], offset: 1 })).toContain("边界");
    expect(expectFail(root, { type: "move", path: [], offset: 1 })).toContain("根");
  });
});

describe("defaultJsonValueForType", () => {
  it("provides type-switch defaults with fresh container instances", () => {
    expect(defaultJsonValueForType("string")).toBe("");
    expect(defaultJsonValueForType("number")).toBe(0);
    expect(defaultJsonValueForType("boolean")).toBe(false);
    expect(defaultJsonValueForType("null")).toBeNull();
    expect(defaultJsonValueForType("object")).toEqual({});
    expect(defaultJsonValueForType("array")).toEqual([]);
    expect(defaultJsonValueForType("object")).not.toBe(defaultJsonValueForType("object"));
    expect(defaultJsonValueForType("array")).not.toBe(defaultJsonValueForType("array"));
  });
});

describe("parseLooseJsonInput", () => {
  it("uses strict JSON when parseable and falls back to the raw string", () => {
    expect(parseLooseJsonInput("42")).toBe(42);
    expect(parseLooseJsonInput('"42"')).toBe("42");
    expect(parseLooseJsonInput("true")).toBe(true);
    expect(parseLooseJsonInput("null")).toBeNull();
    expect(parseLooseJsonInput('{"a":1}')).toEqual({ a: 1 });
    expect(parseLooseJsonInput("[1,2]")).toEqual([1, 2]);
    expect(parseLooseJsonInput("hello world")).toBe("hello world");
    expect(parseLooseJsonInput("")).toBe("");
  });
});

describe("migrateExpandedKeys", () => {
  it("replaces prefixes for rename-key", () => {
    const keys = new Set([enc([]), enc(["user"]), enc(["user", "tags"]), enc(["other"])]);
    const next = migrateExpandedKeys(keys, {
      type: "rename-key",
      path: ["user"],
      newKey: "account",
    });
    expect(next).toEqual(
      new Set([enc([]), enc(["account"]), enc(["account", "tags"]), enc(["other"])]),
    );
  });

  it("shifts sibling indexes on array insert", () => {
    const keys = new Set([
      enc(["list"]),
      enc(["list", 0]),
      enc(["list", 1]),
      enc(["list", 1, "x"]),
      enc(["list", 10]),
    ]);
    const next = migrateExpandedKeys(keys, {
      type: "insert",
      parentPath: ["list"],
      index: 1,
      value: 0,
    });
    expect(next).toEqual(
      new Set([
        enc(["list"]),
        enc(["list", 0]),
        enc(["list", 2]),
        enc(["list", 2, "x"]),
        enc(["list", 11]),
      ]),
    );
  });

  it("drops the removed subtree and shifts later siblings on array remove", () => {
    const keys = new Set([
      enc(["list", 0]),
      enc(["list", 1]),
      enc(["list", 1, "x"]),
      enc(["list", 2]),
    ]);
    const next = migrateExpandedKeys(keys, { type: "remove", path: ["list", 1] });
    expect(next).toEqual(new Set([enc(["list", 0]), enc(["list", 1])]));
  });

  it("drops the removed subtree only on object remove", () => {
    const keys = new Set([enc(["a"]), enc(["a", "b"]), enc(["c"])]);
    const next = migrateExpandedKeys(keys, { type: "remove", path: ["a"] });
    expect(next).toEqual(new Set([enc(["c"])]));
  });

  it("swaps sibling prefixes on array move and keeps object move unchanged", () => {
    const keys = new Set([enc(["list", 0, "x"]), enc(["list", 1]), enc(["list", 2])]);
    const moved = migrateExpandedKeys(keys, { type: "move", path: ["list", 0], offset: 1 });
    expect(moved).toEqual(new Set([enc(["list", 1, "x"]), enc(["list", 0]), enc(["list", 2])]));

    const objKeys = new Set([enc(["a"]), enc(["b", "c"])]);
    expect(migrateExpandedKeys(objKeys, { type: "move", path: ["b"], offset: -1 })).toEqual(
      objKeys,
    );
  });

  it("keeps keys unchanged for set-value and object insert", () => {
    const keys = new Set([enc(["a"]), enc(["a", "b"])]);
    expect(
      migrateExpandedKeys(keys, { type: "set-value", path: ["a", "b"], value: 1 }),
    ).toEqual(keys);
    expect(
      migrateExpandedKeys(keys, { type: "insert", parentPath: ["a"], key: "z", value: 1 }),
    ).toEqual(keys);
  });
});
