import { describe, expect, it } from "vitest";
import { buildCronExpression, coerceCronParts, templatesForStandard } from "./cron";
import type { CronFieldParts } from "../types";

const parts: CronFieldParts = {
  second: "0",
  minute: "0",
  hour: "9",
  dayOfMonth: "*",
  month: "*",
  dayOfWeek: "Mon-Fri",
};

describe("cron dialect helpers", () => {
  it("emits five fields for Linux", () => {
    expect(buildCronExpression(parts, "linux5")).toBe("0 9 * * Mon-Fri");
  });

  it("keeps seconds for Spring", () => {
    expect(buildCronExpression(parts, "spring6")).toBe("0 0 9 * * Mon-Fri");
  });

  it("uses question mark for the unused Quartz day field", () => {
    expect(buildCronExpression(parts, "quartz")).toBe("0 0 9 ? * Mon-Fri");
    expect(coerceCronParts({ ...parts, dayOfMonth: "1", dayOfWeek: "*" }, "quartz")).toMatchObject({
      dayOfMonth: "1",
      dayOfWeek: "?",
    });
  });

  it("does not offer second-level templates for Linux", () => {
    expect(templatesForStandard("linux5").some((item) => item.parts.second !== "0")).toBe(false);
  });
});
