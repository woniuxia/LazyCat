import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./DataDictionaryPanel.vue", import.meta.url), "utf8");
const appSource = readFileSync(new URL("../App.vue", import.meta.url), "utf8");
const handoffSource = readFileSync(
  new URL("../utils/navigation-handoff.ts", import.meta.url),
  "utf8",
);

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
    expect(source).toContain(
      '@visible-change="(visible) => handleDictionaryMenuVisibleChange(visible, dictionary.id)"',
    );
    expect(source).toContain(
      '@command="(command) => handleDictionaryCommand(command, dictionary)"',
    );
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
    expect(source).toContain(
      "const dictionaryMenuRefs = new Map<number, DictionaryMenuInstance>()",
    );
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
    expect(source).toContain(
      'class="dd-dictionary-meta dd-dictionary-count dd-dictionary-drag-handle"',
    );
    expect(source).toContain("拖拽右侧条数排序");
    expect(source).not.toContain('<el-icon class="dd-dictionary-drag-handle"');
    expect(source).toContain('import Sortable from "sortablejs"');
    expect(source).toContain("Sortable.create(listEl");
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

  it("supports importing large data dictionary JSON by file path", () => {
    expect(source).toContain('import { open } from "@tauri-apps/plugin-dialog"');
    expect(source).toContain("async function selectImportFile()");
    expect(source).toContain("inputPath");
    expect(source).toContain("buildImportPayload()");
  });

  it("uses source labels as fallback titles without rendering a separate source marker", () => {
    expect(source).toContain("buildResultTitle");
    expect(source).toContain("resultTitle(item)");
    expect(source).toContain("const detailTitle = computed");
    expect(source).not.toContain("dd-result-source");
  });

  it("renders the record detail header as one computed title without a source subtitle", () => {
    expect(source).toContain("<h3>{{ detailTitle }}</h3>");
    expect(source).toContain("const detailTitle = computed");
    expect(source).toContain("return recordDetail.value.record.title;");
    expect(source).toContain(
      'return selectedItem.value ? resultTitle(selectedItem.value) : "未选择记录";',
    );
    expect(source).not.toContain("recordDetail.record.dictionaryName }} #");
    expect(source).not.toContain(
      'v-else-if="selectedItem">{{ selectedItem.dictionaryName }}</span>',
    );
  });

  it("uses JsonTreeViewer for the raw JSON detail copy and folding actions", () => {
    expect(source).toContain('import JsonTreeViewer from "./common/JsonTreeViewer.vue"');
    expect(source).toContain('class="dd-json-shell"');
    expect(source).toContain("<JsonTreeViewer");
    expect(source).toContain('class="dd-json-view"');
    expect(source).toContain(':value="recordDetail.record.rawJson"');
    expect(source).toContain(':copy-text="selectedJson"');
    expect(source).toContain('default-expand-depth="all"');
    expect(source).not.toContain('class="dd-json-copy-btn"');
    expect(source).not.toContain('@click="copySelectedJson"');
    expect(source).not.toContain("async function copySelectedJson()");
  });

  it("renders relation groups below the raw JSON detail", () => {
    const detailStart = source.indexOf('class="dd-detail"');
    const detailEnd = source.indexOf("<el-dialog", detailStart);
    const detailSource = source.slice(detailStart, detailEnd);

    const jsonIndex = detailSource.indexOf('class="dd-json-shell"');
    const relationIndex = detailSource.indexOf('class="dd-relation-groups"');
    expect(jsonIndex).toBeGreaterThan(-1);
    expect(relationIndex).toBeGreaterThan(-1);
    expect(jsonIndex).toBeLessThan(relationIndex);
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

  it("blocks field configuration save until fields finish loading", () => {
    expect(source).toContain("const fieldLoading = ref(false)");
    expect(source).toContain('v-loading="fieldLoading"');
    expect(source).toContain(':disabled="fieldLoading || !fieldDrafts.length"');
    expect(source).toContain("fieldLoading.value = true");
    expect(source).toContain("fieldLoading.value = false");
    expect(source).toContain('ElMessage.warning("字段还在加载，请稍后保存")');
  });

  it("renders search failures as a retryable result panel state", () => {
    expect(source).toContain('const searchError = ref("")');
    expect(source).toContain('v-if="searchError"');
    expect(source).toContain("搜索失败");
    expect(source).toContain('@click="runSearch"');
    expect(source).toContain('searchError.value = (error as Error).message || "搜索失败"');
    expect(source).toContain("searchItems.value = []");
  });

  it("uses contextual empty states for dictionary and result states", () => {
    expect(source).toContain("const resultEmptyTitle = computed");
    expect(source).toContain("const resultEmptyDescription = computed");
    expect(source).toContain("const resultEmptyActionText = computed");
    expect(source).toContain("openContextualEmptyAction");
    expect(source).toContain("暂无字典");
    expect(source).toContain("当前字典没有记录");
    expect(source).toContain("未找到匹配记录");
  });

  it("requires primary field selection when creating a dictionary", () => {
    expect(source).toContain('const importPrimaryPath = ref("")');
    expect(source).toContain("primaryFieldPath: importPrimaryPath.value");
    expect(source).toContain('v-model="importPrimaryPath"');
    expect(source).toContain("选择用于唯一定位记录的字段");
  });

  it("loads popular records separately from keyword search", () => {
    expect(source).toContain("tool:data-dictionary:popular-records");
    expect(source).toContain("tool:data-dictionary:mark-record-used");
    expect(source).toContain("mergePopularAndSearchItems");
    expect(source).toContain("pickInitialRecordItem");
  });

  it("does not mark the auto-selected first record as used", () => {
    expect(source).toContain("selectSearchItem(initialItem, { markUsed: false })");
    expect(source).toContain("options: { markUsed?: boolean } = {}");
    expect(source).toContain("if (options.markUsed !== false)");
  });

  it("guards primary key pruning with explicit confirmation", () => {
    expect(source).toContain("confirmPrimaryPrune");
    expect(source).toContain("PRIMARY_PRUNE_CONFIRMATION_REQUIRED");
    expect(source).toContain("确认更换主键");
  });

  it("copies summary part values without selecting the parent result", () => {
    expect(source).toContain('@click.stop="copySummaryValue(part.value)"');
    expect(source).toContain("async function copySummaryValue(value: string)");
    expect(source).toContain("await navigator.clipboard.writeText(value)");
    expect(source).toContain('ElMessage.error("复制失败")');
  });

  it("copies detail summary values from the match list", () => {
    const matchListStart = source.indexOf('class="dd-match-list"');
    const matchListEnd = source.indexOf('class="dd-relation-groups"', matchListStart);
    const matchListSource = source.slice(matchListStart, matchListEnd);

    expect(matchListSource).toContain('v-for="part in recordDetail.record.summary"');
    expect(matchListSource).toContain('title="点击复制"');
    expect(matchListSource).toContain('@click.stop="copySummaryValue(part.value)"');
    expect(source).toContain(".dd-match-tag:hover");
  });

  it("consumes Spotlight focus requests without mutating the search keyword", () => {
    expect(source).toContain("useDataDictionaryNavigation");
    expect(source).toContain("consumeDataDictionaryFocus");
    expect(source).toContain("focusDataDictionaryRecord");
    expect(source).not.toContain("keyword.value = String(focus.recordId)");
  });

  it("routes hotkey navigation to data dictionary focus requests", () => {
    expect(handoffSource).toContain('payload.target === "data-dictionary"');
    expect(appSource).toContain("useDataDictionaryNavigation");
    expect(appSource).toContain("requestFocus(intent.focus.itemId)");
  });

  it("renders related record titles as one title without an extra source subtitle", () => {
    const relatedStart = source.indexOf('class="dd-related-record"');
    const relatedEnd = source.indexOf('<div v-if="!group.items.length"', relatedStart);
    const relatedSource = source.slice(relatedStart, relatedEnd);

    expect(relatedSource).toContain("<strong>{{ item.title }}</strong>");
    expect(relatedSource).not.toContain("{{ item.dictionaryName }} #{{ item.rowIndex + 1 }}");
    expect(relatedSource).not.toContain("<span>");
    expect(source).not.toContain(".dd-related-record span");
  });
});
