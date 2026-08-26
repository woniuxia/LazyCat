export interface FollowUpQuickInput {
  id: string;
  text: string;
  usageCount: number;
  lastUsedAt: number | null;
  createdAt: number;
}

export const FOLLOW_UP_QUICK_INPUTS_SETTING_KEY = "follow-up.quick-inputs";
export const FOLLOW_UP_QUICK_INPUT_MAX_LENGTH = 2000;
export const DEFAULT_FOLLOW_UP_QUICK_INPUTS = [
  "暂无新进展",
  "正在处理中",
  "等待对方反馈",
  "已完成",
  "已取消",
] as const;

export function createDefaultFollowUpQuickInputs(now: number): FollowUpQuickInput[] {
  return DEFAULT_FOLLOW_UP_QUICK_INPUTS.map((text, index) => ({
    id: `default-${index}`,
    text,
    usageCount: 0,
    lastUsedAt: null,
    createdAt: now + index,
  }));
}

export function parseFollowUpQuickInputs(value: unknown): FollowUpQuickInput[] | null {
  const raw =
    typeof value === "string"
      ? value
      : value && typeof value === "object"
        ? (value as { value?: unknown }).value
        : null;
  if (raw === null || raw === undefined) return null;
  if (typeof raw !== "string") throw new Error("快速输入配置格式无效");

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error("快速输入配置不是有效的 JSON");
  }
  if (!Array.isArray(parsed)) throw new Error("快速输入配置必须是数组");
  const valid = parsed.every(
    (item) =>
      item &&
      typeof item.id === "string" &&
      typeof item.text === "string" &&
      typeof item.usageCount === "number" &&
      (item.lastUsedAt === null || typeof item.lastUsedAt === "number") &&
      typeof item.createdAt === "number",
  );
  if (!valid) throw new Error("快速输入配置包含无效条目");
  return parsed as FollowUpQuickInput[];
}
export function sortFollowUpQuickInputs(items: FollowUpQuickInput[]): FollowUpQuickInput[] {
  return [...items].sort(
    (a, b) =>
      b.usageCount - a.usageCount ||
      (b.lastUsedAt ?? 0) - (a.lastUsedAt ?? 0) ||
      a.createdAt - b.createdAt,
  );
}

export function editFollowUpQuickInput(
  items: FollowUpQuickInput[],
  id: string,
  text: string,
): FollowUpQuickInput[] {
  return items.map((item) => (item.id === id ? { ...item, text } : item));
}

export function deleteFollowUpQuickInput(
  items: FollowUpQuickInput[],
  id: string,
): FollowUpQuickInput[] {
  return items.filter((item) => item.id !== id);
}

export function recordFollowUpQuickInputUsage(
  items: FollowUpQuickInput[],
  id: string,
  usedAt: number,
): FollowUpQuickInput[] {
  return items.map((item) =>
    item.id === id ? { ...item, usageCount: item.usageCount + 1, lastUsedAt: usedAt } : item,
  );
}

export function appendFollowUpQuickInput(current: string, text: string): string {
  return current.length ? `${current}\n${text}` : text;
}
