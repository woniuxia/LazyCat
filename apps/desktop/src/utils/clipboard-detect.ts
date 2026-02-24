export type ClipboardContentType =
  | "json" | "xml" | "html" | "sql" | "java"
  | "jwt" | "timestamp" | "base64" | "url-encoded" | "bcrypt"
  | "unknown";

export interface ClipboardAction {
  label: string;
  toolId: string;
  toolName: string;
}

export interface ClipboardDetectResult {
  type: ClipboardContentType;
  label: string;
  preview: string;
  actions: ClipboardAction[];
}

function truncatePreview(text: string, maxLen = 80): string {
  const oneLine = text.replace(/\n/g, " ").trim();
  if (oneLine.length <= maxLen) return oneLine;
  return oneLine.slice(0, maxLen) + "...";
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
      actions: [{ label: "JWT 解析", toolId: "jwt", toolName: "JWT 解析" }],
    };
  }

  // 2. Bcrypt hash
  if (/^\$2[aby]\$\d{2}\$/.test(trimmed)) {
    return {
      type: "bcrypt",
      label: "Bcrypt",
      preview,
      actions: [{ label: "Bcrypt 验证", toolId: "bcrypt", toolName: "Bcrypt" }],
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
          { label: "格式化", toolId: "formatter", toolName: "代码格式化" },
          { label: "JSON 处理", toolId: "json-process", toolName: "JSON 处理" },
        ],
      };
    }
  } catch { /* not json */ }

  // 4. HTML (starts with < and contains html-specific tags)
  if (trimmed.startsWith("<")) {
    const lower = trimmed.toLowerCase();
    if (
      lower.includes("<!doctype html") ||
      /<html[\s>]/i.test(trimmed) ||
      /<(head|body|div|span|script|style|main|section|article|nav|footer|header)[\s>]/i.test(trimmed)
    ) {
      return {
        type: "html",
        label: "HTML",
        preview,
        actions: [{ label: "格式化", toolId: "formatter", toolName: "代码格式化" }],
      };
    }

    // 5. XML (starts with < and ends with >)
    if (trimmed.endsWith(">")) {
      return {
        type: "xml",
        label: "XML",
        preview,
        actions: [{ label: "格式化", toolId: "formatter", toolName: "代码格式化" }],
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
      actions: [{ label: "格式化", toolId: "formatter", toolName: "代码格式化" }],
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
      actions: [{ label: "格式化", toolId: "formatter", toolName: "代码格式化" }],
    };
  }

  // 8. Timestamp (10-digit seconds or 13-digit milliseconds)
  if (/^\d{10}$/.test(trimmed) || /^\d{13}$/.test(trimmed)) {
    return {
      type: "timestamp",
      label: "时间戳",
      preview,
      actions: [{ label: "时间戳转换", toolId: "timestamp", toolName: "时间戳转换" }],
    };
  }

  // 9. URL-encoded (contains multiple percent-encoded sequences)
  if ((trimmed.match(/%[0-9A-Fa-f]{2}/g) || []).length >= 3) {
    return {
      type: "url-encoded",
      label: "URL 编码",
      preview,
      actions: [{ label: "URL 解码", toolId: "url", toolName: "URL 编解码" }],
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
      actions: [{ label: "Base64 解码", toolId: "base64", toolName: "Base64" }],
    };
  }

  return null;
}
