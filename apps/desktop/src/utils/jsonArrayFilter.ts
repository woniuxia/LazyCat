import { toJsonPath, type JsonTreePath } from "./jsonTreeView";

export type JsonObject = Record<string, unknown>;

export interface ObjectArrayTarget {
  path: string;
  value: JsonObject[];
}

function isJsonObject(value: unknown): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isObjectArray(value: unknown): value is JsonObject[] {
  return Array.isArray(value) && value.every(isJsonObject);
}

/** 按 JSON 文档顺序找到首个对象数组；空数组也属于可用目标。 */
export function findFirstObjectArray(value: unknown): ObjectArrayTarget | null {
  function visit(current: unknown, path: JsonTreePath): ObjectArrayTarget | null {
    if (Array.isArray(current)) {
      if (isObjectArray(current)) return { path: toJsonPath(path), value: current };

      for (let index = 0; index < current.length; index += 1) {
        const result = visit(current[index], [...path, index]);
        if (result) return result;
      }
      return null;
    }

    if (!isJsonObject(current)) return null;

    for (const [key, child] of Object.entries(current)) {
      const result = visit(child, [...path, key]);
      if (result) return result;
    }
    return null;
  }

  return visit(value, []);
}

/** 收集对象数组所有顶层属性，并保留首次出现顺序。 */
export function collectArrayProperties(value: readonly JsonObject[]): string[] {
  const properties = new Set<string>();
  for (const item of value) {
    for (const key of Object.keys(item)) properties.add(key);
  }
  return [...properties];
}

/** 按每个输入对象的原始 key 顺序构造不可变的字段投影。 */
export function projectObjectArray(
  value: readonly JsonObject[],
  selectedProperties: ReadonlySet<string>,
): JsonObject[] {
  return value.map((item) => {
    const projected: JsonObject = {};
    for (const [key, child] of Object.entries(item)) {
      if (!selectedProperties.has(key)) continue;
      Object.defineProperty(projected, key, {
        configurable: true,
        enumerable: true,
        value: child,
        writable: true,
      });
    }
    return projected;
  });
}
