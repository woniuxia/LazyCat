import { describe, expect, it } from "vitest";
import { isFiveMinuteDateTime, splitDateTime } from "./todoSchedule";
import { buildQuickAddPayload, type QuickAddContext, type QuickAddInput } from "./todoQuickAdd";

const baseContext: QuickAddContext = {
  typeId: null,
  projectId: null,
  priorityDefault: "P2",
};

function buildInput(overrides: Partial<QuickAddInput> = {}): QuickAddInput {
  return {
    title: "写周报",
    dateChoice: null,
    priorityOverride: null,
    ...overrides,
  };
}

describe("buildQuickAddPayload", () => {
  it("returns null for empty or whitespace-only titles", () => {
    expect(buildQuickAddPayload(buildInput({ title: "" }), baseContext)).toBeNull();
    expect(buildQuickAddPayload(buildInput({ title: "   " }), baseContext)).toBeNull();
    expect(buildQuickAddPayload(buildInput({ title: "\t\n" }), baseContext)).toBeNull();
  });

  it("rounds 'today' up to the next five-minute tick", () => {
    const payload = buildQuickAddPayload(
      buildInput({ dateChoice: { kind: "today" } }),
      baseContext,
      new Date(2026, 6, 4, 14, 3, 0, 0),
    );
    expect(splitDateTime(payload?.eventAt as string)).toEqual({
      date: "2026-07-04",
      time: "14:05",
    });
  });

  it("moves 'today' forward even when already on a tick", () => {
    const payload = buildQuickAddPayload(
      buildInput({ dateChoice: { kind: "today" } }),
      baseContext,
      new Date(2026, 6, 4, 14, 0, 0, 0),
    );
    expect(splitDateTime(payload?.eventAt as string)).toEqual({
      date: "2026-07-04",
      time: "14:05",
    });
  });

  it("carries 'today' across midnight into the next day", () => {
    const payload = buildQuickAddPayload(
      buildInput({ dateChoice: { kind: "today" } }),
      baseContext,
      new Date(2026, 6, 4, 23, 58, 0, 0),
    );
    expect(splitDateTime(payload?.eventAt as string)).toEqual({
      date: "2026-07-05",
      time: "00:00",
    });
  });

  it("uses 09:00 for 'tomorrow' and explicit dates", () => {
    const tomorrow = buildQuickAddPayload(
      buildInput({ dateChoice: { kind: "tomorrow" } }),
      baseContext,
      new Date(2026, 6, 4, 14, 3, 0, 0),
    );
    expect(splitDateTime(tomorrow?.eventAt as string)).toEqual({
      date: "2026-07-05",
      time: "09:00",
    });

    const picked = buildQuickAddPayload(
      buildInput({ dateChoice: { kind: "date", date: "2026-07-20" } }),
      baseContext,
      new Date(2026, 6, 4, 14, 3, 0, 0),
    );
    expect(splitDateTime(picked?.eventAt as string)).toEqual({ date: "2026-07-20", time: "09:00" });
  });

  it("carries 'tomorrow' across month boundaries", () => {
    const payload = buildQuickAddPayload(
      buildInput({ dateChoice: { kind: "tomorrow" } }),
      baseContext,
      new Date(2026, 6, 31, 10, 0, 0, 0),
    );
    expect(splitDateTime(payload?.eventAt as string)).toEqual({
      date: "2026-08-01",
      time: "09:00",
    });
  });

  it("omits eventAt when no date is chosen", () => {
    const payload = buildQuickAddPayload(buildInput(), baseContext);
    expect(payload).not.toBeNull();
    expect(payload).not.toHaveProperty("eventAt");
  });

  it("prefers priorityOverride over the context default", () => {
    const overridden = buildQuickAddPayload(buildInput({ priorityOverride: "P0" }), baseContext);
    expect(overridden?.priority).toBe("P0");

    const inherited = buildQuickAddPayload(buildInput(), { ...baseContext, priorityDefault: "P1" });
    expect(inherited?.priority).toBe("P1");
  });

  it("only includes typeId/projectId when the context resolves them", () => {
    const bare = buildQuickAddPayload(buildInput(), baseContext);
    expect(bare).not.toHaveProperty("typeId");
    expect(bare).not.toHaveProperty("projectId");

    const scoped = buildQuickAddPayload(buildInput(), {
      typeId: 3,
      projectId: 7,
      priorityDefault: "P2",
    });
    expect(scoped?.typeId).toBe(3);
    expect(scoped?.projectId).toBe(7);
  });

  it("always sets reminderPresets to none and never sends kind", () => {
    const payload = buildQuickAddPayload(
      buildInput({ dateChoice: { kind: "today" } }),
      baseContext,
      new Date(2026, 6, 4, 14, 3, 0, 0),
    );
    expect(payload?.reminderPresets).toEqual(["none"]);
    expect(payload).not.toHaveProperty("kind");
    expect(payload?.title).toBe("写周报");
    expect(isFiveMinuteDateTime(payload?.eventAt as string)).toBe(true);
  });

  it("keeps five-minute alignment for every date choice", () => {
    for (const dateChoice of [
      { kind: "today" } as const,
      { kind: "tomorrow" } as const,
      { kind: "date", date: "2026-07-20" } as const,
    ]) {
      const payload = buildQuickAddPayload(
        buildInput({ dateChoice }),
        baseContext,
        new Date(2026, 6, 4, 9, 57, 30, 0),
      );
      expect(isFiveMinuteDateTime(payload?.eventAt as string)).toBe(true);
    }
  });
});
