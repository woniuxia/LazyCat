import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../../bridge/tauri";
import { createSearchField } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  DataDictionaryRecordDetail,
  DataDictionaryRecordSummaryPart,
  DataDictionarySearchItem,
  DataDictionarySearchResult,
} from "../../types/data-dictionary";
import type {
  ProviderDescriptor,
  SpotlightAction,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightSearchContext,
} from "../types";

interface DataDictionaryPayload {
  recordId: number;
  dictionaryId: number;
  summary: DataDictionaryRecordSummaryPart[];
}

function truncate(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

function payloadOf(item: SpotlightItem): DataDictionaryPayload | null {
  const recordId = item.payload?.recordId;
  const dictionaryId = item.payload?.dictionaryId;
  const summary = item.payload?.summary;
  if (typeof recordId !== "number" || typeof dictionaryId !== "number") return null;
  if (!Array.isArray(summary)) return null;
  return { recordId, dictionaryId, summary: summary as DataDictionaryRecordSummaryPart[] };
}

export function buildDataDictionaryItem(row: DataDictionarySearchItem): SpotlightItem {
  const summary = Array.isArray(row.summary) ? row.summary : [];
  const subtitleParts = [
    row.dictionaryName,
    ...summary.slice(0, 3).map((part) => `${part.label}：${part.value}`),
  ].filter(Boolean);
  const searchFields = [
    createSearchField(row.title, 1.2),
    createSearchField(row.dictionaryName, 0.8),
    ...summary.flatMap((part) => [
      createSearchField(part.label, 0.5),
      createSearchField(`${part.label} ${part.value}`, 0.9),
      createSearchField(part.value, 0.9),
    ]),
    ...row.matches.flatMap((match) => [
      createSearchField(match.fieldPath, 0.5),
      createSearchField(match.value, 0.9),
    ]),
  ].filter((field) => field.text);

  return {
    providerId: "data-dictionary",
    itemId: String(row.id),
    title: row.title || `${row.dictionaryName} #${row.rowIndex + 1}`,
    subtitle: truncate(subtitleParts.join(" · "), 96),
    badge: { short: "典", tone: "info" },
    status: summary.length > 0 ? { text: `${summary.length} 字段`, tone: "muted" } : undefined,
    searchFields,
    recallScore: row.recallScore,
    payload: {
      recordId: row.id,
      dictionaryId: row.dictionaryId,
      summary,
    },
  };
}

async function searchDataDictionary(
  query: string,
  context: SpotlightSearchContext,
): Promise<SpotlightItem[]> {
  const keyword = query.trim();
  if (!keyword) return [];
  if (context.signal.aborted) return [];
  const result = (await invokeToolByChannel("tool:data-dictionary:search", {
    scope: "all",
    keyword,
    limit: 50,
    includeRawJson: false,
  })) as DataDictionarySearchResult;
  if (context.signal.aborted) return [];
  if (!Array.isArray(result?.items)) throw new Error("数据字典搜索返回格式无效");
  return result.items.map((row) => buildDataDictionaryItem(row));
}

async function openRecord(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const payload = payloadOf(item);
  if (!payload) return { errorMessage: "无效数据字典记录" };
  await invoke("spotlight_pick", {
    target: "data-dictionary",
    itemId: String(payload.recordId),
  });
  return { closeSpotlight: true };
}

export function buildDataDictionaryActions(item: SpotlightItem): SpotlightAction[] {
  const payload = payloadOf(item);
  const summary = payload?.summary ?? [];
  return [
    ...summary.map((part, index) => ({
      id: `copy_field:${index}`,
      label: `复制${part.label}`,
      icon: "copy",
    })),
    {
      id: "copy_raw_json",
      label: "复制完整 JSON",
      icon: "copy",
    },
  ];
}

async function copySummaryField(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  const payload = payloadOf(item);
  if (!payload) return { errorMessage: "无效数据字典记录" };
  const indexText = actionId.slice("copy_field:".length);
  const index = Number(indexText);
  if (!Number.isInteger(index) || index < 0 || index >= payload.summary.length) {
    return { errorMessage: "字段不存在" };
  }
  try {
    await navigator.clipboard.writeText(payload.summary[index].value);
    return {
      closeSpotlight: true,
      toast: { message: "字段值已复制", type: "success" },
    };
  } catch {
    return { errorMessage: "复制字段值失败" };
  }
}

async function copyRawJson(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const payload = payloadOf(item);
  if (!payload) return { errorMessage: "无效数据字典记录" };
  try {
    const detail = (await invokeToolByChannel("tool:data-dictionary:record-detail", {
      recordId: payload.recordId,
    })) as DataDictionaryRecordDetail;
    const text = JSON.stringify(detail.record.rawJson, null, 2);
    if (!text) return { errorMessage: "复制 JSON 失败" };
    await navigator.clipboard.writeText(text);
    return {
      closeSpotlight: true,
      toast: { message: "完整 JSON 已复制", type: "success" },
    };
  } catch {
    return { errorMessage: "复制 JSON 失败" };
  }
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  if (actionId.startsWith("copy_field:")) return copySummaryField(item, actionId);
  if (actionId === "copy_raw_json") return copyRawJson(item);
  return { errorMessage: `未知动作 ${actionId}` };
}

export const dataDictionaryProvider: ProviderDescriptor = {
  id: "data-dictionary",
  name: "数据字典",
  description: "搜索数据字典记录",
  badgeShort: "典",
  badgeTone: "info",
  defaultAliases: ["dd", "dict"],
  defaultEnabled: true,
  prefetch: async () => [],
  search: searchDataDictionary,
  defaultAction: openRecord,
  buildActions: buildDataDictionaryActions,
  executeAction,
};

registerProvider(dataDictionaryProvider);
