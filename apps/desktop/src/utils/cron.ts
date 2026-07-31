import type { CronFieldParts, CronStandard } from "../types";

export interface CronStandardOption {
  label: string;
  value: CronStandard;
  caption: string;
}

export interface CronTemplate {
  label: string;
  parts: CronFieldParts;
}

export const CRON_STANDARD_OPTIONS: CronStandardOption[] = [
  { label: "Linux", value: "linux5", caption: "5 字段 · Crontab / K8s" },
  { label: "Spring", value: "spring6", caption: "6 字段 · @Scheduled" },
  { label: "Quartz", value: "quartz", caption: "6 字段 · 日/周使用 ?" },
];

const baseParts: CronFieldParts = {
  second: "0",
  minute: "*",
  hour: "*",
  dayOfMonth: "*",
  month: "*",
  dayOfWeek: "*",
};

function template(label: string, parts: Partial<CronFieldParts>): CronTemplate {
  return { label, parts: { ...baseParts, ...parts } };
}

export const CRON_TEMPLATES: CronTemplate[] = [
  template("每30秒", { second: "*/30" }),
  template("每分钟", {}),
  template("每5分钟", { minute: "*/5" }),
  template("每10分钟", { minute: "*/10" }),
  template("每15分钟", { minute: "*/15" }),
  template("每30分钟", { minute: "*/30" }),
  template("每小时整点", { minute: "0" }),
  template("每2小时", { minute: "0", hour: "*/2" }),
  template("每天 00:00", { minute: "0", hour: "0" }),
  template("每天 09:00", { minute: "0", hour: "9" }),
  template("每天 18:00", { minute: "0", hour: "18" }),
  template("工作日 09:00", { minute: "0", hour: "9", dayOfWeek: "Mon-Fri" }),
  template("工作日 18:00", { minute: "0", hour: "18", dayOfWeek: "Mon-Fri" }),
  template("每周一 09:00", { minute: "0", hour: "9", dayOfWeek: "Mon" }),
  template("每周五 18:00", { minute: "0", hour: "18", dayOfWeek: "Fri" }),
  template("每月 1 日 00:00", { minute: "0", hour: "0", dayOfMonth: "1" }),
  template("每月 1 日 09:00", { minute: "0", hour: "9", dayOfMonth: "1" }),
];

export function templatesForStandard(standard: CronStandard): CronTemplate[] {
  if (standard !== "linux5") return CRON_TEMPLATES;
  return CRON_TEMPLATES.filter((item) => item.parts.second === "0");
}

export function coerceCronParts(parts: CronFieldParts, standard: CronStandard): CronFieldParts {
  const result = { ...parts };
  if (standard === "linux5") result.second = "0";

  if (standard !== "quartz") {
    if (result.dayOfMonth === "?") result.dayOfMonth = "*";
    if (result.dayOfWeek === "?") result.dayOfWeek = "*";
    return result;
  }

  const domSpecific = result.dayOfMonth !== "*" && result.dayOfMonth !== "?";
  const dowSpecific = result.dayOfWeek !== "*" && result.dayOfWeek !== "?";
  if (domSpecific && !dowSpecific) result.dayOfWeek = "?";
  else if (dowSpecific && !domSpecific) result.dayOfMonth = "?";
  else if (!domSpecific && !dowSpecific) {
    result.dayOfMonth = "*";
    result.dayOfWeek = "?";
  }
  return result;
}

export function buildCronExpression(parts: CronFieldParts, standard: CronStandard): string {
  const normalized = coerceCronParts(parts, standard);
  const common = [
    normalized.minute,
    normalized.hour,
    normalized.dayOfMonth,
    normalized.month,
    normalized.dayOfWeek,
  ];
  return standard === "linux5" ? common.join(" ") : [normalized.second, ...common].join(" ");
}
