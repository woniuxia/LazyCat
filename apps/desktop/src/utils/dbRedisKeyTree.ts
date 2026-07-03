/**
 * Redis key 树聚合纯函数：把 SCAN 累积到的扁平 key 列表按分隔符（默认 ":"）
 * 聚合成分组树，供 el-tree 渲染。叶子节点携带完整 key 与类型。
 */

import type { RedisScanItem } from "../types/db";

export interface RedisTreeNode {
  /** 节点显示名（段名；叶子为最后一段） */
  label: string;
  /** 叶子节点的完整 key */
  key?: string;
  /** 叶子节点的类型 */
  keyType?: string;
  /** 子孙叶子数量（分组节点） */
  count: number;
  children: RedisTreeNode[];
}

/**
 * 构建 key 树。相同前缀段聚合为分组节点；同名分组与叶子可共存
 * （例如同时存在 key `a` 与 `a:b`）。分组在前、叶子在后，各自按名称排序。
 */
export function buildRedisKeyTree(items: RedisScanItem[], delimiter = ":"): RedisTreeNode[] {
  interface Builder {
    groups: Map<string, Builder>;
    leaves: RedisTreeNode[];
  }
  const root: Builder = { groups: new Map(), leaves: [] };

  for (const item of items) {
    const segments = delimiter ? item.key.split(delimiter) : [item.key];
    let node = root;
    for (let i = 0; i < segments.length - 1; i++) {
      const seg = segments[i];
      if (!node.groups.has(seg)) {
        node.groups.set(seg, { groups: new Map(), leaves: [] });
      }
      node = node.groups.get(seg)!;
    }
    node.leaves.push({
      label: segments[segments.length - 1],
      key: item.key,
      keyType: item.type,
      count: 1,
      children: [],
    });
  }

  function materialize(builder: Builder): RedisTreeNode[] {
    const groups: RedisTreeNode[] = Array.from(builder.groups.entries())
      .map(([label, child]) => {
        const children = materialize(child);
        const count = children.reduce((sum, c) => sum + c.count, 0);
        return { label, count, children };
      })
      .sort((a, b) => a.label.localeCompare(b.label));
    const leaves = [...builder.leaves].sort((a, b) => a.label.localeCompare(b.label));
    return [...groups, ...leaves];
  }

  return materialize(root);
}
