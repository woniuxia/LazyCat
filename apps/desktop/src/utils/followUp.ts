import type { FollowUpDraft, FollowUpGroup, FollowUpItem } from "../types/follow-up";
import type { PendingToolInput } from "../types/navigation-handoff";

export function emptyFollowUpDraft(now = new Date()): FollowUpDraft {
  return {
    id: null,
    title: "",
    description: "",
    expectedOutcome: "",
    priority: "P2",
    personId: null,
    reviewAt: quickReviewAt(1, now),
    expectedCompletionAt: "",
    links: [],
  };
}

export function quickReviewAt(days: number, now = new Date()): string {
  const next = new Date(now);
  next.setDate(next.getDate() + days);
  next.setHours(9, 0, 0, 0);
  return next.toISOString();
}

export function followUpGroup(item: FollowUpItem, now = new Date()): FollowUpGroup {
  if (item.attentionStatus === "ended") return "ended";
  const reviewAt = new Date(item.reviewAt ?? "");
  if (reviewAt.getTime() <= now.getTime()) return "due";
  const sevenDaysLater = new Date(now);
  sevenDaysLater.setHours(0, 0, 0, 0);
  sevenDaysLater.setDate(sevenDaysLater.getDate() + 8);
  return reviewAt.getTime() < sevenDaysLater.getTime() ? "soon" : "later";
}

export function groupFollowUpItems(items: readonly FollowUpItem[], now = new Date()) {
  const groups: Record<FollowUpGroup, FollowUpItem[]> = { due: [], soon: [], later: [], ended: [] };
  for (const item of items) groups[followUpGroup(item, now)].push(item);
  for (const key of ["due", "soon", "later"] as const) {
    groups[key].sort(
      (a, b) =>
        new Date(a.reviewAt ?? 0).getTime() - new Date(b.reviewAt ?? 0).getTime() || a.id - b.id,
    );
  }
  groups.ended.sort(
    (a, b) =>
      new Date(b.endedAt ?? 0).getTime() - new Date(a.endedAt ?? 0).getTime() || b.id - a.id,
  );
  return groups;
}

export function externalDeadlineReached(item: FollowUpItem, now = new Date()): boolean {
  return (
    item.externalResult === "unknown" &&
    Boolean(item.expectedCompletionAt) &&
    new Date(item.expectedCompletionAt!).getTime() <= now.getTime()
  );
}

export function buildFollowUpTodoInput(item: FollowUpItem): PendingToolInput {
  const context = [
    item.expectedOutcome && `预期结果：${item.expectedOutcome}`,
    item.description,
    item.latestProgress && `最近进展：${item.latestProgress.content}`,
    item.links.length && `相关链接：\n${item.links.map((link) => link.url).join("\n")}`,
  ]
    .filter(Boolean)
    .join("\n\n");
  return {
    toolId: "todo",
    text: item.title,
    todoDraft: { title: item.title, description: context },
  };
}
