import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const source = readFileSync(
  fileURLToPath(new URL("./RequestForwardPanel.vue", import.meta.url)),
  "utf8",
);
const listSource = readFileSync(
  fileURLToPath(
    new URL("./request-forward/RequestForwardRuleList.vue", import.meta.url),
  ),
  "utf8",
);
const formSource = readFileSync(
  fileURLToPath(
    new URL("./request-forward/RequestForwardRuleForm.vue", import.meta.url),
  ),
  "utf8",
);

describe("RequestForwardPanel source structure", () => {
  it("keeps running rules readonly and exposes stop-and-edit", () => {
    expect(source).toContain("停止并编辑");
    expect(source).toContain("handleStopAndEdit");
    expect(formSource).toContain(':disabled="readonly"');
  });

  it("separates save from save-and-start", () => {
    expect(source).toContain("仅保存");
    expect(source).toContain("保存并启动");
    expect(source).toContain("saveRule");
    expect(source).toContain("startRule");
  });

  it("provides single-rule and batch start-stop controls", () => {
    expect(listSource).toMatch(/emit\(["']start["']/);
    expect(listSource).toMatch(/emit\(["']stop["']/);
    expect(listSource).toMatch(/emit\(["']start-all["']/);
    expect(listSource).toMatch(/emit\(["']stop-all["']/);
  });

  it("warns when a listener is exposed beyond loopback", () => {
    expect(formSource).toContain("当前监听地址可被其他设备访问");
    expect(formSource).toContain("isExposedForwardBindHost");
  });

  it("confirms deletion and keeps persisted protocols immutable", () => {
    expect(source).toContain("ElMessageBox.confirm");
    expect(source).toContain("删除后无法恢复");
    expect(formSource).toContain(':disabled="persisted"');
  });

  it("polls only for active runtime states and clears the timer", () => {
    expect(source).toContain("hasActiveRuntimeRule");
    expect(source).toContain("setInterval");
    expect(source).toContain("2_000");
    expect(source).toContain("onUnmounted");
    expect(source).toContain("clearInterval");
  });
});
