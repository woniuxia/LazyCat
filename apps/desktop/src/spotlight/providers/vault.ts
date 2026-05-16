import { invokeToolByChannel } from "../../bridge/tauri";
import { toPinyinInitials } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightProvider,
} from "../types";

interface VaultMetaEntry {
  id: number;
  category: string;
  title: string;
  environment: string;
  viewCount: number;
  copyCount: number;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

interface VaultStatus {
  setup: boolean;
  unlocked: boolean;
  lockState: "unlocked" | "locked";
}

interface VaultEntryDetail {
  id: number;
  category: string;
  title: string;
  environment: string;
  fields: Record<string, unknown>;
  tags: string[];
}

const CATEGORY_LABEL: Record<string, string> = {
  app: "应用",
  server: "服务器",
  database: "数据库",
};

async function loadStatus(): Promise<VaultStatus | null> {
  try {
    return ((await invokeToolByChannel("tool:vault:status", {})) as VaultStatus | null) ?? null;
  } catch {
    return null;
  }
}

async function loadMeta(): Promise<VaultMetaEntry[]> {
  try {
    const list = (await invokeToolByChannel("tool:vault:meta-list", {})) as VaultMetaEntry[];
    return Array.isArray(list) ? list : [];
  } catch {
    return [];
  }
}

function buildSubtitle(entry: VaultMetaEntry): string {
  const parts: string[] = [];
  const label = CATEGORY_LABEL[entry.category] ?? entry.category;
  if (label) parts.push(label);
  if (entry.environment) parts.push(entry.environment);
  if (entry.tags?.length) parts.push(entry.tags.map((t) => `#${t}`).join(" "));
  return parts.join(" · ");
}

function makeField(text: string, weight: number) {
  const cleaned = text.trim();
  return {
    text: cleaned,
    initials: toPinyinInitials(cleaned),
    weight,
  };
}

function buildItem(entry: VaultMetaEntry, unlocked: boolean): SpotlightItem {
  const tagsField = entry.tags?.length ? entry.tags.join(" ") : "";
  const subtitle = buildSubtitle(entry);
  const usage = (entry.viewCount ?? 0) + (entry.copyCount ?? 0);
  const weight = 1 + Math.min(usage, 50) * 0.01;
  return {
    providerId: "vault",
    itemId: String(entry.id),
    title: entry.title || "(无标题)",
    subtitle,
    badge: { short: "凭", tone: "warn" },
    status: {
      text: unlocked ? "解锁" : "需主密码",
      tone: unlocked ? "success" : "muted",
    },
    searchFields: [
      makeField(entry.title, 1.2),
      makeField(tagsField, 1.0),
      makeField(CATEGORY_LABEL[entry.category] ?? entry.category, 0.6),
      makeField(entry.environment, 0.7),
    ],
    weight,
    payload: {
      entryId: entry.id,
      category: entry.category,
      title: entry.title,
      unlocked,
    },
  };
}

async function prefetchVault(): Promise<SpotlightItem[]> {
  const status = await loadStatus();
  if (!status?.setup) return [];
  const list = await loadMeta();
  const unlocked = !!status.unlocked;
  return list.map((entry) => buildItem(entry, unlocked));
}

async function writeClipboard(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

async function recordCopy(entryId: number): Promise<void> {
  try {
    await invokeToolByChannel("tool:vault:record-usage", { id: entryId, type: "copy" });
  } catch {
    // 不阻断主流程
  }
}

function pickPassword(detail: VaultEntryDetail): string {
  const value = (detail.fields?.["password"] as string | undefined) ?? "";
  return value;
}

const CLIPBOARD_CLEAR_DELAY_MS = 30_000;

function scheduleClipboardClear(secret: string): void {
  if (typeof window === "undefined") return;
  window.setTimeout(async () => {
    try {
      const current = await navigator.clipboard.readText().catch(() => "");
      if (current === secret) {
        await navigator.clipboard.writeText("");
      }
    } catch {
      // 忽略
    }
  }, CLIPBOARD_CLEAR_DELAY_MS);
}

async function copyPasswordFlow(
  item: SpotlightItem,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  const entryId = item.payload?.entryId as number | undefined;
  if (!entryId) return { errorMessage: "无效条目" };

  const password = await ctx.requestMasterPassword(item.title);
  if (password == null) return { closeSpotlight: false };

  let detail: VaultEntryDetail;
  try {
    detail = (await invokeToolByChannel("tool:vault:reveal-one", {
      id: entryId,
      masterPassword: password,
    })) as VaultEntryDetail;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (msg.includes("too_many_attempts")) {
      return { errorMessage: "尝试次数过多，请稍后再试" };
    }
    if (msg.includes("bad_master_password")) {
      return { errorMessage: "主密码错误" };
    }
    return { errorMessage: msg };
  }

  const secret = pickPassword(detail);
  if (!secret) return { errorMessage: "该条目没有密码字段" };

  await writeClipboard(secret);
  scheduleClipboardClear(secret);
  await recordCopy(entryId);

  return {
    closeSpotlight: true,
    toast: { message: "密码已复制，30 秒后自动清空", type: "success" },
  };
}

async function defaultAction(
  item: SpotlightItem,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  return copyPasswordFlow(item, ctx);
}

function buildActions() {
  return [
    {
      id: "copy_password",
      label: "复制密码",
      icon: "lock",
      needsMasterPassword: true,
      shortcut: "Enter",
    },
    {
      id: "open_vault",
      label: "跳转到凭据工具",
      icon: "external",
    },
  ];
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  if (actionId === "copy_password") return copyPasswordFlow(item, ctx);
  if (actionId === "open_vault") {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("spotlight_pick", { target: "vault" });
    return { closeSpotlight: true };
  }
  return { errorMessage: `未知动作 ${actionId}` };
}

export const vaultProvider: SpotlightProvider = {
  id: "vault",
  scopeKeys: ["v", "vault"],
  badgeShort: "凭",
  badgeTone: "warn",
  weight: 0.9,
  prefetch: prefetchVault,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(vaultProvider);
