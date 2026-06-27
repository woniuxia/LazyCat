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
    expect(source).toContain('command="rebuild"');
    expect(source).toContain('command="rename"');
    expect(source).toContain('command="delete"');
    expect(source).not.toContain('command="relations"');
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
    expect(source).toContain("主键字段");
    expect(source).toContain("标题字段");
    expect(source).toContain("fieldPrimaryPath");
    expect(source).toContain("fieldSortPath");
    expect(source).toContain("fieldSortDirection");
    expect(source).toContain("fieldTitlePath");
    expect(source).toContain("sortFieldPath");
    expect(source).toContain("sortDirection");
    expect(source).toContain("titleFieldPath");
  });

  it("renders relation configuration before the field lists", () => {
    expect(source).toContain("关系配置");
    expect(source).toContain("fieldRelationDrafts");
    expect(source).toContain("relationTargetPrimaryLabel");
    expect(source).toContain("duplicateRelationKeys");
    expect(source).toContain('"tool:data-dictionary:record-detail"');

    const relationIndex = source.indexOf('class="dd-relation-editor"');
    const fieldListIndex = source.indexOf('class="dd-field-sections"');
    expect(relationIndex).toBeGreaterThan(-1);
    expect(fieldListIndex).toBeGreaterThan(-1);
    expect(relationIndex).toBeLessThan(fieldListIndex);
  });

  it("uses a responsive field configuration drawer with a compact summary panel", () => {
    expect(source).toContain(':size="fieldDrawerSize"');
    expect(source).toContain('class="dd-field-drawer"');
    expect(source).toContain("const fieldDrawerSize = computed");
    expect(source).toContain('class="dd-field-config-panel"');
    expect(source).toContain('class="dd-field-config-summary"');
    expect(source).toContain("min(1040px, calc(100vw - 48px))");
  });

  it("supports dragging dictionary items to persist sidebar order", () => {
    expect(source).toContain('ref="dictionarySortListRef"');
    expect(source).toContain('class="dd-dictionary-drag-handle"');
    expect(source).toContain('import Sortable from "sortablejs"');
    expect(source).toContain('Sortable.create(listEl');
    expect(source).toContain('handle: ".dd-dictionary-drag-handle"');
    expect(source).toContain('draggable: ".dd-dictionary-menu"');
    expect(source).toContain("forceFallback: true");
    expect(source).toContain("async function handleDictionarySortEnd");
    expect(source).toContain('"tool:data-dictionary:reorder"');
    expect(source).not.toContain("@dragstart=");
    expect(source).not.toContain("@dragover.prevent=");
    expect(source).not.toContain("@drop.prevent=");
  });

  it("keeps the all-dictionaries option outside drag sorting", () => {
    expect(source).not.toContain('dd-dictionary-all"\n          :draggable');
    expect(source).not.toContain('@dragstart="handleDictionaryDragStart(null)"');
  });

  it("uses source labels as fallback titles without rendering a separate source marker", () => {
    expect(source).toContain("buildResultTitle");
    expect(source).toContain("resultTitle(item)");
    expect(source).toContain("dictionarySourceLabel(selectedItem)");
    expect(source).not.toContain("dd-result-source");
  });

  it("renders visible and hidden field configuration lists separately", () => {
    expect(source).toContain("展示字段");
    expect(source).toContain("非展示字段");
    expect(source).toContain("visibleFieldDrafts");
    expect(source).toContain("hiddenFieldDrafts");
    expect(source).toContain('class="dd-visible-field-list"');
    expect(source).toContain('class="dd-hidden-field-list"');
    expect(source).toContain("setFieldVisible");
    expect(source).toContain("setDataDictionaryFieldVisibility");

    const hiddenListStart = source.indexOf('class="dd-hidden-field-list"');
    const hiddenListEnd = source.indexOf("<template #footer>", hiddenListStart);
    expect(source.slice(hiddenListStart, hiddenListEnd)).not.toContain("dd-field-drag-handle");
  });

  it("supports dragging visible field rows to persist display order", () => {
    expect(source).toContain('@opened="initFieldSortable"');
    expect(source).toContain('ref="visibleFieldTableRef"');
    expect(source).toContain('row-key="fieldPath"');
    expect(source).toContain('class="dd-field-drag-handle"');
    expect(source).toContain("initFieldSortable");
    expect(source).toContain("handleFieldSortEnd");
    expect(source).toContain('handle: ".dd-field-drag-handle"');
    expect(source).toContain("moveDataDictionaryFieldDraft");
  });

  it("retries field drag initialization until table rows are rendered", () => {
    expect(source).toContain("function initFieldSortable(retries = 5)");
    expect(source).toContain("bodyEl.children.length === 0");
    expect(source).toContain("setTimeout(() => initFieldSortable(retries - 1), 120)");
  });

  it("copies summary part values without selecting the parent result", () => {
    expect(source).toContain('@click.stop="copySummaryValue(part.value)"');
    expect(source).toContain("async function copySummaryValue(value: string)");
    expect(source).toContain("await navigator.clipboard.writeText(value)");
    expect(source).toContain('ElMessage.error("复制失败")');
  });
});
