import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it } from "vitest";
import { workbenchTabState } from "./workbenchTabState";

const jsonSource = readFileSync(new URL("./JsonWorkbenchPanel.vue", import.meta.url), "utf8");
const dataSource = readFileSync(new URL("./DataConvertPanel.vue", import.meta.url), "utf8");
const jsonProcessSource = readFileSync(new URL("./JsonProcessPanel.vue", import.meta.url), "utf8");

afterEach(() => {
  workbenchTabState.json = "process";
  workbenchTabState.dataConvert = "csv";
});

describe("conversion workbench panels", () => {
  it("groups the existing panels under the requested tabs", () => {
    expect(jsonSource).toContain('label="处理与转换" name="process"');
    expect(jsonSource).toContain('label="JSON Schema" name="schema"');
    expect(jsonSource).toContain('label="数组过滤" name="array-filter"');
    expect(jsonSource).toContain("<JsonProcessPanel");
    expect(jsonSource).toContain("<JsonSchemaPanel");
    expect(jsonSource).toContain("<JsonArrayFilterPanel");

    expect(dataSource).toContain('label="CSV → JSON" name="csv"');
    expect(dataSource).toContain('label="JavaBean / JSON / JS" name="java-bean"');
    expect(dataSource).toContain('label="配置文件互转" name="config"');
    expect(dataSource).toContain("<CsvJsonPanel");
    expect(dataSource).toContain("<JavaBeanJsPanel");
    expect(dataSource).toContain("<ConfigConvertPanel");
  });

  it("defaults to the first tab and retains changes in module state", () => {
    expect(workbenchTabState.json).toBe("process");
    expect(workbenchTabState.dataConvert).toBe("csv");

    workbenchTabState.json = "schema";
    workbenchTabState.dataConvert = "config";

    expect(workbenchTabState.json).toBe("schema");
    expect(workbenchTabState.dataConvert).toBe("config");
    expect(jsonSource).toContain("ref<JsonWorkbenchTab>(workbenchTabState.json)");
    expect(dataSource).toContain("ref<DataConvertTab>(workbenchTabState.dataConvert)");
  });

  it("routes JSON input to the processing tab for mounted and first-open consumption", () => {
    expect(jsonSource).toContain('watchPendingInput("json-workbench"');
    expect(jsonSource).toContain('activeTab.value = "process"');
    expect(jsonSource).toContain("await nextTick()");
    expect(jsonSource).toContain("processPanelRef.value?.applyExternalInput(text)");
    expect(jsonProcessSource).toContain("defineExpose({ applyExternalInput })");
  });

  it("uses a bounded flex content area and a narrow-window vertical editor layout", () => {
    for (const source of [jsonSource, dataSource]) {
      expect(source).toContain("min-width: 0");
      expect(source).toContain("min-height: 0");
      expect(source).toContain("overflow: hidden");
    }
    expect(jsonProcessSource).toContain("@media (max-width: 900px)");
    expect(jsonProcessSource).toContain("grid-template-columns: 1fr");
    expect(jsonProcessSource).toContain("overflow: auto");
  });
});
