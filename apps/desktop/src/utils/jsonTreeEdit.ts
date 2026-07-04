import { toRaw } from "vue";
import type { JsonTreePath } from "./jsonTreeView";
import { encodeJsonTreePath } from "./jsonTreeView";

export type JsonTreeEditOp =
  | { type: "set-value"; path: JsonTreePath; value: unknown }
  | { type: "rename-key"; path: JsonTreePath; newKey: string }
  | { type: "insert"; parentPath: JsonTreePath; key?: string; index?: number; value: unknown }
  | { type: "remove"; path: JsonTreePath }
  | { type: "move"; path: JsonTreePath; offset: -1 | 1 };

export type JsonTreeEditResult = { ok: true; value: unknown } | { ok: false; reason: string };

export type JsonTreeSwitchableType = "string" | "number" | "boolean" | "null" | "object" | "array";

/** 类型切换缺省值;容器每次返回全新实例。 */
export function defaultJsonValueForType(type: JsonTreeSwitchableType): unknown {
  switch (type) {
    case "string":
      return "";
    case "number":
      return 0;
    case "boolean":
      return false;
    case "null":
      return null;
    case "object":
      return {};
    case "array":
      return [];
  }
}

type JsonContainer = Record<string, unknown> | unknown[];

interface PathStep {
  container: JsonContainer;
  segment: string | number;
}

type ResolveResult =
  | { ok: true; chain: PathStep[]; target: unknown }
  | { ok: false; reason: string };

function fail(reason: string): { ok: false; reason: string } {
  return { ok: false, reason };
}

function isPlainObjectValue(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeRaw(value: unknown): unknown {
  return typeof value === "object" && value !== null ? toRaw(value) : value;
}

/**
 * 沿路径逐段校验并收集祖先容器;命中循环引用(占位节点路径)时失败。
 */
function resolvePath(root: unknown, path: JsonTreePath): ResolveResult {
  const chain: PathStep[] = [];
  const seen = new WeakSet<object>();
  let current = normalizeRaw(root);

  for (const segment of path) {
    if (typeof current === "object" && current !== null) {
      if (seen.has(current)) return fail("循环引用节点不支持编辑");
      seen.add(current);
    }
    if (typeof segment === "number") {
      if (!Array.isArray(current)) return fail("路径类型不匹配:期望数组");
      if (!Number.isInteger(segment) || segment < 0 || segment >= current.length) {
        return fail("路径不存在:下标越界");
      }
      chain.push({ container: current, segment });
      current = normalizeRaw(current[segment]);
    } else {
      if (!isPlainObjectValue(current)) return fail("路径类型不匹配:期望对象");
      if (!Object.prototype.hasOwnProperty.call(current, segment)) return fail("路径不存在");
      chain.push({ container: current, segment });
      current = normalizeRaw(current[segment]);
    }
  }

  if (typeof current === "object" && current !== null && seen.has(current)) {
    return fail("循环引用节点不支持编辑");
  }
  return { ok: true, chain, target: current };
}

function cloneContainer(container: JsonContainer): JsonContainer {
  return Array.isArray(container) ? container.slice() : { ...container };
}

/** 自底向上沿链浅克隆:把 chain 末端 segment 指向 child,返回新根。 */
function rebuildThroughChain(chain: PathStep[], child: unknown): unknown {
  let next = child;
  for (let index = chain.length - 1; index >= 0; index -= 1) {
    const { container, segment } = chain[index];
    const clone = cloneContainer(container);
    (clone as Record<string | number, unknown>)[segment] = next;
    next = clone;
  }
  return next;
}

function applySetValue(root: unknown, path: JsonTreePath, value: unknown): JsonTreeEditResult {
  if (!path.length) return { ok: true, value };
  const resolved = resolvePath(root, path);
  if (!resolved.ok) return resolved;
  return { ok: true, value: rebuildThroughChain(resolved.chain, value) };
}

function applyRenameKey(root: unknown, path: JsonTreePath, newKey: string): JsonTreeEditResult {
  if (!path.length) return fail("根节点不支持重命名");
  const lastSegment = path[path.length - 1];
  if (typeof lastSegment !== "string") return fail("数组元素不支持重命名");

  const resolved = resolvePath(root, path);
  if (!resolved.ok) return resolved;
  if (newKey === lastSegment) return fail("键名未变化");

  const parent = resolved.chain[resolved.chain.length - 1].container as Record<string, unknown>;
  if (Object.prototype.hasOwnProperty.call(parent, newKey)) {
    return fail("同级已存在同名 key");
  }

  const renamed: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(parent)) {
    renamed[key === lastSegment ? newKey : key] = value;
  }
  return { ok: true, value: rebuildThroughChain(resolved.chain.slice(0, -1), renamed) };
}

function applyInsert(
  root: unknown,
  parentPath: JsonTreePath,
  key: string | undefined,
  index: number | undefined,
  value: unknown,
): JsonTreeEditResult {
  const resolved = resolvePath(root, parentPath);
  if (!resolved.ok) return resolved;

  const target = resolved.target;
  if (Array.isArray(target)) {
    if (index === undefined) return fail("缺少插入下标");
    if (!Number.isInteger(index) || index < 0 || index > target.length) {
      return fail("插入下标越界");
    }
    const inserted = target.slice();
    inserted.splice(index, 0, value);
    return { ok: true, value: rebuildThroughChain(resolved.chain, inserted) };
  }
  if (isPlainObjectValue(target)) {
    if (key === undefined) return fail("缺少字段名");
    if (Object.prototype.hasOwnProperty.call(target, key)) return fail("已存在同名 key");
    const inserted = { ...target, [key]: value };
    return { ok: true, value: rebuildThroughChain(resolved.chain, inserted) };
  }
  return fail("插入目标不是容器");
}

function applyRemove(root: unknown, path: JsonTreePath): JsonTreeEditResult {
  if (!path.length) return fail("不能删除根节点");
  const resolved = resolvePath(root, path);
  if (!resolved.ok) return resolved;

  const { container, segment } = resolved.chain[resolved.chain.length - 1];
  let removed: JsonContainer;
  if (Array.isArray(container)) {
    removed = container.slice();
    (removed as unknown[]).splice(segment as number, 1);
  } else {
    removed = {};
    for (const [key, value] of Object.entries(container)) {
      if (key !== segment) (removed as Record<string, unknown>)[key] = value;
    }
  }
  return { ok: true, value: rebuildThroughChain(resolved.chain.slice(0, -1), removed) };
}

function applyMove(root: unknown, path: JsonTreePath, offset: -1 | 1): JsonTreeEditResult {
  if (!path.length) return fail("不能移动根节点");
  const resolved = resolvePath(root, path);
  if (!resolved.ok) return resolved;

  const { container, segment } = resolved.chain[resolved.chain.length - 1];
  let moved: JsonContainer;
  if (Array.isArray(container)) {
    const from = segment as number;
    const to = from + offset;
    if (to < 0 || to >= container.length) return fail("已到达边界");
    const swapped = container.slice();
    [swapped[from], swapped[to]] = [swapped[to], swapped[from]];
    moved = swapped;
  } else {
    const entries = Object.entries(container);
    const from = entries.findIndex(([key]) => key === segment);
    const to = from + offset;
    if (to < 0 || to >= entries.length) return fail("已到达边界");
    [entries[from], entries[to]] = [entries[to], entries[from]];
    const swapped: Record<string, unknown> = {};
    for (const [key, value] of entries) swapped[key] = value;
    moved = swapped;
  }
  return { ok: true, value: rebuildThroughChain(resolved.chain.slice(0, -1), moved) };
}

/**
 * 不可变编辑:只沿目标路径浅克隆祖先容器,其余结构共享;
 * 失败返回 { ok: false, reason } 且不修改文档。
 */
export function applyJsonTreeEdit(root: unknown, op: JsonTreeEditOp): JsonTreeEditResult {
  const rawRoot = normalizeRaw(root);
  switch (op.type) {
    case "set-value":
      return applySetValue(rawRoot, op.path, op.value);
    case "rename-key":
      return applyRenameKey(rawRoot, op.path, op.newKey);
    case "insert":
      return applyInsert(rawRoot, op.parentPath, op.key, op.index, op.value);
    case "remove":
      return applyRemove(rawRoot, op.path);
    case "move":
      return applyMove(rawRoot, op.path, op.offset);
  }
}

interface ArrayChildKeyParts {
  index: number;
  rest: string;
}

/** 解析 base 直属数组子节点 key:返回第一段下标与剩余后缀。 */
function splitArrayChildKey(key: string, base: string): ArrayChildKeyParts | null {
  const marker = `${base}i:`;
  if (!key.startsWith(marker)) return null;
  const tail = key.slice(marker.length);
  const slash = tail.indexOf("/");
  const indexText = slash === -1 ? tail : tail.slice(0, slash);
  const rest = slash === -1 ? "" : tail.slice(slash);
  const index = Number(indexText);
  if (!Number.isInteger(index) || index < 0) return null;
  return { index, rest };
}

/**
 * 编辑 op 成功后迁移展开 key 集合:
 * rename 前缀替换;数组 insert/remove 兄弟平移(remove 丢弃被删子树);
 * 数组 move 交换前缀;set-value 与对象 insert/move 不变。
 */
export function migrateExpandedKeys(keys: Set<string>, op: JsonTreeEditOp): Set<string> {
  switch (op.type) {
    case "set-value":
      return new Set(keys);
    case "rename-key": {
      const lastSegment = op.path[op.path.length - 1];
      if (!op.path.length || typeof lastSegment !== "string") return new Set(keys);
      const oldPrefix = encodeJsonTreePath(op.path);
      const newPrefix = encodeJsonTreePath([...op.path.slice(0, -1), op.newKey]);
      return new Set(
        [...keys].map((key) => {
          if (key === oldPrefix) return newPrefix;
          if (key.startsWith(`${oldPrefix}/`)) return newPrefix + key.slice(oldPrefix.length);
          return key;
        }),
      );
    }
    case "insert": {
      // 对象 insert 不改既有路径;数组 insert 以 index 判别
      if (op.index === undefined) return new Set(keys);
      const insertAt = op.index;
      const base = `${encodeJsonTreePath(op.parentPath)}/`;
      return new Set(
        [...keys].map((key) => {
          const parts = splitArrayChildKey(key, base);
          if (!parts || parts.index < insertAt) return key;
          return `${base}i:${parts.index + 1}${parts.rest}`;
        }),
      );
    }
    case "remove": {
      if (!op.path.length) return new Set(keys);
      const removedPrefix = encodeJsonTreePath(op.path);
      const kept = [...keys].filter(
        (key) => key !== removedPrefix && !key.startsWith(`${removedPrefix}/`),
      );
      const lastSegment = op.path[op.path.length - 1];
      if (typeof lastSegment !== "number") return new Set(kept);
      const base = `${encodeJsonTreePath(op.path.slice(0, -1))}/`;
      return new Set(
        kept.map((key) => {
          const parts = splitArrayChildKey(key, base);
          if (!parts || parts.index <= lastSegment) return key;
          return `${base}i:${parts.index - 1}${parts.rest}`;
        }),
      );
    }
    case "move": {
      const lastSegment = op.path[op.path.length - 1];
      if (!op.path.length || typeof lastSegment !== "number") return new Set(keys);
      const from = lastSegment;
      const to = from + op.offset;
      const base = `${encodeJsonTreePath(op.path.slice(0, -1))}/`;
      return new Set(
        [...keys].map((key) => {
          const parts = splitArrayChildKey(key, base);
          if (!parts) return key;
          if (parts.index === from) return `${base}i:${to}${parts.rest}`;
          if (parts.index === to) return `${base}i:${from}${parts.rest}`;
          return key;
        }),
      );
    }
  }
}
