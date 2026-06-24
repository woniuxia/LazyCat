import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./TodoDetailView.vue", import.meta.url), "utf-8");

describe("TodoDetailView header layout", () => {
  it("keeps eyebrow and actions on the first row and title on its own row", () => {
    const headerStart = source.indexOf('<div class="detail-pane-header detail-pane-header--view">');
    const topRowStart = source.indexOf('<div class="detail-header-top">', headerStart);
    const eyebrowStart = source.indexOf('<div class="detail-eyebrow">事项详情</div>', topRowStart);
    const actionsStart = source.indexOf('<div class="detail-header-actions">', topRowStart);
    const titleRowStart = source.indexOf('<div class="detail-title-row">', topRowStart);
    const titleStart = source.indexOf('class="detail-title detail-title--copyable"', titleRowStart);

    expect(headerStart).toBeGreaterThanOrEqual(0);
    expect(topRowStart).toBeGreaterThan(headerStart);
    expect(eyebrowStart).toBeGreaterThan(topRowStart);
    expect(actionsStart).toBeGreaterThan(eyebrowStart);
    expect(titleRowStart).toBeGreaterThan(actionsStart);
    expect(titleStart).toBeGreaterThan(titleRowStart);
  });

  it("lets the view title row span the full header width", () => {
    expect(source).toMatch(/\.detail-pane-header--view\s*\{[^}]*flex-direction:\s*column;/s);
    expect(source).toMatch(/\.detail-pane-header--view\s*\{[^}]*align-items:\s*stretch;/s);
    expect(source).toMatch(/\.detail-title\s*\{[^}]*width:\s*100%;/s);
  });
});
