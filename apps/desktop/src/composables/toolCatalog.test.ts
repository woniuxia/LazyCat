import { describe, expect, it } from "vitest";
import { getAllTools, getSidebarItems, isRealToolId } from "./toolCatalog";

describe("toolCatalog retired workbenches", () => {
  it("removes retired workbenches while keeping API Mock", () => {
    const toolIds = getAllTools().map((tool) => tool.id);
    const groupIds = getSidebarItems()
      .filter((item) => item.kind === "group")
      .map((item) => item.group.id);

    expect(toolIds).not.toContain("api-workbench");
    expect(toolIds).not.toContain("db-workbench");
    expect(groupIds).not.toContain("database");
    expect(toolIds).toContain("api-mock");
    expect(isRealToolId("api-workbench")).toBe(false);
    expect(isRealToolId("db-workbench")).toBe(false);
    expect(isRealToolId("api-mock")).toBe(true);
  });
});
