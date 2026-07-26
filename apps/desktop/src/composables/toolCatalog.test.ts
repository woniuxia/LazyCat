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

  it("removes the retired packet capture tool", () => {
    const toolIds = getAllTools().map((tool) => tool.id);

    expect(toolIds).not.toContain("capture");
    expect(isRealToolId("capture")).toBe(false);
  });
});

describe("toolCatalog release package", () => {
  it("registers the release package tool", () => {
    expect(getAllTools()).toContainEqual(
      expect.objectContaining({ id: "release-package", name: "上线包打包" }),
    );
    expect(isRealToolId("release-package")).toBe(true);
  });
});

describe("toolCatalog action center", () => {
  it("registers the action center as a real tool", () => {
    expect(getAllTools()).toContainEqual(
      expect.objectContaining({ id: "action-center", name: "动作中心" }),
    );
    expect(isRealToolId("action-center")).toBe(true);
  });
});
