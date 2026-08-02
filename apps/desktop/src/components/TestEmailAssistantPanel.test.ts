import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./TestEmailAssistantPanel.vue", import.meta.url), "utf8");

describe("TestEmailAssistantPanel source structure", () => {
  it("supports template inspection, dynamic fields, and the two output actions", () => {
    expect(source).toContain("tool:test-email-assistant:inspect-template");
    expect(source).toContain("tool:test-email-assistant:generate-document");
    expect(source).toContain("v-for=\"name in allPlaceholders\"");
    expect(source).toContain("isMultilineFieldName(name)");
    expect(source).toContain(":autosize=\"isMultilineFieldName(name)");
    expect(source).toContain("navigator.clipboard.writeText(emailPreview.value)");
    expect(source).toContain("tool:system:reveal-in-folder");
  });

  it("keeps cancellation quiet and exposes real failures", () => {
    expect(source).toContain("if (!selected) return");
    expect(source).toContain("errorMessage.value = error instanceof Error ? error.message : String(error)");
    expect(source).toContain("role=\"alert\"");
  });

  it("only adds newly visible fields and keeps hidden values in the session", () => {
    expect(source).toContain("if (!(name in values)) values[name] = \"\";");
    expect(source).not.toContain("delete values");
  });

  it("leaves output naming to the backend and shows the generated path", () => {
    expect(source).not.toContain("建议文件名");
    expect(source).not.toContain("buildSuggestedDocumentFileName");
    expect(source).toContain(":title=\"outputPath\"");
  });

  it("uses compact responsive layout with long-content wrapping", () => {
    expect(source).toContain("overflow-wrap: anywhere");
    expect(source).toContain("@media (max-width: 760px)");
    expect(source).toContain("grid-template-columns: minmax(260px, 0.92fr) minmax(0, 1.08fr)");
  });
});
