export type ClipboardContentType =
  | "json"
  | "xml"
  | "html"
  | "sql"
  | "java"
  | "jwt"
  | "timestamp"
  | "base64"
  | "url-encoded"
  | "bcrypt"
  | "path"
  | "unknown";

export interface ClipboardToolAction {
  kind: "tool";
  label: string;
  toolId: string;
  toolName: string;
}

export interface ClipboardOpenPathAction {
  kind: "open-path";
  label: string;
  path: string;
  reveal: boolean;
}

export type ClipboardAction = ClipboardToolAction | ClipboardOpenPathAction;

export interface ClipboardDetectResult {
  type: ClipboardContentType;
  label: string;
  preview: string;
  actions: ClipboardAction[];
}

export interface ClipboardPathDetectResult {
  path: string;
  reveal: boolean;
}

function truncatePreview(text: string, maxLen = 80): string {
  const oneLine = text.replace(/\n/g, " ").trim();
  if (oneLine.length <= maxLen) return oneLine;
  return oneLine.slice(0, maxLen) + "...";
}

function createToolAction(label: string, toolId: string, toolName: string): ClipboardToolAction {
  return {
    kind: "tool",
    label,
    toolId,
    toolName,
  };
}

function inferPathLabel(path: string, reveal: boolean): string {
  if (!reveal) return "目录路径";
  const segments = path.split(/[\\/]/).filter(Boolean);
  const lastSegment = segments.length > 0 ? segments[segments.length - 1] : "";
  return /\.[^\\/.]+$/.test(lastSegment) ? "文件路径" : "目录路径";
}

function stripOuterQuotes(text: string): string {
  if (text.length >= 2) {
    const first = text[0];
    const last = text[text.length - 1];
    if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
      return text.slice(1, -1).trim();
    }
  }
  return text;
}

function normalizeFileUri(text: string): string | null {
  try {
    const url = new URL(text);
    if (url.protocol !== "file:") return null;
    if (url.hostname && url.hostname !== "localhost") {
      return `\\\\${url.hostname}${decodeURIComponent(url.pathname).replace(/\//g, "\\")}`;
    }
    const decodedPath = decodeURIComponent(url.pathname);
    if (/^\/[A-Za-z]:\//.test(decodedPath)) {
      return decodedPath.slice(1).replace(/\//g, "\\");
    }
    return null;
  } catch {
    return null;
  }
}

function isWindowsAbsolutePath(text: string): boolean {
  return /^[A-Za-z]:\\/.test(text);
}

function isUncPath(text: string): boolean {
  return /^\\\\[^\\/]+\\[^\\/]+/.test(text);
}

function inferRevealFromPath(text: string): boolean {
  return !/[\\/]$/.test(text) && !/^[A-Za-z]:\\?$/.test(text);
}

export function detectClipboardPath(text: string): ClipboardPathDetectResult | null {
  if (!text) return null;

  const trimmed = text.trim();
  if (!trimmed || trimmed.includes("\n") || trimmed.includes("\r")) return null;

  const hadOuterQuotes =
    trimmed.length >= 2 &&
    ((trimmed.startsWith('"') && trimmed.endsWith('"')) ||
      (trimmed.startsWith("'") && trimmed.endsWith("'")));
  const unquoted = stripOuterQuotes(trimmed);
  if (!unquoted || /[%][A-Za-z_][A-Za-z0-9_]*%/.test(unquoted)) return null;
  if (/[<>|]/.test(unquoted) || /\s(?:[-/][A-Za-z][\w-]*)(?:\s|$)/.test(unquoted)) return null;
  if (!hadOuterQuotes && /\s{2,}|\t/.test(unquoted)) return null;

  const normalized = unquoted.toLowerCase().startsWith("file://")
    ? normalizeFileUri(unquoted)
    : unquoted;
  if (!normalized) return null;
  if (!isWindowsAbsolutePath(normalized) && !isUncPath(normalized)) return null;

  return {
    path: normalized,
    reveal: inferRevealFromPath(normalized),
  };
}

export function buildClipboardPathSuggestion(
  match: ClipboardPathDetectResult,
): ClipboardDetectResult {
  return {
    type: "path",
    label: inferPathLabel(match.path, match.reveal),
    preview: truncatePreview(match.path),
    actions: [
      {
        kind: "open-path",
        label: "直接打开",
        path: match.path,
        reveal: match.reveal,
      },
    ],
  };
}

/**
 * 检测剪贴板文本内容类型，返回检测结果或 null（无法识别/不适合检测）。
 * 检测优先级从高置信度到低置信度排列。
 */
export function detectClipboardContent(text: string): ClipboardDetectResult | null {
  if (!text) return null;
  const trimmed = text.trim();
  if (trimmed.length < 5 || trimmed.length > 100_000) return null;

  const preview = truncatePreview(trimmed);

  // 1. JWT: header.payload.signature
  if (/^[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+$/.test(trimmed)) {
    return {
      type: "jwt",
      label: "JWT Token",
      preview,
      actions: [createToolAction("JWT 解析", "jwt", "JWT 解析")],
    };
  }

  // 2. Bcrypt hash
  if (/^\$2[aby]\$\d{2}\$/.test(trimmed)) {
    return {
      type: "bcrypt",
      label: "Bcrypt",
      preview,
      actions: [createToolAction("Bcrypt 验证", "bcrypt", "Bcrypt")],
    };
  }

  // 3. JSON
  try {
    const parsed = JSON.parse(trimmed);
    if (typeof parsed === "object" && parsed !== null) {
      return {
        type: "json",
        label: "JSON",
        preview,
        actions: [
          createToolAction("处理与转换", "json-workbench", "JSON 工作台"),
          createToolAction("代码格式化", "formatter", "代码格式化"),
        ],
      };
    }
  } catch {
    /* not json */
  }

  // 4. HTML (starts with < and contains html-specific tags)
  if (trimmed.startsWith("<")) {
    const lower = trimmed.toLowerCase();
    if (
      lower.includes("<!doctype html") ||
      /<html[\s>]/i.test(trimmed) ||
      /<(head|body|div|span|script|style|main|section|article|nav|footer|header)[\s>]/i.test(
        trimmed,
      )
    ) {
      return {
        type: "html",
        label: "HTML",
        preview,
        actions: [createToolAction("格式化", "formatter", "代码格式化")],
      };
    }

    // 5. XML (starts with < and ends with >)
    if (trimmed.endsWith(">")) {
      return {
        type: "xml",
        label: "XML",
        preview,
        actions: [createToolAction("格式化", "formatter", "代码格式化")],
      };
    }
  }

  // 6. SQL
  if (
    /\b(select|insert|update|delete|create|alter|drop|truncate|with)\b/i.test(trimmed) &&
    /\b(from|into|table|where|values|set|join)\b/i.test(trimmed)
  ) {
    return {
      type: "sql",
      label: "SQL",
      preview,
      actions: [createToolAction("格式化", "formatter", "代码格式化")],
    };
  }

  // 7. Java
  if (
    /\b(class|interface|enum|record)\b/.test(trimmed) &&
    /\b(public|private|protected|static|void|package|import)\b/.test(trimmed)
  ) {
    return {
      type: "java",
      label: "Java",
      preview,
      actions: [createToolAction("格式化", "formatter", "代码格式化")],
    };
  }

  // 8. Timestamp (10-digit seconds or 13-digit milliseconds)
  if (/^\d{10}$/.test(trimmed) || /^\d{13}$/.test(trimmed)) {
    return {
      type: "timestamp",
      label: "时间戳",
      preview,
      actions: [createToolAction("时间戳转换", "timestamp", "时间戳转换")],
    };
  }

  // 9. URL-encoded (contains multiple percent-encoded sequences)
  if ((trimmed.match(/%[0-9A-Fa-f]{2}/g) || []).length >= 3) {
    return {
      type: "url-encoded",
      label: "URL 编码",
      preview,
      actions: [createToolAction("URL 解码", "url", "URL 编解码")],
    };
  }

  // 10. Base64 (20+ chars, valid alphabet, length % 4 == 0)
  const base64Candidate = trimmed.replace(/\n/g, "");
  if (
    base64Candidate.length >= 20 &&
    base64Candidate.length % 4 === 0 &&
    /^[A-Za-z0-9+/]+={0,2}$/.test(base64Candidate)
  ) {
    return {
      type: "base64",
      label: "Base64",
      preview,
      actions: [createToolAction("Base64 解码", "base64", "Base64")],
    };
  }

  return null;
}
