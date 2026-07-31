export type DiffLineChange = {
  originalStartLineNumber: number;
  originalEndLineNumber: number;
  modifiedStartLineNumber: number;
  modifiedEndLineNumber: number;
};

export type DiffSummary = {
  hunks: number;
  addedLines: number;
  removedLines: number;
  changedLines: number;
};

const EXTENSION_LANGUAGE_MAP: Record<string, string> = {
  bat: "bat",
  c: "c",
  cc: "cpp",
  cpp: "cpp",
  cs: "csharp",
  css: "css",
  csv: "plaintext",
  go: "go",
  h: "cpp",
  hpp: "cpp",
  htm: "html",
  html: "html",
  ini: "ini",
  java: "java",
  js: "javascript",
  json: "json",
  json5: "json",
  jsx: "javascript",
  kt: "kotlin",
  kts: "kotlin",
  less: "less",
  lua: "lua",
  md: "markdown",
  mdx: "markdown",
  php: "php",
  properties: "ini",
  ps1: "powershell",
  py: "python",
  rb: "ruby",
  rs: "rust",
  scss: "scss",
  sh: "shell",
  sql: "sql",
  toml: "ini",
  ts: "typescript",
  tsx: "typescript",
  txt: "plaintext",
  vue: "html",
  xml: "xml",
  yaml: "yaml",
  yml: "yaml",
};

export function fileNameFromPath(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

export function detectMonacoLanguage(path: string): string {
  const fileName = fileNameFromPath(path).toLowerCase();
  if (fileName === "dockerfile") return "dockerfile";
  const extension = fileName.includes(".") ? (fileName.split(".").at(-1) ?? "") : "";
  return EXTENSION_LANGUAGE_MAP[extension] ?? "plaintext";
}

function lineCount(start: number, end: number): number {
  return end === 0 ? 0 : Math.max(0, end - start + 1);
}

export function summarizeDiff(changes: readonly DiffLineChange[] | null): DiffSummary {
  const summary: DiffSummary = { hunks: 0, addedLines: 0, removedLines: 0, changedLines: 0 };
  if (!changes) return summary;

  summary.hunks = changes.length;
  for (const change of changes) {
    const originalLines = lineCount(change.originalStartLineNumber, change.originalEndLineNumber);
    const modifiedLines = lineCount(change.modifiedStartLineNumber, change.modifiedEndLineNumber);
    summary.addedLines += Math.max(0, modifiedLines - originalLines);
    summary.removedLines += Math.max(0, originalLines - modifiedLines);
    summary.changedLines += Math.min(originalLines, modifiedLines);
  }
  return summary;
}
