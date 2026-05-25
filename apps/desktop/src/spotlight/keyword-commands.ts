import type {
  KeywordCommandCustom,
  KeywordCommandDescriptor,
  SpotlightConfigKeywordCommands,
} from "./types";

// ── 内置 KeywordCommand 集 ─────────────────────────────────────────────
//
// 8 个默认启用的内置项。用户可在 SpotlightSettings 中启用 / 禁用,但不能
// 改 keyword、name、目标行为。内置项的"行为微调"留给后续版本。
//
// 工具 ID 与 toolCatalog.ts 中 sidebar items 对齐:
//   base64 / jwt / regex / color 都已在 sidebar
//   hash(SHA/HMAC)与 uuid 走对应工具,但本期 ;hash 和 ;uuid 都是 show-value 类
//
// 注意:keyword 不含 ";" 前缀,统一小写存放。

export const BUILTIN_KEYWORD_COMMANDS: KeywordCommandDescriptor[] = [
  {
    id: "ip",
    keyword: "ip",
    name: "本机 IP",
    description: "列出所有网卡的 IPv4 / IPv6 地址",
    kind: "show-value",
    origin: "builtin",
    valueProducer: "local-ip",
    defaultEnabled: true,
  },
  {
    id: "uuid",
    keyword: "uuid",
    name: "生成 UUID",
    description: "一次生成 5 个 UUID v4,Enter 复制",
    kind: "show-value",
    origin: "builtin",
    valueProducer: "uuid-v4",
    defaultEnabled: true,
  },
  {
    id: "ts",
    keyword: "ts",
    name: "当前时间",
    description: "Unix 秒 / 毫秒 / ISO / RFC 3339 / 本地友好",
    kind: "show-value",
    origin: "builtin",
    valueProducer: "timestamp-now",
    defaultEnabled: true,
  },
  {
    id: "hash",
    keyword: "hash",
    name: "哈希文本",
    description: "对参数计算 MD5 / SHA-1 / SHA-256",
    kind: "show-value",
    origin: "builtin",
    valueProducer: "hash-text",
    defaultEnabled: true,
  },
  {
    id: "b64",
    keyword: "b64",
    name: "跳 Base64 工具",
    description: "打开 Base64 工具并预填参数",
    kind: "open-tool",
    origin: "builtin",
    toolId: "base64",
    forwardArgs: true,
    defaultEnabled: true,
  },
  {
    id: "jwt",
    keyword: "jwt",
    name: "跳 JWT 工具",
    description: "打开 JWT 解析工具并预填 Token",
    kind: "open-tool",
    origin: "builtin",
    toolId: "jwt",
    forwardArgs: true,
    defaultEnabled: true,
  },
  {
    id: "regex",
    keyword: "regex",
    name: "跳正则工具",
    description: "打开正则工具,在测试文本里预填参数",
    kind: "open-tool",
    origin: "builtin",
    toolId: "regex",
    forwardArgs: true,
    defaultEnabled: true,
  },
  {
    id: "color",
    keyword: "color",
    name: "跳颜色工具",
    description: "打开颜色转换工具并预填",
    kind: "open-tool",
    origin: "builtin",
    toolId: "color",
    forwardArgs: true,
    defaultEnabled: true,
  },
];

// ── 校验:用户自定义 keyword 是否合法 ─────────────────────────────────

const KEYWORD_PATTERN = /^[a-zA-Z0-9_-]{1,24}$/;

export interface KeywordValidationResult {
  ok: boolean;
  error?: string;
  normalized: string;
}

/**
 * 校验自定义 keyword 是否合法。
 *
 * - 字符集 [a-zA-Z0-9_-]+,长度 1-24
 * - 不允许与内置 keyword 重复
 * - 不允许与其它已启用的自定义 keyword 重复
 *
 * 注意:禁用的自定义项不参与冲突检测(允许保留同名禁用项)。
 */
export function validateCustomKeyword(
  rawKeyword: string,
  options: {
    /** 当前编辑项的 id,用于排除自身 */
    selfId?: string;
    /** 当前已有的自定义项,用于查重 */
    existingCustom: KeywordCommandCustom[];
  },
): KeywordValidationResult {
  const normalized = rawKeyword.trim().toLowerCase();
  if (!normalized) {
    return { ok: false, normalized, error: "keyword 不能为空" };
  }
  if (!KEYWORD_PATTERN.test(normalized)) {
    return {
      ok: false,
      normalized,
      error: "仅允许 1-24 位字母数字与 _ -",
    };
  }
  for (const b of BUILTIN_KEYWORD_COMMANDS) {
    if (b.keyword === normalized) {
      return { ok: false, normalized, error: `"${normalized}" 是内置命令` };
    }
  }
  for (const c of options.existingCustom) {
    if (c.id === options.selfId) continue;
    if (!c.enabled) continue;
    if (c.keyword.toLowerCase() === normalized) {
      return { ok: false, normalized, error: `"${normalized}" 已被另一项占用` };
    }
  }
  return { ok: true, normalized };
}

// ── 合并:运行时 keyword 索引 ──────────────────────────────────────────

export interface ResolvedKeywordCommands {
  commands: KeywordCommandDescriptor[];
  index: Map<string, KeywordCommandDescriptor>;
}

/**
 * 把内置默认 + 用户覆盖合并为运行时 keyword 列表与索引。
 *
 * 处理顺序:
 *   1. 遍历 BUILTIN_KEYWORD_COMMANDS,按 builtinOverrides 决定 enabled
 *   2. 遍历 customList,过滤非法 / 禁用项
 *   3. 内置 vs 自定义冲突时,内置优先(custom 在 store 校验阶段已挡掉同名内置,
 *      这里防御性兜底)
 *   4. 自定义之间冲突时,先注册者胜出(降级到"仅留一个"策略)
 */
export function resolveKeywordCommands(
  config: SpotlightConfigKeywordCommands | undefined,
): ResolvedKeywordCommands {
  const builtinOverrides = config?.builtins ?? {};
  const customList = config?.custom ?? [];

  const commands: KeywordCommandDescriptor[] = [];
  const index = new Map<string, KeywordCommandDescriptor>();

  for (const b of BUILTIN_KEYWORD_COMMANDS) {
    const override = builtinOverrides[b.id];
    const enabled = override?.enabled ?? b.defaultEnabled;
    if (!enabled) continue;
    commands.push(b);
    index.set(b.keyword, b);
  }

  for (const c of customList) {
    if (!c.enabled) continue;
    const keyword = c.keyword.trim().toLowerCase();
    if (!KEYWORD_PATTERN.test(keyword)) continue;
    if (index.has(keyword)) continue; // 内置优先,或先注册者胜出
    if (c.kind === "open-tool" && !c.toolId) continue;
    if ((c.kind === "vault-tag" || c.kind === "snippet-tag") && !c.targetTag) continue;
    const descriptor: KeywordCommandDescriptor = {
      id: c.id,
      keyword,
      name: c.name || keyword,
      description: c.description || "",
      kind: c.kind,
      origin: "custom",
      toolId: c.toolId,
      forwardArgs: c.forwardArgs ?? true,
      targetTag: c.targetTag,
      defaultEnabled: true,
    };
    commands.push(descriptor);
    index.set(keyword, descriptor);
  }

  return { commands, index };
}

// ── 工具:为新自定义项生成稳定 id ──────────────────────────────────────

let __idSeq = 0;
export function generateCustomKeywordId(): string {
  __idSeq += 1;
  const ts = Date.now().toString(36);
  const seq = __idSeq.toString(36);
  const rand = Math.floor(Math.random() * 1e6).toString(36);
  return `kw-${ts}-${seq}-${rand}`;
}
