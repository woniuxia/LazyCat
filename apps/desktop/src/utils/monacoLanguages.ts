import { detectClipboardContent, type ClipboardContentType } from "./clipboard-detect";

export const MAX_REFERENCE_CARD_TEXT_BYTES = 8 * 1024 * 1024;

export const MONACO_LANGUAGE_OPTIONS = [
  "javascript", "typescript", "python", "java", "go", "rust", "sql", "html", "css",
  "json", "xml", "yaml", "bash", "shell", "markdown", "plaintext", "c", "cpp", "csharp",
  "php", "ruby", "swift", "kotlin", "scala", "lua", "r", "dart", "dockerfile", "graphql", "toml",
] as const;

export const MONACO_LANGUAGE_EXTENSIONS: Record<string, string> = {
  javascript: "js",
  typescript: "ts",
  python: "py",
  java: "java",
  go: "go",
  rust: "rs",
  sql: "sql",
  html: "html",
  css: "css",
  json: "json",
  xml: "xml",
  yaml: "yml",
  bash: "sh",
  shell: "sh",
  markdown: "md",
  plaintext: "txt",
  c: "c",
  cpp: "cpp",
  csharp: "cs",
  php: "php",
  ruby: "rb",
  swift: "swift",
  kotlin: "kt",
  scala: "scala",
  lua: "lua",
  r: "r",
  dart: "dart",
  dockerfile: "dockerfile",
  graphql: "graphql",
  toml: "toml",
};

const CLIPBOARD_LANGUAGE_MAP: Partial<Record<ClipboardContentType, string>> = {
  json: "json",
  xml: "xml",
  html: "html",
  sql: "sql",
  java: "java",
};

export function detectClipboardMonacoLanguage(text: string): string {
  const type = detectClipboardContent(text)?.type;
  return (type && CLIPBOARD_LANGUAGE_MAP[type]) || "plaintext";
}

export function validateReferenceCardText(text: string): { ok: true } | { ok: false; message: string } {
  if (!text.trim()) return { ok: false, message: "剪贴板中没有可用文本" };
  if (new TextEncoder().encode(text).byteLength > MAX_REFERENCE_CARD_TEXT_BYTES) {
    return { ok: false, message: "参考文本不能超过 8 MiB" };
  }
  return { ok: true };
}
