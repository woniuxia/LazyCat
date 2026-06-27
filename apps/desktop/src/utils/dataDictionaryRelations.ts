import type {
  DataDictionaryRelation,
  DataDictionaryRelationDraft,
  DataDictionarySummary,
} from "../types/data-dictionary";

export function duplicateRelationKeys(drafts: DataDictionaryRelationDraft[]): string[] {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const draft of drafts) {
    if (!draft.sourceFieldPath || !draft.targetDictionaryId) continue;
    const key = `${draft.sourceFieldPath}::${draft.targetDictionaryId}`;
    if (seen.has(key)) {
      duplicates.add(key);
    } else {
      seen.add(key);
    }
  }
  return Array.from(duplicates);
}

export function relationTargetPrimaryLabel(
  targetDictionaryId: number | null,
  dictionaries: DataDictionarySummary[],
): string {
  if (!targetDictionaryId) return "请选择目标字典";
  const target = dictionaries.find((dictionary) => dictionary.id === targetDictionaryId);
  if (!target?.primaryFieldPath) return "目标字典未配置主键";
  return `目标主键：${target.primaryFieldPath}`;
}

export function toRelationDrafts(
  relations: DataDictionaryRelation[],
): DataDictionaryRelationDraft[] {
  return relations.map((relation) => ({
    sourceFieldPath: relation.sourceFieldPath,
    targetDictionaryId: relation.targetDictionaryId,
    relationName: relation.relationName,
    reverseName: relation.reverseName,
  }));
}

export function hasInvalidRelationTarget(
  draft: DataDictionaryRelationDraft,
  dictionaries: DataDictionarySummary[],
): boolean {
  if (!draft.targetDictionaryId) return true;
  return !dictionaries.find((dictionary) => dictionary.id === draft.targetDictionaryId)
    ?.primaryFieldPath;
}
