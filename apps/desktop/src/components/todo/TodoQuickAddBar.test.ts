import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./TodoQuickAddBar.vue", import.meta.url), "utf-8");

describe("TodoQuickAddBar source structure", () => {
  it("creates through the shared payload builder and the todo item-create channel", () => {
    expect(source).toContain("buildQuickAddPayload");
    expect(source).toContain('"tool:todo:item-create"');
    expect(source).toContain('emit("created", response.id)');
  });

  it("ignores empty titles via the payload builder null gate", () => {
    expect(source).toMatch(/if\s*\(!payload\)\s*return/);
  });

  it("guards IME composition enters", () => {
    expect(source).toMatch(/if\s*\(event\.isComposing\)\s*return/);
    expect(source).toContain('@keydown.enter.exact.prevent="onTitleEnter"');
  });

  it("ignores re-entry while a create request is in flight", () => {
    expect(source).toMatch(/if\s*\(loading\.value\)\s*return/);
    expect(source).toContain("useToolInvoke()");
    expect(source).not.toContain("submitting");
  });

  it("clears only the title after success and keeps quick-pick values with focus retained", () => {
    expect(source).toMatch(
      /title\.value = "";\s*flashSuccess\(\);\s*emit\("created", response\.id\);\s*titleInputRef\.value\?\.focus\(\);/,
    );
  });

  it("resets title and both quick-pick values on escape", () => {
    expect(source).toContain('@keydown.esc="resetAll"');
    expect(source).toMatch(
      /function resetAll\(\)\s*\{[^}]*title\.value = "";[^}]*dateChoice\.value = null;[^}]*priorityOverride\.value = null;/,
    );
  });

  it("keeps user input on failure and does not duplicate the error toast", () => {
    expect(source).toMatch(/if\s*\(!response\)\s*return/);
    expect(source).not.toContain("ElMessage.error");
  });

  it("offers today/tomorrow/pick/clear date choices with a hidden date picker", () => {
    expect(source).toContain(">今天<");
    expect(source).toContain(">明天<");
    expect(source).toContain(">选日期…<");
    expect(source).toContain(">清除日期<");
    expect(source).toContain('value-format="YYYY-MM-DD"');
    expect(source).toContain("handleOpen");
  });

  it("renders P0-P3 priority choices and treats manual choice as an independent model", () => {
    expect(source).toContain('["P0", "P1", "P2", "P3"]');
    expect(source).toContain("priorityOverride.value ?? props.context.priorityDefault");
    expect(source).not.toMatch(/watch\([^)]*priorityDefault/);
  });

  it("uses the placeholder from the spec and shows a pending indicator", () => {
    expect(source).toContain('placeholder="添加任务，回车创建…"');
    expect(source).toContain('v-if="loading"');
  });

  it("cleans up the success flash timer on unmount", () => {
    expect(source).toContain("onBeforeUnmount");
    expect(source).toMatch(/clearTimeout\(flashTimer\)/);
  });
});
