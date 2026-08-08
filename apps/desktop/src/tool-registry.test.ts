import { describe, expect, it } from "vitest";
import { getToolComponent } from "./tool-registry";

describe("tool registry conversion workbenches", () => {
  it("registers new workbench components and removes old component ids", () => {
    expect(getToolComponent("json-workbench")).toBeDefined();
    expect(getToolComponent("data-convert")).toBeDefined();
    expect(getToolComponent("data-dictionary")).toBeDefined();
    expect(getToolComponent("exception-stack")).toBeDefined();

    for (const toolId of [
      "json-process",
      "json-schema",
      "csv-json",
      "java-bean-js",
      "config-convert",
    ]) {
      expect(getToolComponent(toolId)).toBeUndefined();
    }
  });
});
