import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./DataDictionaryPanel.vue", import.meta.url), "utf8");

describe("DataDictionaryPanel dictionary context menu", () => {
  it("uses a pinned all option in the dictionary list for global search", () => {
    expect(source).toContain('class="dd-dictionary-item dd-dictionary-all"');
    expect(source).toContain(":class=\"{ active: searchScope === 'all' }\"");
    expect(source).toContain('@click="selectAllDictionaries"');
    expect(source).toContain('const searchScope = ref<DataDictionarySearchScope>("all")');
    expect(source).toContain("async function selectAllDictionaries()");
  });

  it("does not render the toolbar scope segmented control", () => {
    expect(source).not.toContain("<el-segmented");
    expect(source).not.toContain("scopeOptions");
  });

  it("puts dictionary management actions in each dictionary item context menu", () => {
    expect(source).toContain('trigger="contextmenu"');
    expect(source).toContain(':ref="(el) => setDictionaryMenuRef(dictionary.id, el)"');
    expect(source).toContain('@visible-change="(visible) => handleDictionaryMenuVisibleChange(visible, dictionary.id)"');
    expect(source).toContain('@command="(command) => handleDictionaryCommand(command, dictionary)"');
    expect(source).toContain('command="replace"');
    expect(source).toContain('command="fields"');
    expect(source).toContain('command="rename"');
    expect(source).toContain('command="delete"');
  });

  it("does not render a separate dictionary actions area", () => {
    expect(source).not.toContain('class="dd-actions"');
    expect(source).not.toContain(".dd-actions");
  });

  it("closes other dictionary context menus before showing a new one", () => {
    expect(source).toContain("function handleDictionaryMenuVisibleChange");
    expect(source).toContain("closeOtherDictionaryMenus(id)");
    expect(source).toContain("function closeOtherDictionaryMenus(activeId: number)");
    expect(source).toContain("menu?.handleClose()");
  });

  it("keeps function-ref menu instances outside Vue reactivity", () => {
    expect(source).toContain("const dictionaryMenuRefs = new Map<number, DictionaryMenuInstance>()");
    expect(source).not.toContain("const dictionaryMenuRefs = ref");
    expect(source).not.toContain("dictionaryMenuRefs.value");
  });

  it("renders dictionary sort controls inside field configuration", () => {
    expect(source).toContain("排序字段");
    expect(source).toContain("排序方向");
    expect(source).toContain("fieldSortPath");
    expect(source).toContain("fieldSortDirection");
    expect(source).toContain("sortFieldPath");
    expect(source).toContain("sortDirection");
  });

  it("supports dragging dictionary items to persist sidebar order", () => {
    expect(source).toContain(':draggable="!savingDictionaryOrder"');
    expect(source).toContain('@dragstart="handleDictionaryDragStart(dictionary.id)"');
    expect(source).toContain('@dragover.prevent="handleDictionaryDragOver(dictionary.id)"');
    expect(source).toContain('@drop.prevent="handleDictionaryDrop(dictionary.id)"');
    expect(source).toContain('@dragend="handleDictionaryDragEnd"');
    expect(source).toContain('"tool:data-dictionary:reorder"');
  });

  it("keeps the all-dictionaries option outside drag sorting", () => {
    expect(source).not.toContain('dd-dictionary-all"\n          :draggable');
    expect(source).not.toContain('@dragstart="handleDictionaryDragStart(null)"');
  });

  it("copies summary part values without selecting the parent result", () => {
    expect(source).toContain('@click.stop="copySummaryValue(part.value)"');
    expect(source).toContain("async function copySummaryValue(value: string)");
    expect(source).toContain("await navigator.clipboard.writeText(value)");
    expect(source).toContain('ElMessage.error("复制失败")');
  });
});
