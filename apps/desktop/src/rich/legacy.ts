import type { JSONContent } from "@tiptap/vue-3";

/**
 * 描述字段的 legacy 兼容层：
 * - 空字符串 → 空 doc
 * - 合法 JSON → 直接 parse
 * - 其他（legacy 纯文本） → 按行切成 paragraph 数组
 *
 * Viewer 的 tryParseDoc 在 JSON.parse 失败时返回 null，交由模板走 legacy 渲染路径
 * （white-space: pre-wrap）。
 */

function emptyDoc(): JSONContent {
  return { type: "doc", content: [{ type: "paragraph" }] };
}

export function normalizeLegacy(raw: string | null | undefined): JSONContent {
  const t = raw?.trim() ?? "";
  if (!t) return emptyDoc();
  if (t.startsWith("{")) {
    try {
      const parsed = JSON.parse(t) as JSONContent;
      if (parsed && typeof parsed === "object") return parsed;
    } catch {
      /* fall through */
    }
  }
  const paragraphs = (raw ?? "").split(/\r?\n/).map((line) => ({
    type: "paragraph",
    content: line ? [{ type: "text", text: line }] : [],
  }));
  return { type: "doc", content: paragraphs };
}

export function tryParseDoc(raw: string | null | undefined): JSONContent | null {
  const t = raw?.trim() ?? "";
  if (!t || !t.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(t) as JSONContent;
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}

/**
 * 遍历 doc 收集所有"已落盘附件"的 attId：
 * - image 节点：attId（历史图片场景）
 * - fileRef 节点且 kind='attachment'：attId（文件附件场景）
 *
 * 忽略 null / 未落盘占位节点。
 * 用于 `attachments:cleanup_orphans` 的 keepIds 列表。
 */
export function walkAttIds(doc: JSONContent | null | undefined): number[] {
  const out = new Set<number>();
  if (!doc) return [];
  const visit = (node: JSONContent | undefined) => {
    if (!node) return;
    if (node.type === "image" && node.attrs) {
      const v = node.attrs.attId;
      if (typeof v === "number" && Number.isFinite(v)) out.add(v);
    } else if (node.type === "fileRef" && node.attrs && node.attrs.kind !== "path") {
      const v = node.attrs.attId;
      if (typeof v === "number" && Number.isFinite(v)) out.add(v);
    }
    if (Array.isArray(node.content)) node.content.forEach(visit);
  };
  visit(doc);
  return [...out];
}

/**
 * 收集所有 kind='path' 的 FileRef 节点的 src（绝对路径）。
 * 用于 Viewer 挂载后的批量失效检测。
 */
export function walkFileRefPaths(doc: JSONContent | null | undefined): string[] {
  const out = new Set<string>();
  if (!doc) return [];
  const visit = (node: JSONContent | undefined) => {
    if (!node) return;
    if (node.type === "fileRef" && node.attrs && node.attrs.kind === "path") {
      const v = node.attrs.src;
      if (typeof v === "string" && v) out.add(v);
    }
    if (Array.isArray(node.content)) node.content.forEach(visit);
  };
  visit(doc);
  return [...out];
}

/**
 * 递归克隆 doc：
 * - 对 image.src 是相对路径（不带 scheme）的情况调用 rewrite 得到可访问 URL
 * - 对 link mark 的 href 做 sanitize，拦截 javascript:/data:/file: 等协议
 *
 * 注：FileRef 节点的 src 不在此重写。FileRef 的 renderHTML 只输出 data-src 原值，
 * Viewer 点击时会用 data-kind + data-src 组合自行解析绝对路径，避免 JSON 序列化
 * 保存回 DB 时污染原始相对路径 / 绝对路径字段。
 */
export function rewriteLocalSrc(doc: JSONContent, rewrite: (src: string) => string): JSONContent {
  const clone = (node: JSONContent): JSONContent => {
    const next: JSONContent = { ...node };
    if (node.attrs) next.attrs = { ...node.attrs };
    if (next.type === "image" && next.attrs && typeof next.attrs.src === "string") {
      const src = next.attrs.src as string;
      // 已带 scheme 的视作远程/已重写；其余走 rewrite
      if (!/^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(src)) {
        next.attrs.src = rewrite(src);
      }
    }
    if (Array.isArray(node.marks)) {
      next.marks = node.marks.map((m) => {
        if (m.type === "link" && m.attrs && typeof m.attrs.href === "string") {
          return { ...m, attrs: { ...m.attrs, href: sanitizeHref(m.attrs.href as string) } };
        }
        return m;
      });
    }
    if (Array.isArray(node.content)) {
      next.content = node.content.map(clone);
    }
    return next;
  };
  return clone(doc);
}

/** 清洗 href：拒绝 javascript/data/file 等危险协议，返回空串表示移除。 */
export function sanitizeHref(href: string): string {
  const t = (href ?? "").trim().toLowerCase();
  if (!t) return "";
  if (t.startsWith("javascript:") || t.startsWith("data:") || t.startsWith("file:")) return "";
  return href;
}
