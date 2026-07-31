import { invokeToolByChannel } from "../../bridge/tauri";
import { toPinyinInitials } from "../../utils/fuzzy-match";
import { writeSecretToClipboard, scheduleClipboardClear } from "../../utils/vaultClipboard";
import { registerProvider } from "../registry";
import type {
  ProviderDescriptor,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
} from "../types";

export interface VaultPlainFields {
  account?: string;
  url?: string;
  address?: string;
  port?: number;
  serverType?: string;
  dbType?: string;
  dbName?: string;
  schema?: string;
  notes?: string;
}

export interface VaultMetaEntry {
  id: number;
  category: string;
  title: string;
  environment: string;
  viewCount: number;
  copyCount: number;
  plainFields?: VaultPlainFields | null;
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

export function buildSubtitle(entry: VaultMetaEntry): string {
  const parts: string[] = [];
  const label = CATEGORY_LABEL[entry.category] ?? entry.category;
  if (label) parts.push(label);
  if (entry.environment) parts.push(entry.environment);
  const account = entry.plainFields?.account?.trim();
  if (account) parts.push(account);
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

export function buildItem(entry: VaultMetaEntry, unlocked: boolean): SpotlightItem {
  const tagsField = entry.tags?.length ? entry.tags.join(" ") : "";
  const subtitle = buildSubtitle(entry);
  const pf = entry.plainFields ?? undefined;
  const account = pf?.account ?? "";
  const searchFields = [
    makeField(entry.title, 1.2),
    makeField(account, 1.1),
    makeField(tagsField, 1.0),
    makeField(pf?.url ?? "", 0.8),
    makeField(pf?.address ?? "", 0.8),
    makeField(pf?.dbName ?? "", 0.8),
    makeField(pf?.schema ?? "", 0.8),
    makeField(entry.environment, 0.7),
    makeField(CATEGORY_LABEL[entry.category] ?? entry.category, 0.6),
    makeField(pf?.serverType ?? "", 0.6),
    makeField(pf?.dbType ?? "", 0.6),
    makeField(pf?.notes ?? "", 0.5),
  ].filter((f) => f.text);
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
    searchFields,
    ranking: {
      usageRef: {
        resourceType: "vault-entry",
        resourceId: String(entry.id),
        actions: ["reveal", "copy"],
      },
    },
    payload: {
      entryId: entry.id,
      category: entry.category,
      title: entry.title,
      account,
      unlocked,
      isLegacy: entry.plainFields === null,
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
  await writeSecretToClipboard(text);
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

async function copyPasswordFlow(
  item: SpotlightItem,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  const entryId = item.payload?.entryId as number | undefined;
  if (!entryId) return { errorMessage: "无效条目" };

  // 复用 vault 主面板的解锁状态：若全局 session 仍在有效期内，跳过主密码输入
  let unlocked = false;
  try {
    const status = (await invokeToolByChannel("tool:vault:status", {})) as VaultStatus | null;
    unlocked = !!status?.unlocked;
  } catch {
    /* 视为未解锁，走完整解锁流程 */
  }

  if (!unlocked) {
    const ok = await ctx.ensureVaultUnlocked(item.title);
    if (!ok) return { closeSpotlight: false };
  }

  // 此时 vault session 已解锁，使用 session key 直接解密
  let detail: VaultEntryDetail;
  try {
    detail = (await invokeToolByChannel("tool:vault:get", {
      id: entryId,
    })) as VaultEntryDetail;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (msg.includes("vault_locked")) {
      return { errorMessage: "密码库已锁定，请重新尝试" };
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
    toast: { message: "密码已复制到剪贴板（30 秒后自动清空）", type: "success" },
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
      id: "copy_account",
      label: "复制账号",
      icon: "copy",
    },
    {
      id: "open_vault",
      label: "跳转到凭据工具",
      icon: "external",
    },
  ];
}

async function copyAccountFlow(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const entryId = item.payload?.entryId as number | undefined;
  const account = typeof item.payload?.account === "string" ? item.payload.account : "";
  if (!account) {
    const isLegacy = item.payload?.isLegacy === true;
    return {
      errorMessage: isLegacy
        ? "该条目需先解锁一次完成迁移后才能复制账号"
        : "该条目没有填写账号信息",
    };
  }
  // 账号为明文索引字段，不按密级处理：不调度剪贴板自动清空
  await writeClipboard(account);
  if (entryId) await recordCopy(entryId);
  return {
    closeSpotlight: true,
    toast: { message: "账号已复制到剪贴板", type: "success" },
  };
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  if (actionId === "copy_password") return copyPasswordFlow(item, ctx);
  if (actionId === "copy_account") return copyAccountFlow(item);
  if (actionId === "open_vault") {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("spotlight_pick", { target: "vault" });
    return { closeSpotlight: true };
  }
  return { errorMessage: `未知动作 ${actionId}` };
}

export const vaultProvider: ProviderDescriptor = {
  id: "vault",
  name: "凭据",
  description: "密码库快速复制",
  badgeShort: "凭",
  badgeTone: "warn",
  weight: 0.9,
  defaultAliases: ["v", "vault"],
  defaultEnabled: true,
  prefetch: prefetchVault,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(vaultProvider);
