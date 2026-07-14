const JAVA_IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

export function validateJavaQualifiedName(value: string): string {
  const normalized = value.trim();
  if (!normalized) return "完整类名不能为空";
  const invalid = normalized.split(".").find((part) => !JAVA_IDENTIFIER.test(part));
  return invalid ? `完整类名包含非法 Java 标识符：${invalid}` : "";
}

export function parseBaseClassFields(input: string): string[] {
  const result: string[] = [];
  const seen = new Set<string>();
  const fields = input
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);

  for (const field of fields) {
    if (!JAVA_IDENTIFIER.test(field)) {
      throw new Error(`非法 Java 字段名：${field}`);
    }
    if (!seen.has(field)) {
      seen.add(field);
      result.push(field);
    }
  }
  return result;
}

export function reconcileBaseClassSelection(
  selectedIds: number[],
  parentId: number | null,
  availableIds: number[],
): { selectedIds: number[]; parentId: number | null } {
  const available = new Set(availableIds);
  const nextSelected = selectedIds.filter((id) => available.has(id));
  if (nextSelected.length === 0) return { selectedIds: [], parentId: null };
  if (nextSelected.length === 1) {
    return { selectedIds: nextSelected, parentId: nextSelected[0] };
  }
  return {
    selectedIds: nextSelected,
    parentId: parentId !== null && nextSelected.includes(parentId) ? parentId : nextSelected[0],
  };
}
