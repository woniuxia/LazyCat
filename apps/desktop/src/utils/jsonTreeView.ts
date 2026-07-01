export type JsonTreeValueType =
  | "object"
  | "array"
  | "string"
  | "number"
  | "boolean"
  | "null"
  | "unknown";

export interface JsonTreeNode {
  key: string;
  path: Array<string | number>;
  depth: number;
  label: string;
  value: unknown;
  valueType: JsonTreeValueType;
  childCount: number;
  summary: string;
  children: JsonTreeNode[];
}

const ROOT_KEY = "$";
const MAX_TREE_DEPTH = 100;
const MAX_DEPTH_LABEL = "[Max depth reached]";
const CIRCULAR_LABEL = "[Circular]";

function getValueType(value: unknown): JsonTreeValueType {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";

  const type = typeof value;
  if (type === "string") return "string";
  if (type === "number") return "number";
  if (type === "boolean") return "boolean";
  if (type === "object") return "object";
  return "unknown";
}

export function encodeJsonTreePath(path: Array<string | number>): string {
  if (!path.length) return ROOT_KEY;
  return `${ROOT_KEY}/${path
    .map((segment) => {
      if (typeof segment === "number") return `i:${segment}`;
      return `k:${segment.length}:${segment}`;
    })
    .join("/")}`;
}

function safeString(value: unknown): string {
  try {
    return String(value);
  } catch {
    return "[Unserializable]";
  }
}

export function formatJsonPrimitive(value: unknown, valueType = getValueType(value)): string {
  if (valueType === "string") return JSON.stringify(value) ?? '""';
  if (valueType === "number") return Number.isFinite(value) ? String(value) : safeString(value);
  if (valueType === "boolean") return value ? "true" : "false";
  if (valueType === "null") return "null";
  return safeString(value);
}

export function summarizeJsonNode(node: JsonTreeNode): string {
  if (node.valueType === "object") return `object · ${node.childCount}`;
  if (node.valueType === "array") return `array · ${node.childCount}`;
  return formatJsonPrimitive(node.value, node.valueType);
}

function createNode(input: Omit<JsonTreeNode, "summary">): JsonTreeNode {
  const node: JsonTreeNode = { ...input, summary: "" };
  node.summary = summarizeJsonNode(node);
  return node;
}

function createPlaceholderNode(
  path: Array<string | number>,
  depth: number,
  label: string,
  value: string,
): JsonTreeNode {
  return createNode({
    key: encodeJsonTreePath(path),
    path,
    depth,
    label,
    value,
    valueType: "unknown",
    childCount: 0,
    children: [],
  });
}

function objectEntries(value: unknown): Array<[string | number, unknown]> {
  if (Array.isArray(value)) {
    return Array.from({ length: value.length }, (_, index) => [index, value[index]]);
  }
  return Object.entries(value as Record<string, unknown>);
}

function labelForSegment(segment: string | number): string {
  return typeof segment === "number" ? `[${segment}]` : JSON.stringify(segment);
}

function buildNode(
  value: unknown,
  path: Array<string | number>,
  depth: number,
  label: string,
  ancestors: WeakSet<object>,
): JsonTreeNode {
  const valueType = getValueType(value);
  if ((valueType === "object" || valueType === "array") && value && typeof value === "object") {
    if (ancestors.has(value)) {
      return createPlaceholderNode(path, depth, label, CIRCULAR_LABEL);
    }

    const entries = objectEntries(value);
    const childCount = entries.length;
    let children: JsonTreeNode[] = [];

    if (childCount > 0) {
      if (depth >= MAX_TREE_DEPTH) {
        const maxDepthPath = [...path, MAX_DEPTH_LABEL];
        children = [
          createPlaceholderNode(maxDepthPath, depth + 1, MAX_DEPTH_LABEL, MAX_DEPTH_LABEL),
        ];
      } else {
        ancestors.add(value);
        children = entries.map(([segment, childValue]) =>
          buildNode(childValue, [...path, segment], depth + 1, labelForSegment(segment), ancestors),
        );
        ancestors.delete(value);
      }
    }

    return createNode({
      key: encodeJsonTreePath(path),
      path,
      depth,
      label,
      value,
      valueType,
      childCount,
      children,
    });
  }

  return createNode({
    key: encodeJsonTreePath(path),
    path,
    depth,
    label,
    value,
    valueType,
    childCount: 0,
    children: [],
  });
}

export function buildJsonTree(value: unknown): JsonTreeNode {
  return buildNode(value, [], 0, "", new WeakSet<object>());
}

export function isJsonTreeExpandable(node: JsonTreeNode): boolean {
  return (node.valueType === "object" || node.valueType === "array") && node.childCount > 0;
}

export function collectExpandableKeys(root: JsonTreeNode): Set<string> {
  const keys = new Set<string>();
  const visit = (node: JsonTreeNode) => {
    if (isJsonTreeExpandable(node)) keys.add(node.key);
    for (const child of node.children) visit(child);
  };
  visit(root);
  return keys;
}

export function collectExpandedKeysByDepth(
  root: JsonTreeNode,
  depth: number | "all",
): Set<string> {
  if (depth === "all") return collectExpandableKeys(root);

  const keys = new Set<string>();
  const maxDepth = Math.max(0, depth);
  const visit = (node: JsonTreeNode) => {
    if (isJsonTreeExpandable(node) && node.depth < maxDepth) keys.add(node.key);
    for (const child of node.children) visit(child);
  };
  visit(root);
  return keys;
}

function normalizeJsonCopyValue(value: unknown): unknown {
  if (value === undefined) return "undefined";
  if (typeof value === "function") return safeString(value);
  if (typeof value === "symbol") return safeString(value);
  if (typeof value === "bigint") return safeString(value);
  if (typeof value === "number" && !Number.isFinite(value)) return safeString(value);
  return value;
}

export function formatJsonForCopy(value: unknown): string {
  const seen = new WeakSet<object>();
  const normalizedRoot = normalizeJsonCopyValue(value);

  try {
    const text = JSON.stringify(
      normalizedRoot,
      (_key, item) => {
        const normalized = normalizeJsonCopyValue(item);
        if (normalized !== item) return normalized;
        if (item && typeof item === "object") {
          if (seen.has(item)) return CIRCULAR_LABEL;
          seen.add(item);
        }
        return item;
      },
      2,
    );
    return text ?? safeString(value);
  } catch {
    return safeString(value);
  }
}
