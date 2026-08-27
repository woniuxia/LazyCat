import { describe, expect, it } from "vitest";
import type { FollowUpItem } from "../types/follow-up";
import {
  buildFollowUpTodoInput,
  emptyFollowUpDraft,
  externalDeadlineReached,
  followUpProcessedLabel,
  groupFollowUpItems,
  mergeProcessedFollowUpItems,
  quickReviewAt,
  type FollowUpProcessedMark,
} from "./followUp";

function item(id: number, reviewAt: string | null, endedAt: string | null = null): FollowUpItem {
  return {
    id,
    title: `事项${id}`,
    description: "",
    expectedOutcome: "",
    priority: "P2",
    attentionStatus: endedAt ? "ended" : "active",
    externalResult: "unknown",
    endingMode: endedAt ? "stopped_following" : null,
    personId: 1,
    personName: "张三",
    personNameSnapshot: "张三",
    reviewAt,
    expectedCompletionAt: null,
    snoozeUntil: null,
    lastNotifiedReviewAt: null,
    endedAt,
    createdAt: "2026-08-01T00:00:00Z",
    updatedAt: "2026-08-01T00:00:00Z",
    latestProgress: null,
    progress: [],
    links: [],
  };
}

describe("follow-up grouping", () => {
  it("defaults a new review to 09:00 on the next local day", () => {
    const now = new Date("2026-08-18T22:47:30+08:00");
    const expected = new Date(now);
    expected.setDate(expected.getDate() + 1);
    expected.setHours(9, 0, 0, 0);

    expect(quickReviewAt(1, now)).toBe(expected.toISOString());
    expect(emptyFollowUpDraft(now).reviewAt).toBe(expected.toISOString());
  });

  it("groups by persisted review time using the next seven local calendar days", () => {
    const now = new Date("2026-08-18T10:00:00+08:00");
    const groups = groupFollowUpItems(
      [
        item(3, "2026-08-27T00:00:00+08:00"),
        item(2, "2026-08-20T10:00:00+08:00"),
        item(1, "2026-08-18T09:00:00+08:00"),
        item(4, null, "2026-08-17T10:00:00+08:00"),
      ],
      now,
    );
    expect(groups.due.map((value) => value.id)).toEqual([1]);
    expect(groups.soon.map((value) => value.id)).toEqual([2]);
    expect(groups.later.map((value) => value.id)).toEqual([3]);
    expect(groups.ended.map((value) => value.id)).toEqual([4]);
  });

  it("derives external deadline independently from review grouping", () => {
    const value = item(1, "2026-08-25T10:00:00+08:00");
    value.expectedCompletionAt = "2026-08-18T09:00:00+08:00";
    expect(externalDeadlineReached(value, new Date("2026-08-18T10:00:00+08:00"))).toBe(true);
  });

  it("builds a reviewable Todo draft without a persistent association", () => {
    const value = item(1, "2026-08-25T10:00:00+08:00");
    value.expectedOutcome = "通过验收";
    value.latestProgress = {
      id: 3,
      kind: "progress",
      content: "等待复测",
      occurredAt: "2026-08-18T01:00:00Z",
      updatedAt: "2026-08-18T01:00:00Z",
    };
    const input = buildFollowUpTodoInput(value);
    expect(input.todoDraft?.title).toBe("事项1");
    expect(input.todoDraft?.description).toContain("预期结果：通过验收");
    expect(input.todoDraft?.description).toContain("最近进展：等待复测");
    expect(input.meta).toBeUndefined();
  });
});

describe("就地已处理合并", () => {
  it("appends processed items that left the group in processing order", () => {
    const dueItems = [item(1, "2026-08-18T09:00:00+08:00")];
    const candidates = [
      ...dueItems,
      item(2, "2026-08-20T10:00:00+08:00"),
      item(3, "2026-08-27T10:00:00+08:00"),
    ];
    const marks = new Map<number, FollowUpProcessedMark>([
      [2, "continued"],
      [3, "updated"],
    ]);

    const merged = mergeProcessedFollowUpItems(dueItems, candidates, marks);

    expect(merged.map((value) => value.id)).toEqual([1, 2, 3]);
    expect(followUpProcessedLabel("continued")).toBe("已复查");
    expect(followUpProcessedLabel("ended")).toBe("已结束");
    expect(followUpProcessedLabel("reopened")).toBe("已重新关注");
    expect(followUpProcessedLabel("updated")).toBe("已更新");
  });

  it("does not duplicate items that returned to the group", () => {
    const dueItems = [item(1, "2026-08-18T09:00:00+08:00"), item(2, "2026-08-18T09:30:00+08:00")];
    const candidates = [...dueItems, item(3, "2026-08-27T10:00:00+08:00")];
    const marks = new Map<number, FollowUpProcessedMark>([
      [2, "continued"],
      [3, "ended"],
    ]);

    const merged = mergeProcessedFollowUpItems(dueItems, candidates, marks);

    expect(merged.map((value) => value.id)).toEqual([1, 2, 3]);
  });

  it("skips marks whose item is missing from the loaded candidates", () => {
    const merged = mergeProcessedFollowUpItems(
      [],
      [item(1, "2026-08-18T09:00:00+08:00")],
      new Map<number, FollowUpProcessedMark>([[9, "continued"]]),
    );

    expect(merged).toEqual([]);
  });
});
