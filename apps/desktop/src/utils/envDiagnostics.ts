export type EnvironmentStatus = "ok" | "missing" | "error" | "timeout" | "invalid";

export interface EnvToolItem {
  key: string;
  name: string;
  installed: boolean;
  status: EnvironmentStatus;
  version: string;
  path: string;
  paths: string[];
  error: string | null;
  suggestion: string | null;
}

export interface EnvVariableItem {
  key: string;
  value: string;
  status: EnvironmentStatus;
  detail: string;
}

export interface EnvDiagnostic {
  level: "error" | "warning" | "info";
  title: string;
  detail: string;
  suggestion: string;
}

export interface EnvDetectResponse {
  platform: string;
  arch: string;
  durationMs: number;
  summary: {
    total: number;
    installed: number;
    missing: number;
    problems: number;
    warnings: number;
  };
  tools: EnvToolItem[];
  environment: EnvVariableItem[];
  diagnostics: EnvDiagnostic[];
}

export function environmentSummaryType(result: EnvDetectResponse): "success" | "warning" | "error" {
  if (result.summary.problems > 0) return "error";
  if (result.summary.warnings > 0) return "warning";
  return "success";
}

export function buildEnvironmentReport(result: EnvDetectResponse): string {
  const lines = [
    "LazyCat 开发环境检测报告",
    `平台：${result.platform} / ${result.arch}`,
    `耗时：${result.durationMs} ms`,
    `概览：${result.summary.total} 项工具，正常 ${result.summary.installed} 项，未就绪 ${result.summary.missing} 项`,
    "",
    "[工具]",
  ];

  for (const tool of result.tools) {
    lines.push(`- ${tool.name}：${toolStatusLabel(tool.status)}；${tool.version}`);
    if (tool.path) lines.push(`  路径：${tool.path}`);
    if (tool.paths.length > 1) lines.push(`  PATH 命中：${tool.paths.join(" | ")}`);
    if (tool.error) lines.push(`  问题：${tool.error}`);
  }

  lines.push("", "[关键环境变量]");
  for (const item of result.environment) {
    lines.push(`- ${item.key}：${item.value || "未配置"}（${item.detail}）`);
  }

  if (result.diagnostics.length) {
    lines.push("", "[诊断与建议]");
    for (const diagnostic of result.diagnostics) {
      lines.push(`- [${diagnostic.level.toUpperCase()}] ${diagnostic.title}`);
      lines.push(`  ${diagnostic.detail}`);
      if (diagnostic.suggestion) lines.push(`  建议：${diagnostic.suggestion}`);
    }
  }
  return lines.join("\n");
}

export function toolStatusLabel(status: EnvironmentStatus): string {
  return {
    ok: "正常",
    missing: "未找到",
    error: "异常",
    timeout: "超时",
    invalid: "无效",
  }[status];
}
