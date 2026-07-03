import { describe, expect, it } from "vitest";
import { buildRedisKeyTree } from "./dbRedisKeyTree";
import type { RedisScanItem } from "../types/db";

function items(...keys: Array<[string, string]>): RedisScanItem[] {
  return keys.map(([key, type]) => ({ key, type }));
}

describe("buildRedisKeyTree", () => {
  it("按分隔符聚合成分组树，叶子带完整 key", () => {
    const tree = buildRedisKeyTree(
      items(
        ["user:1:name", "string"],
        ["user:1:tags", "set"],
        ["user:2:name", "string"],
        ["session:abc", "string"]
      )
    );
    expect(tree.map((n) => n.label)).toEqual(["session", "user"]);
    const user = tree[1];
    expect(user.count).toBe(3);
    expect(user.children.map((n) => n.label)).toEqual(["1", "2"]);
    const user1 = user.children[0];
    expect(user1.count).toBe(2);
    expect(user1.children.map((n) => n.key)).toEqual(["user:1:name", "user:1:tags"]);
    expect(user1.children[1].keyType).toBe("set");
  });

  it("无分隔符的 key 是根级叶子；分组在前叶子在后", () => {
    const tree = buildRedisKeyTree(items(["zzz", "string"], ["aa:bb", "hash"]));
    expect(tree.map((n) => n.label)).toEqual(["aa", "zzz"]);
    expect(tree[0].children[0].key).toBe("aa:bb");
    expect(tree[1].key).toBe("zzz");
  });

  it("同名分组与叶子共存", () => {
    const tree = buildRedisKeyTree(items(["a", "string"], ["a:b", "string"]));
    expect(tree).toHaveLength(2);
    expect(tree[0].label).toBe("a");
    expect(tree[0].children).toHaveLength(1);
    expect(tree[1].label).toBe("a");
    expect(tree[1].key).toBe("a");
  });

  it("空输入返回空树", () => {
    expect(buildRedisKeyTree([])).toEqual([]);
  });
});
