import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./VaultPanel.vue", import.meta.url), "utf-8");

function getRule(selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`${escapedSelector}\\s*\\{([^}]*)\\}`));
  expect(match, `missing CSS rule: ${selector}`).not.toBeNull();
  return match?.[1] ?? "";
}

describe("Vault navigation layout", () => {
  it("scrolls overflowing tags inside the remaining sidebar height", () => {
    expect(getRule(".vault-nav")).toContain("overflow: hidden");

    const tagSection = getRule(".vault-nav-section--tags");
    expect(tagSection).toContain("flex: 1");
    expect(tagSection).toContain("min-height: 0");
    expect(tagSection).toContain("overflow-y: auto");
  });

  it("keeps security actions outside the shrinkable tag area", () => {
    expect(source).not.toContain('class="vault-nav-spacer"');
    expect(getRule(".vault-nav-actions")).toContain("flex-shrink: 0");
  });
});
