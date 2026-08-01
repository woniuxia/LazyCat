/**
 * 递归收集 JSON 对象字段，再利用 replacer 的属性列表稳定控制各层输出顺序。
 * 数组索引不参与排序，数组元素仍按原顺序序列化。
 */
export function stringifyJsonWithSortedKeys(value: unknown, space = 2): string {
  const keys = new Set<string>();

  function collectKeys(current: unknown) {
    if (Array.isArray(current)) {
      current.forEach(collectKeys);
      return;
    }
    if (current === null || typeof current !== "object") return;

    for (const [key, child] of Object.entries(current)) {
      keys.add(key);
      collectKeys(child);
    }
  }

  collectKeys(value);
  return JSON.stringify(value, [...keys].sort(), space);
}
