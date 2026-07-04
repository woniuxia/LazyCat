import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./JwtPanel.vue", import.meta.url), "utf8");

describe("JwtPanel source structure", () => {
  it("renders header and payload through read-only JSON trees", () => {
    expect(source).toContain('import JsonTreeViewer from "./common/JsonTreeViewer.vue"');
    expect(source.split("<JsonTreeViewer").length - 1).toBe(2);
    expect(source).toContain(':value="decoded.headerValue"');
    expect(source).toContain(':value="decoded.payloadValue"');
    expect(source).toContain(':show-search="false"');
    expect(source).not.toContain("editable");
  });

  it("keeps raw decoded objects alongside formatted copy text", () => {
    expect(source).toContain("headerValue: data.header");
    expect(source).toContain("payloadValue: data.payload");
    expect(source).toContain(':copy-text="decoded.header"');
    expect(source).toContain(':copy-text="decoded.payload"');
  });

  it("keeps the signature block and expiry tag untouched", () => {
    expect(source).toContain('class="jwt-section-content jwt-signature"');
    expect(source).toContain('decoded.expired ? "已过期" : "未过期"');
  });
});
