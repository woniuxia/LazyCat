import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../bridge/tauri";
import {
  writeSecretToClipboard,
  scheduleClipboardClear,
} from "../utils/vaultClipboard";
import type {
  KeywordCommandDescriptor,
  KeywordCommandInvocation,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
} from "./types";

// ── 内联渲染项的 payload 形状(供 SpotlightPanel.executeKeywordItem 派发)
//
// keyword 模式下,每条 SpotlightItem 的 payload.keywordItemKind 标识其执行路径,
// 渲染层无需深入了解 producer / provider 细节。

interface KeywordItemPayloadBase {
  __keyword: true;
}

interface KeywordItemShowValue extends KeywordItemPayloadBase {
  keywordItemKind: "show-value";
  value: string;
}

interface KeywordItemOpenTool extends KeywordItemPayloadBase {
  keywordItemKind: "open-tool";
  toolId: string;
  text: string;
}

interface KeywordItemVaultEntry extends KeywordItemPayloadBase {
  keywordItemKind: "vault-entry";
  entryId: number;
  title: string;
  unlocked: boolean;
}

interface KeywordItemSnippetEntry extends KeywordItemPayloadBase {
  keywordItemKind: "snippet-entry";
  entryId: number;
  title: string;
  defaultCode: string;
}

interface KeywordItemJumpTool extends KeywordItemPayloadBase {
  keywordItemKind: "jump-tool";
  toolId: string;
  toastOnJump?: string;
}

interface KeywordItemHint extends KeywordItemPayloadBase {
  keywordItemKind: "hint";
}

export type KeywordItemPayload =
  | KeywordItemShowValue
  | KeywordItemOpenTool
  | KeywordItemVaultEntry
  | KeywordItemSnippetEntry
  | KeywordItemJumpTool
  | KeywordItemHint;

// ── value producers(show-value 类的取数实现) ─────────────────────────

interface LocalIpsResponse {
  interfaces: Array<{ name: string; ipv4: string[]; ipv6: string[] }>;
}

async function produceLocalIp(): Promise<SpotlightItem[]> {
  try {
    const raw = (await invokeToolByChannel("tool:system:local-ips", {})) as LocalIpsResponse;
    const items: SpotlightItem[] = [];
    for (const iface of raw?.interfaces ?? []) {
      for (const ipv4 of iface.ipv4 ?? []) {
        items.push(
          buildShowValueItem({
            id: `ip:v4:${iface.name}:${ipv4}`,
            title: ipv4,
            subtitle: `${iface.name} · IPv4`,
            value: ipv4,
            badgeShort: "IP",
            badgeTone: "info",
          }),
        );
      }
      for (const ipv6 of iface.ipv6 ?? []) {
        items.push(
          buildShowValueItem({
            id: `ip:v6:${iface.name}:${ipv6}`,
            title: ipv6,
            subtitle: `${iface.name} · IPv6`,
            value: ipv6,
            badgeShort: "IP",
            badgeTone: "info",
          }),
        );
      }
    }
    if (items.length === 0) {
      return [buildHintItem("local-ip-empty", "未检测到可用网卡", "请检查网络连接")];
    }
    return items;
  } catch (err) {
    return [
      buildHintItem(
        "local-ip-error",
        "网卡读取失败",
        err instanceof Error ? err.message : String(err),
      ),
    ];
  }
}

function produceUuid(): SpotlightItem[] {
  const items: SpotlightItem[] = [];
  for (let i = 0; i < 5; i += 1) {
    const id =
      typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
        ? crypto.randomUUID()
        : fallbackUuid();
    items.push(
      buildShowValueItem({
        id: `uuid:${i}:${id}`,
        title: id,
        subtitle: `UUID v4 · #${i + 1}`,
        value: id,
        badgeShort: "UUID",
        badgeTone: "primary",
      }),
    );
  }
  return items;
}

function fallbackUuid(): string {
  const hex = (n: number) =>
    Math.floor(Math.random() * 16 ** n)
      .toString(16)
      .padStart(n, "0");
  return `${hex(8)}-${hex(4)}-4${hex(3)}-${(
    8 +
    Math.floor(Math.random() * 4)
  ).toString(16)}${hex(3)}-${hex(12)}`;
}

function produceTimestamp(): SpotlightItem[] {
  const now = new Date();
  const seconds = Math.floor(now.getTime() / 1000);
  const millis = now.getTime();
  const iso = now.toISOString();
  const rfc = iso.replace("Z", "+00:00");
  const local = now.toLocaleString();
  return [
    buildShowValueItem({
      id: `ts:s:${seconds}`,
      title: String(seconds),
      subtitle: "Unix 秒",
      value: String(seconds),
      badgeShort: "时",
      badgeTone: "info",
    }),
    buildShowValueItem({
      id: `ts:ms:${millis}`,
      title: String(millis),
      subtitle: "Unix 毫秒",
      value: String(millis),
      badgeShort: "时",
      badgeTone: "info",
    }),
    buildShowValueItem({
      id: `ts:iso:${iso}`,
      title: iso,
      subtitle: "ISO 8601",
      value: iso,
      badgeShort: "时",
      badgeTone: "info",
    }),
    buildShowValueItem({
      id: `ts:rfc:${rfc}`,
      title: rfc,
      subtitle: "RFC 3339",
      value: rfc,
      badgeShort: "时",
      badgeTone: "info",
    }),
    buildShowValueItem({
      id: `ts:local:${local}`,
      title: local,
      subtitle: "本地友好",
      value: local,
      badgeShort: "时",
      badgeTone: "info",
    }),
  ];
}

async function produceHash(text: string): Promise<SpotlightItem[]> {
  if (!text) {
    return [buildHintItem("hash-empty", ";hash <文本>", "请在 ;hash 后输入要计算哈希的文本")];
  }
  const items: SpotlightItem[] = [];
  for (const [channel, label, short] of [
    ["tool:encode:md5", "MD5", "MD5"],
    ["tool:encode:sha1", "SHA-1", "SHA1"],
    ["tool:encode:sha256", "SHA-256", "SHA256"],
  ] as const) {
    try {
      const raw = (await invokeToolByChannel(channel, { input: text })) as
        | string
        | { result?: string }
        | null;
      const value =
        typeof raw === "string" ? raw : (raw?.result as string | undefined) ?? "";
      if (!value) {
        items.push(buildHintItem(`hash-${label}-empty`, label, "未返回结果"));
        continue;
      }
      items.push(
        buildShowValueItem({
          id: `hash:${label}:${value}`,
          title: value,
          subtitle: `${label} · ${truncatePreview(text, 24)}`,
          value,
          badgeShort: short,
          badgeTone: "primary",
        }),
      );
    } catch (err) {
      items.push(
        buildHintItem(
          `hash-${label}-error`,
          `${label} 计算失败`,
          err instanceof Error ? err.message : String(err),
        ),
      );
    }
  }
  return items;
}

// ── vault-tag / snippet-tag ────────────────────────────────────────────

interface VaultMetaEntry {
  id: number;
  category: string;
  title: string;
  environment: string;
  tags: string[];
}

interface VaultStatus {
  setup: boolean;
  unlocked: boolean;
}

async function produceVaultTag(tag: string, filterArg: string): Promise<SpotlightItem[]> {
  if (!tag) {
    return [buildHintItem("vault-tag-empty", "未配置 tag", "前往设置补全 vault-tag 关键字")];
  }
  let status: VaultStatus | null;
  try {
    status = ((await invokeToolByChannel("tool:vault:status", {})) as VaultStatus) ?? null;
  } catch {
    status = null;
  }
  if (!status?.setup) {
    return [
      buildJumpToolItem(
        "vault-not-setup",
        "凭据库未初始化",
        "Enter 跳转到凭据工具",
        "vault",
      ),
    ];
  }
  let list: VaultMetaEntry[];
  try {
    const raw = (await invokeToolByChannel("tool:vault:meta-list", {})) as VaultMetaEntry[];
    list = Array.isArray(raw) ? raw : [];
  } catch (err) {
    return [
      buildHintItem(
        "vault-tag-error",
        "凭据列表加载失败",
        err instanceof Error ? err.message : String(err),
      ),
    ];
  }
  const target = tag.toLowerCase();
  let filtered = list.filter((entry) =>
    (entry.tags ?? []).some((t) => t.toLowerCase() === target),
  );
  if (filterArg) {
    const lower = filterArg.toLowerCase();
    filtered = filtered.filter((entry) =>
      entry.title.toLowerCase().includes(lower),
    );
  }
  if (filtered.length === 0) {
    return [
      buildJumpToolItem(
        `vault-tag-empty:${tag}`,
        `未找到含 tag "${tag}" 的凭据`,
        "Enter 跳转到凭据工具",
        "vault",
      ),
    ];
  }
  const unlocked = !!status.unlocked;
  return filtered.map<SpotlightItem>((entry) => {
    const payload: KeywordItemVaultEntry = {
      __keyword: true,
      keywordItemKind: "vault-entry",
      entryId: entry.id,
      title: entry.title,
      unlocked,
    };
    return {
      providerId: "__keyword__",
      itemId: `kw-vault:${entry.id}`,
      title: entry.title || "(无标题)",
      subtitle: `凭据 · #${tag}${entry.environment ? ` · ${entry.environment}` : ""}`,
      badge: { short: "凭", tone: "warn" },
      status: unlocked
        ? { text: "解锁", tone: "success" }
        : { text: "需主密码", tone: "muted" },
      searchFields: [],
      payload: payload as unknown as Record<string, unknown>,
    };
  });
}

interface SnippetEntry {
  id: number;
  title: string;
  description: string;
  tags: string[];
  primaryLanguage: string;
}

async function produceSnippetTag(tag: string, filterArg: string): Promise<SpotlightItem[]> {
  if (!tag) {
    return [
      buildHintItem("snippet-tag-empty", "未配置 tag", "前往设置补全 snippet-tag 关键字"),
    ];
  }
  let list: SnippetEntry[];
  try {
    const raw = (await invokeToolByChannel("tool:snippets:v2:list", { tag })) as
      | SnippetEntry[]
      | null;
    list = Array.isArray(raw) ? raw : [];
  } catch (err) {
    return [
      buildHintItem(
        "snippet-tag-error",
        "片段列表加载失败",
        err instanceof Error ? err.message : String(err),
      ),
    ];
  }
  if (filterArg) {
    const lower = filterArg.toLowerCase();
    list = list.filter(
      (e) =>
        e.title.toLowerCase().includes(lower) ||
        e.description.toLowerCase().includes(lower),
    );
  }
  if (list.length === 0) {
    return [
      buildJumpToolItem(
        `snippet-tag-empty:${tag}`,
        `未找到含 tag "${tag}" 的片段`,
        "Enter 跳转到代码片段工具",
        "snippets",
      ),
    ];
  }
  return list.map<SpotlightItem>((entry) => {
    const payload: KeywordItemSnippetEntry = {
      __keyword: true,
      keywordItemKind: "snippet-entry",
      entryId: entry.id,
      title: entry.title,
      defaultCode: "",
    };
    return {
      providerId: "__keyword__",
      itemId: `kw-snippet:${entry.id}`,
      title: entry.title || "(无标题)",
      subtitle: entry.description
        ? `${entry.primaryLanguage} · ${truncatePreview(entry.description, 40)}`
        : `${entry.primaryLanguage} · #${tag}`,
      badge: { short: "片", tone: "primary" },
      searchFields: [],
      payload: payload as unknown as Record<string, unknown>,
    };
  });
}

// ── resolver 主入口 ───────────────────────────────────────────────────

export async function resolveKeywordInvocation(
  invocation: KeywordCommandInvocation,
): Promise<SpotlightItem[]> {
  const { command, args } = invocation;
  switch (command.kind) {
    case "show-value":
      return produceShowValue(command, args);
    case "open-tool":
      return [produceOpenToolSuggestion(command, args)];
    case "vault-tag":
      return produceVaultTag(command.targetTag ?? "", args);
    case "snippet-tag":
      return produceSnippetTag(command.targetTag ?? "", args);
    default:
      return [];
  }
}

async function produceShowValue(
  command: KeywordCommandDescriptor,
  args: string,
): Promise<SpotlightItem[]> {
  switch (command.valueProducer) {
    case "local-ip":
      return produceLocalIp();
    case "uuid-v4":
      return produceUuid();
    case "timestamp-now":
      return produceTimestamp();
    case "hash-text":
      return produceHash(args);
    default:
      return [buildHintItem("show-value-unknown", "未知 value producer", "")];
  }
}

function produceOpenToolSuggestion(
  command: KeywordCommandDescriptor,
  args: string,
): SpotlightItem {
  const toolId = command.toolId ?? "";
  const text = command.forwardArgs ?? true ? args : "";
  const preview = text ? `(${truncatePreview(text, 28)})` : "";
  const payload: KeywordItemOpenTool = {
    __keyword: true,
    keywordItemKind: "open-tool",
    toolId,
    text,
  };
  return {
    providerId: "__keyword__",
    itemId: `kw-open:${toolId}:${text ? text.slice(0, 16) : "empty"}`,
    title: `${command.name}${preview}`,
    subtitle: text
      ? "Enter 打开工具并预填参数"
      : "Enter 打开工具",
    badge: { short: "跳", tone: "primary" },
    searchFields: [],
    payload: payload as unknown as Record<string, unknown>,
  };
}

// ── 执行链路 ─────────────────────────────────────────────────────────

export function isKeywordItem(item: SpotlightItem): boolean {
  return item.providerId === "__keyword__";
}

export async function executeKeywordItem(
  item: SpotlightItem,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  const payload = item.payload as KeywordItemPayload | undefined;
  if (!payload || payload.__keyword !== true) {
    return { errorMessage: "无效 keyword item" };
  }
  switch (payload.keywordItemKind) {
    case "show-value":
      return copyValueAction(payload.value);
    case "open-tool":
      return openToolAction(payload.toolId, payload.text);
    case "vault-entry":
      return copyVaultPasswordAction(payload, ctx);
    case "snippet-entry":
      return copySnippetCodeAction(payload);
    case "jump-tool":
      return jumpToolAction(payload.toolId, payload.toastOnJump);
    case "hint":
      return { closeSpotlight: false };
    default:
      return { errorMessage: "未知 keyword item 类型" };
  }
}

async function copyValueAction(value: string): Promise<SpotlightExecuteResult> {
  try {
    await navigator.clipboard.writeText(value);
    return {
      closeSpotlight: true,
      toast: { message: `已复制 ${truncatePreview(value, 24)}`, type: "success" },
    };
  } catch {
    return { errorMessage: "复制到剪贴板失败" };
  }
}

async function openToolAction(toolId: string, text: string): Promise<SpotlightExecuteResult> {
  if (!toolId) return { errorMessage: "未配置目标工具" };
  try {
    await invoke("spotlight_pick", {
      target: toolId,
      text: text || undefined,
      source: "keyword",
    });
    return { closeSpotlight: true };
  } catch (err) {
    return { errorMessage: err instanceof Error ? err.message : String(err) };
  }
}

async function jumpToolAction(
  toolId: string,
  toast?: string,
): Promise<SpotlightExecuteResult> {
  if (!toolId) return { errorMessage: "未配置目标工具" };
  try {
    await invoke("spotlight_pick", { target: toolId, source: "keyword" });
    return {
      closeSpotlight: true,
      toast: toast ? { message: toast, type: "info" } : undefined,
    };
  } catch (err) {
    return { errorMessage: err instanceof Error ? err.message : String(err) };
  }
}

interface VaultEntryDetail {
  fields: Record<string, unknown>;
}

async function copyVaultPasswordAction(
  payload: KeywordItemVaultEntry,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  // 复用 vault 主面板的解锁状态
  let unlocked = false;
  try {
    const status =
      ((await invokeToolByChannel("tool:vault:status", {})) as VaultStatus | null) ?? null;
    unlocked = !!status?.unlocked;
  } catch {
    /* 视为未解锁 */
  }

  if (!unlocked) {
    const ok = await ctx.ensureVaultUnlocked(payload.title);
    if (!ok) return { closeSpotlight: false };
  }

  let detail: VaultEntryDetail;
  try {
    detail = (await invokeToolByChannel("tool:vault:get", {
      id: payload.entryId,
    })) as VaultEntryDetail;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (msg.includes("vault_locked")) {
      return { errorMessage: "密码库已锁定,请重新尝试" };
    }
    return { errorMessage: msg };
  }
  const secret = ((detail.fields ?? {})["password"] as string | undefined) ?? "";
  if (!secret) return { errorMessage: "该条目没有密码字段" };
  await writeSecretToClipboard(secret);
  scheduleClipboardClear(secret);
  try {
    await invokeToolByChannel("tool:vault:record-usage", {
      id: payload.entryId,
      type: "copy",
    });
  } catch {
    /* 不阻断主流程 */
  }
  return {
    closeSpotlight: true,
    toast: { message: "密码已复制到剪贴板(30 秒后自动清空)", type: "success" },
  };
}

interface SnippetDetail {
  fragments?: Array<{ code?: string }>;
}

async function copySnippetCodeAction(
  payload: KeywordItemSnippetEntry,
): Promise<SpotlightExecuteResult> {
  let detail: SnippetDetail;
  try {
    detail = (await invokeToolByChannel("tool:snippets:v2:get", {
      id: payload.entryId,
    })) as SnippetDetail;
  } catch (err) {
    return { errorMessage: err instanceof Error ? err.message : String(err) };
  }
  const code = (detail.fragments ?? []).find((f) => f.code)?.code ?? "";
  if (!code) {
    return { errorMessage: "该片段没有可复制的代码内容" };
  }
  try {
    await navigator.clipboard.writeText(code);
  } catch {
    return { errorMessage: "复制到剪贴板失败" };
  }
  try {
    await invokeToolByChannel("tool:snippets:v2:mark-used", {
      id: payload.entryId,
      type: "copy",
    });
  } catch {
    /* 不阻断主流程 */
  }
  return {
    closeSpotlight: true,
    toast: {
      message: `已复制 ${payload.title || "片段"} 代码`,
      type: "success",
    },
  };
}

// ── 备选动作菜单(keyword 模式下的 Tab 二级菜单) ──────────────────────

export function buildKeywordItemActions(item: SpotlightItem): Array<{
  id: string;
  label: string;
  icon?: string;
  shortcut?: string;
}> {
  const payload = item.payload as KeywordItemPayload | undefined;
  if (!payload || payload.__keyword !== true) return [];
  switch (payload.keywordItemKind) {
    case "show-value":
      return [{ id: "copy", label: "复制", icon: "copy", shortcut: "Enter" }];
    case "open-tool":
      return [
        { id: "open", label: "打开工具并预填", icon: "external", shortcut: "Enter" },
      ];
    case "vault-entry":
      return [
        { id: "copy_password", label: "复制密码", icon: "lock", shortcut: "Enter" },
        { id: "open_vault", label: "跳转到凭据工具", icon: "external" },
      ];
    case "snippet-entry":
      return [
        { id: "copy_code", label: "复制片段代码", icon: "copy", shortcut: "Enter" },
        { id: "open_snippets", label: "跳转到代码片段工具", icon: "external" },
      ];
    case "jump-tool":
      return [{ id: "jump", label: "跳转", icon: "external", shortcut: "Enter" }];
    case "hint":
      return [];
    default:
      return [];
  }
}

export async function executeKeywordItemAction(
  item: SpotlightItem,
  actionId: string,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  const payload = item.payload as KeywordItemPayload | undefined;
  if (!payload || payload.__keyword !== true) {
    return { errorMessage: "无效 keyword item" };
  }
  if (payload.keywordItemKind === "vault-entry") {
    if (actionId === "copy_password") return copyVaultPasswordAction(payload, ctx);
    if (actionId === "open_vault") return jumpToolAction("vault");
  }
  if (payload.keywordItemKind === "snippet-entry") {
    if (actionId === "copy_code") return copySnippetCodeAction(payload);
    if (actionId === "open_snippets") return jumpToolAction("snippets");
  }
  // 默认走 commitDefault 同一路径
  return executeKeywordItem(item, ctx);
}

// ── 内部帮助 ─────────────────────────────────────────────────────────

interface BuildShowValueArgs {
  id: string;
  title: string;
  subtitle: string;
  value: string;
  badgeShort: string;
  badgeTone: "info" | "primary" | "warn" | "success" | "danger" | "muted";
}

function buildShowValueItem(args: BuildShowValueArgs): SpotlightItem {
  const payload: KeywordItemShowValue = {
    __keyword: true,
    keywordItemKind: "show-value",
    value: args.value,
  };
  return {
    providerId: "__keyword__",
    itemId: args.id,
    title: args.title,
    subtitle: args.subtitle,
    badge: { short: args.badgeShort, tone: args.badgeTone },
    searchFields: [],
    payload: payload as unknown as Record<string, unknown>,
  };
}

function buildJumpToolItem(
  id: string,
  title: string,
  subtitle: string,
  toolId: string,
  toast?: string,
): SpotlightItem {
  const payload: KeywordItemJumpTool = {
    __keyword: true,
    keywordItemKind: "jump-tool",
    toolId,
    toastOnJump: toast,
  };
  return {
    providerId: "__keyword__",
    itemId: id,
    title,
    subtitle,
    badge: { short: "跳", tone: "info" },
    searchFields: [],
    payload: payload as unknown as Record<string, unknown>,
  };
}

function buildHintItem(id: string, title: string, subtitle: string): SpotlightItem {
  const payload: KeywordItemHint = { __keyword: true, keywordItemKind: "hint" };
  return {
    providerId: "__keyword__",
    itemId: id,
    title,
    subtitle,
    badge: { short: "提示", tone: "muted" },
    searchFields: [],
    payload: payload as unknown as Record<string, unknown>,
  };
}

function truncatePreview(text: string, max: number): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  if (oneLine.length <= max) return oneLine;
  return oneLine.slice(0, max) + "…";
}
