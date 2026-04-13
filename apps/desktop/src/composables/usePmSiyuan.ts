import { ref, reactive, computed, watch, type Ref, type ComputedRef } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { getSetting, getSettingJson, setSetting, setSettingJson } from "./useSettings";
import type {
  PmSiyuanLocation,
  PmSiyuanPageRef,
  PmSiyuanNotebookDirectory,
  PmSiyuanTreeNode,
  PmSiyuanDirectoryResult,
  PmSiyuanSearchResult,
  PmItem,
} from "../types/pm";
import {
  addPmSiyuanExtraPage,
  collectPmSiyuanExpandedKeys,
  collectPmSiyuanPagesForLocation,
  filterPmSiyuanDirectory,
  filterPmSiyuanPages,
  formatPmSiyuanLocationLabel,
  formatPmSiyuanLocationPathLabel,
  formatPmSiyuanLocationTargetLabel,
  isPmSiyuanNotebookDirectory,
  removePmSiyuanPage,
  resolvePmSiyuanEffectiveLocation,
  setPmSiyuanPrimaryPage,
} from "../utils/pmSiyuan";
import { normalizePmDateRangeForDraft } from "../utils/pmDate";

export type PmSiyuanPageLocationState =
  | "ready"
  | "missing-location"
  | "missing-config"
  | "load-error"
  | "invalid-location"
  | "empty";

export interface ItemSiyuanLinkedRow {
  page: PmSiyuanPageRef;
  kind: "primary" | "extra";
}

interface ItemFormData {
  title: string;
  description: string;
  status: string;
  priority: string;
  startAt: string | null;
  endAt: string | null;
}

const DEFAULT_BASE_URL = "http://127.0.0.1:6806";
const PM_SIYUAN_DEFAULT_LOCATION_KEY = "pm_siyuan_default_location";

export function usePmSiyuan(deps: {
  dialogProjectSiyuanOverride: ComputedRef<PmSiyuanLocation | null>;
  editingItem: Ref<PmItem | null>;
  itemForm: ComputedRef<ItemFormData>;
  dialogProjectName: ComputedRef<string>;
  itemPrimaryPage: Ref<PmSiyuanPageRef | null>;
  itemExtraPages: Ref<PmSiyuanPageRef[]>;
  getProjectFormSiyuanOverride: () => PmSiyuanLocation | null;
  setProjectFormSiyuanOverride: (useOverride: boolean, location: PmSiyuanLocation | null) => void;
}) {
  // ── Config state ───────────────────────────────────────
  const drawerVisible = ref(false);
  const form = reactive({
    baseUrl: getSetting("pm_siyuan_base_url") ?? DEFAULT_BASE_URL,
    token: getSetting("pm_siyuan_token") ?? "",
  });
  const globalSiyuanLocation = ref<PmSiyuanLocation | null>(
    getSettingJson<PmSiyuanLocation | null>(PM_SIYUAN_DEFAULT_LOCATION_KEY, null),
  );
  const globalSiyuanLocationDraft = ref<PmSiyuanLocation | null>(
    globalSiyuanLocation.value ? { ...globalSiyuanLocation.value } : null,
  );
  const showToken = ref(false);
  const testing = ref(false);
  const testingVersion = ref("");
  const loadingDirectory = ref(false);
  const directory = ref<PmSiyuanNotebookDirectory[]>([]);
  const directoryFetchedAt = ref("");
  let directoryLoadPromise: Promise<boolean> | null = null;
  const error = ref("");
  const errorContext = ref<"test" | "directory" | null>(null);
  const treeProps = { label: "name", children: "children" };

  // ── Location picker state ──────────────────────────────
  const locationDialogVisible = ref(false);
  const locationPickerTarget = ref<"global" | "project">("global");
  const locationPickerValue = ref<PmSiyuanLocation | null>(null);
  const locationPickerSearch = ref("");

  // ── Page picker state ──────────────────────────────────
  const pageDialogVisible = ref(false);
  const pageDialogMode = ref<"primary" | "extra">("primary");
  const pageDialogIntent = ref<"link" | "replace-primary">("link");
  const pageDialogSessionId = ref(0);
  const pageFilterKeyword = ref("");
  const pageResultSource = ref<"location" | "all">("location");
  const pageLocationResults = ref<PmSiyuanPageRef[]>([]);
  const pageAllResults = ref<PmSiyuanPageRef[]>([]);
  const pageSearchingAll = ref(false);
  const pageLocationState = ref<PmSiyuanPageLocationState>("ready");
  const pageLocationRefreshError = ref("");
  const pageCreating = ref(false);

  // ── Computed: item effective location ──────────────────
  const itemEffectiveLocation = computed(() =>
    resolvePmSiyuanEffectiveLocation(deps.dialogProjectSiyuanOverride.value, globalSiyuanLocation.value),
  );

  // ── Computed: config ───────────────────────────────────
  const errorTitle = computed(() => {
    if (errorContext.value === "test") return "连接失败";
    if (errorContext.value === "directory") return "目录加载失败";
    return "错误";
  });
  const configReady = computed(() => Boolean(getConfigSnapshot()));

  // ── Computed: location picker ──────────────────────────
  const locationPickerTitle = computed(() =>
    locationPickerTarget.value === "global" ? "选择任务默认存储位置" : "选择项目专属存储位置",
  );
  const locationPickerSearchKeyword = computed(() => locationPickerSearch.value.trim());
  const locationPickerTreeData = computed(() => {
    if (!locationPickerSearchKeyword.value) return directory.value;
    return filterPmSiyuanDirectory(directory.value, locationPickerSearchKeyword.value);
  });
  const locationPickerExpandedKeys = computed(() => {
    if (!locationPickerSearchKeyword.value) return [];
    return collectPmSiyuanExpandedKeys(locationPickerTreeData.value);
  });
  const locationPickerTreeKey = computed(
    () =>
      `${locationPickerTarget.value}:${locationPickerSearchKeyword.value}:${
        locationPickerExpandedKeys.value.join("|")
      }:${locationPickerValue.value?.parentDocId ?? locationPickerValue.value?.notebookId ?? "none"}`,
  );
  const locationPickerCurrentNodeKey = computed(
    () => locationPickerValue.value?.parentDocId ?? locationPickerValue.value?.notebookId ?? undefined,
  );
  const locationPickerSelectionTarget = computed(() =>
    formatPmSiyuanLocationTargetLabel(locationPickerValue.value),
  );
  const locationPickerSelectionPath = computed(() =>
    formatPmSiyuanLocationPathLabel(locationPickerValue.value),
  );
  const locationPickerStatusText = computed(() =>
    locationPickerSearchKeyword.value
      ? `已按"${locationPickerSearchKeyword.value}"过滤目录，只保留命中的目录路径。`
      : "默认仅展开笔记本一级，点击文档后会把新页面放到该文档下面。",
  );

  // ── Computed: page picker ──────────────────────────────
  const pageFilterKeywordTrimmed = computed(() => pageFilterKeyword.value.trim());
  const pageFilteredLocationResults = computed(() =>
    filterPmSiyuanPages(pageLocationResults.value, pageFilterKeyword.value),
  );
  const pageDisplayedResults = computed(() =>
    pageResultSource.value === "all" ? pageAllResults.value : pageFilteredLocationResults.value,
  );
  const pageDialogInputPlaceholder = computed(() =>
    itemEffectiveLocation.value && configReady.value
      ? "输入标题或路径过滤当前列表"
      : "输入关键词后点击扩展到全库",
  );
  const pageCreateTitle = computed(() => {
    const itemTitle = deps.itemForm.value.title.trim();
    if (!itemEffectiveLocation.value) return "";
    if (
      pageFilterKeywordTrimmed.value &&
      !pageShowLocationLoading.value &&
      !pageShowAllLoading.value &&
      pageDisplayedResults.value.length === 0
    ) {
      return pageFilterKeywordTrimmed.value;
    }
    return itemTitle;
  });
  const pageCanCreateImmediately = computed(() => {
    if (!itemEffectiveLocation.value || !pageCreateTitle.value) return false;
    if (
      pageLocationState.value === "missing-location" ||
      pageLocationState.value === "invalid-location" ||
      pageLocationState.value === "missing-config"
    ) return false;
    if (pageLocationState.value === "load-error" && pageLocationResults.value.length === 0) return false;
    return true;
  });
  const pageDialogTitle = computed(() => {
    if (pageDialogIntent.value === "replace-primary") return "更换思源主页面";
    return pageDialogMode.value === "primary" ? "关联思源主页面" : "添加思源附加页面";
  });
  const pageCurrentRangeText = computed(() => {
    if (pageResultSource.value === "all") {
      return `当前列表范围：本次全库搜索结果（当前显示 ${pageAllResults.value.length} 条）`;
    }
    if (itemEffectiveLocation.value) {
      return `当前列表范围：${formatPmSiyuanLocationLabel(itemEffectiveLocation.value)}（共 ${pageLocationResults.value.length} 篇）`;
    }
    return "当前列表范围：未配置当前位置";
  });
  const pageFilterSummary = computed(() => {
    if (pageResultSource.value === "all") {
      return pageFilterKeywordTrimmed.value ? `当前关键词：${pageFilterKeywordTrimmed.value}` : "";
    }
    if (pageLocationState.value !== "ready") return "";
    if (!pageFilterKeywordTrimmed.value) return "";
    return `当前过滤命中 ${pageFilteredLocationResults.value.length} 条，完整列表共 ${pageLocationResults.value.length} 篇。`;
  });
  const pageShowReturnToLocation = computed(
    () => pageResultSource.value === "all" && Boolean(itemEffectiveLocation.value),
  );
  const pageShowLocationLoading = computed(
    () =>
      pageResultSource.value === "location" &&
      loadingDirectory.value &&
      pageLocationResults.value.length === 0 &&
      pageLocationState.value === "ready" &&
      configReady.value &&
      Boolean(itemEffectiveLocation.value),
  );
  const pageShowAllLoading = computed(
    () => pageResultSource.value === "all" && pageSearchingAll.value && pageAllResults.value.length === 0,
  );
  const pageEmptyMessage = computed(() => {
    if (pageResultSource.value === "all") {
      return pageSearchingAll.value ? "" : "全库中没有找到匹配文档，请调整关键词后重试。";
    }
    switch (pageLocationState.value) {
      case "missing-location":
        return "当前未配置项目专属位置或全局默认位置，无法展示当前位置列表；你仍可输入关键词后手动扩展到全库搜索。";
      case "missing-config":
        return "当前缺少思源服务地址或 API Token，请先完成思源配置。";
      case "load-error":
        return "当前位置列表加载失败，请稍后重试。";
      case "invalid-location":
        return "当前默认位置已失效，或所在笔记本已关闭，请重新选择位置。";
      case "empty":
        return "当前位置暂无可关联文档，可以直接新建页面。";
      case "ready":
        return pageFilteredLocationResults.value.length === 0 ? "当前过滤条件下没有匹配文档。" : "";
      default:
        return "";
    }
  });

  // ── Helpers ────────────────────────────────────────────
  function cloneLocation(location: PmSiyuanLocation | null | undefined): PmSiyuanLocation | null {
    return location ? { ...location } : null;
  }
  function clonePages(pages: PmSiyuanPageRef[] | null | undefined): PmSiyuanPageRef[] {
    return (pages ?? []).map((p) => ({ ...p }));
  }

  function normalizeBaseUrl(value: string): string {
    let url = value.trim();
    if (!url) return "";
    if (!/^https?:\/\//i.test(url)) url = `http://${url}`;
    while (url.endsWith("/")) url = url.slice(0, -1);
    return url;
  }

  function getConfigSnapshot(): { baseUrl: string; token: string } | null {
    const baseUrl = normalizeBaseUrl(form.baseUrl);
    const token = (form.token ?? "").trim();
    if (!baseUrl || !token) return null;
    return { baseUrl, token };
  }

  function ensureConfig(): { baseUrl: string; token: string } {
    const baseUrl = normalizeBaseUrl(form.baseUrl);
    if (!baseUrl) throw new Error("请填写思源服务地址");
    const token = (form.token ?? "").trim();
    if (!token) throw new Error("请填写 API Token");
    return { baseUrl, token };
  }

  // ── Directory ──────────────────────────────────────────
  async function refreshDirectory(options: { showSuccess?: boolean } = {}): Promise<boolean> {
    if (directoryLoadPromise) return directoryLoadPromise;
    const { showSuccess = true } = options;
    directoryLoadPromise = (async () => {
      try {
        const { baseUrl, token } = ensureConfig();
        loadingDirectory.value = true;
        error.value = "";
        errorContext.value = null;
        const result = (await invokeToolByChannel("tool:pm:siyuan-directory", { baseUrl, token })) as PmSiyuanDirectoryResult;
        directory.value = result?.notebooks ?? [];
        directoryFetchedAt.value = result?.fetchedAt
          ? (typeof result.fetchedAt === "string" ? result.fetchedAt : new Date().toLocaleString())
          : new Date().toLocaleString();
        if (showSuccess) ElMessage.success("目录已加载");
        return true;
      } catch (err) {
        error.value = (err as Error).message;
        errorContext.value = "directory";
        return false;
      } finally {
        loadingDirectory.value = false;
        directoryLoadPromise = null;
      }
    })();
    return directoryLoadPromise;
  }

  async function ensureDirectoryLoaded() {
    if (directory.value.length > 0) return;
    await refreshDirectory({ showSuccess: false });
  }

  // ── Config functions (D10) ─────────────────────────────
  function openDrawer() { drawerVisible.value = true; }

  function saveConfig() {
    try {
      const { baseUrl, token } = ensureConfig();
      setSetting("pm_siyuan_base_url", baseUrl);
      setSetting("pm_siyuan_token", token);
      setSettingJson(PM_SIYUAN_DEFAULT_LOCATION_KEY, globalSiyuanLocationDraft.value);
      form.baseUrl = baseUrl;
      form.token = token;
      globalSiyuanLocation.value = cloneLocation(globalSiyuanLocationDraft.value);
      ElMessage.success("配置已保存");
    } catch (err) {
      ElMessage.error((err as Error).message);
    }
  }

  async function testConnection() {
    try {
      const { baseUrl, token } = ensureConfig();
      testing.value = true;
      error.value = "";
      errorContext.value = null;
      const result = (await invokeToolByChannel("tool:pm:siyuan-test", { baseUrl, token })) as { version?: string };
      testingVersion.value = result.version ?? "未知版本";
      ElMessage.success("连接成功");
    } catch (err) {
      testingVersion.value = "";
      error.value = (err as Error).message;
      errorContext.value = "test";
    } finally {
      testing.value = false;
    }
  }

  async function loadDirectory() {
    await refreshDirectory({ showSuccess: true });
  }

  function refreshLocationPickerDirectory() {
    void refreshDirectory({ showSuccess: true });
  }

  // ── Location tree helpers ──────────────────────────────
  function buildLocationFromTreeNode(
    data: PmSiyuanNotebookDirectory | PmSiyuanTreeNode,
    node: { level: number; parent?: { level: number; data?: unknown; parent?: unknown } | null },
  ): PmSiyuanLocation | null {
    if (isPmSiyuanNotebookDirectory(data)) {
      if (data.closed) return null;
      return {
        notebookId: data.id, notebookName: data.name,
        parentDocId: null, parentDocTitle: null, parentHpath: null, parentPath: null,
      };
    }
    let current = node.parent ?? null;
    while (current && current.level > 1) {
      current = (current.parent as typeof current | null) ?? null;
    }
    const notebook = current?.data as PmSiyuanNotebookDirectory | undefined;
    if (!notebook || notebook.closed) return null;
    return {
      notebookId: notebook.id, notebookName: notebook.name,
      parentDocId: data.id, parentDocTitle: data.name,
      parentHpath: data.hpath, parentPath: data.path,
    };
  }

  function isLocationPickerNodeSelected(data: PmSiyuanNotebookDirectory | PmSiyuanTreeNode) {
    return locationPickerCurrentNodeKey.value === data.id;
  }

  function isLocationPickerNodeDisabled(
    data: PmSiyuanNotebookDirectory | PmSiyuanTreeNode,
    node: { level: number; parent?: { level: number; data?: unknown; parent?: unknown } | null },
  ) {
    return buildLocationFromTreeNode(data, node) === null;
  }

  // ── Location picker functions (D11) ────────────────────
  async function openLocationPicker(target: "global" | "project") {
    locationPickerTarget.value = target;
    locationPickerSearch.value = "";
    locationPickerValue.value = cloneLocation(
      target === "global" ? globalSiyuanLocationDraft.value : deps.getProjectFormSiyuanOverride(),
    );
    locationDialogVisible.value = true;
    if (directory.value.length === 0) {
      await ensureDirectoryLoaded();
      return;
    }
    void refreshDirectory({ showSuccess: false });
  }

  function handleLocationTreeNodeClick(
    data: PmSiyuanNotebookDirectory | PmSiyuanTreeNode,
    node: { level: number; parent?: { level: number; data?: unknown; parent?: unknown } | null },
  ) {
    const loc = buildLocationFromTreeNode(data, node);
    if (!loc) {
      ElMessage.warning("关闭的笔记本不能作为默认存储位置");
      return;
    }
    locationPickerValue.value = loc;
  }

  function applyLocationPicker() {
    const loc = cloneLocation(locationPickerValue.value);
    if (locationPickerTarget.value === "global") {
      globalSiyuanLocationDraft.value = loc;
    } else {
      deps.setProjectFormSiyuanOverride(Boolean(loc), loc);
    }
    locationDialogVisible.value = false;
  }

  function clearLocationPicker() { locationPickerValue.value = null; }

  function clearProjectSiyuanOverride() { deps.setProjectFormSiyuanOverride(false, null); }

  // ── Item page management ───────────────────────────────
  function applyItemPrimaryPage(page: PmSiyuanPageRef | null) {
    const result = setPmSiyuanPrimaryPage(deps.itemPrimaryPage.value, deps.itemExtraPages.value, page);
    deps.itemPrimaryPage.value = result.primaryPage ? { ...result.primaryPage } : null;
    deps.itemExtraPages.value = clonePages(result.extraPages);
  }

  function addItemExtraPage(page: PmSiyuanPageRef) {
    deps.itemExtraPages.value = addPmSiyuanExtraPage(
      deps.itemPrimaryPage.value, deps.itemExtraPages.value, page,
    ).map((item) => ({ ...item }));
  }

  function hasItemLinkedPage(docId: string): boolean {
    return deps.itemPrimaryPage.value?.docId === docId ||
      deps.itemExtraPages.value.some((p) => p.docId === docId);
  }

  function removeItemLinkedPage(docId: string) {
    const result = removePmSiyuanPage(deps.itemPrimaryPage.value, deps.itemExtraPages.value, docId);
    deps.itemPrimaryPage.value = result.primaryPage ? { ...result.primaryPage } : null;
    deps.itemExtraPages.value = clonePages(result.extraPages);
  }

  // ── Page picker functions (D12) ────────────────────────
  function resetPageDialogState(mode: "primary" | "extra") {
    pageDialogSessionId.value += 1;
    pageDialogMode.value = mode;
    pageDialogIntent.value = "link";
    pageFilterKeyword.value = "";
    pageResultSource.value = "location";
    pageLocationResults.value = [];
    pageAllResults.value = [];
    pageSearchingAll.value = false;
    pageLocationState.value = "ready";
    pageLocationRefreshError.value = "";
  }

  function applyPageLocationResultsFromDirectory() {
    const loc = itemEffectiveLocation.value;
    if (!configReady.value) {
      pageLocationResults.value = [];
      pageLocationState.value = "missing-config";
      pageLocationRefreshError.value = "";
      return;
    }
    if (!loc) {
      pageLocationResults.value = [];
      pageLocationState.value = "missing-location";
      pageLocationRefreshError.value = "";
      return;
    }
    const result = collectPmSiyuanPagesForLocation(directory.value, loc);
    pageLocationResults.value = clonePages(result.pages);
    pageLocationState.value = result.state;
    pageLocationRefreshError.value = "";
  }

  async function refreshPageLocationResults(options: { keepResultsOnError?: boolean; sessionId?: number } = {}) {
    const { keepResultsOnError = false, sessionId = pageDialogSessionId.value } = options;
    if (!configReady.value) {
      pageLocationResults.value = [];
      pageLocationState.value = "missing-config";
      pageLocationRefreshError.value = "";
      return;
    }
    if (!itemEffectiveLocation.value) {
      pageLocationResults.value = [];
      pageLocationState.value = "missing-location";
      pageLocationRefreshError.value = "";
      return;
    }
    const success = await refreshDirectory({ showSuccess: false });
    if (sessionId !== pageDialogSessionId.value) return;
    if (success) {
      applyPageLocationResultsFromDirectory();
      return;
    }
    if (!keepResultsOnError) {
      pageLocationResults.value = [];
      pageLocationState.value = "load-error";
    }
    pageLocationRefreshError.value = error.value || "当前位置列表加载失败，请稍后重试。";
  }

  async function openPageDialog(mode: "primary" | "extra", intent: "link" | "replace-primary" = "link") {
    resetPageDialogState(mode);
    pageDialogIntent.value = intent;
    if (!deps.editingItem.value && mode === "primary") {
      pageFilterKeyword.value = deps.itemForm.value.title.trim();
    }
    const sessionId = pageDialogSessionId.value;
    pageDialogVisible.value = true;
    if (!configReady.value) {
      pageLocationState.value = "missing-config";
      return;
    }
    if (!itemEffectiveLocation.value) {
      pageLocationState.value = "missing-location";
      return;
    }
    if (directory.value.length > 0) {
      applyPageLocationResultsFromDirectory();
      void refreshPageLocationResults({ keepResultsOnError: true, sessionId });
      return;
    }
    await refreshPageLocationResults({ sessionId });
  }

  function restoreLocationResults() {
    pageResultSource.value = "location";
    pageAllResults.value = [];
  }

  async function expandPagesToAll() {
    const keyword = pageFilterKeywordTrimmed.value;
    const sessionId = pageDialogSessionId.value;
    if (keyword.length < 2) {
      ElMessage.warning("请输入至少 2 个字符后再扩展到全库");
      return;
    }
    try { ensureConfig(); } catch (err) {
      ElMessage.warning((err as Error).message);
      return;
    }
    try {
      pageSearchingAll.value = true;
      const result = (await invokeToolByChannel("tool:pm:siyuan-search-pages", {
        keyword, searchAll: true, location: null,
      })) as PmSiyuanSearchResult;
      if (sessionId !== pageDialogSessionId.value) return;
      pageAllResults.value = clonePages(result?.items ?? []);
      pageResultSource.value = "all";
    } catch (err) {
      if (sessionId !== pageDialogSessionId.value) return;
      ElMessage.error((err as Error).message);
    } finally {
      if (sessionId === pageDialogSessionId.value) pageSearchingAll.value = false;
    }
  }

  function selectPageResult(page: PmSiyuanPageRef) {
    if (pageDialogIntent.value === "replace-primary") {
      if (deps.itemPrimaryPage.value?.docId === page.docId) {
        pageDialogVisible.value = false;
        return;
      }
      applyItemPrimaryPage(page);
      pageDialogVisible.value = false;
      return;
    }
    if (hasItemLinkedPage(page.docId)) {
      ElMessage.info("该页面已存在，无需重复关联。");
      pageDialogVisible.value = false;
      return;
    }
    if (deps.itemPrimaryPage.value) {
      addItemExtraPage(page);
    } else {
      applyItemPrimaryPage(page);
    }
    pageDialogVisible.value = false;
  }

  async function createPageForItem() {
    const title = pageCreateTitle.value;
    if (!title) {
      ElMessage.warning("请先填写工作项标题，或输入想创建的页面标题");
      return;
    }
    if (!itemEffectiveLocation.value) {
      ElMessage.warning("当前没有可用的思源默认位置，请先在配置或项目设置里指定");
      return;
    }
    try {
      pageCreating.value = true;
      const dateRange = normalizePmDateRangeForDraft(deps.itemForm.value.startAt, deps.itemForm.value.endAt);
      const result = (await invokeToolByChannel("tool:pm:siyuan-create-page", {
        title,
        description: deps.itemForm.value.description,
        projectName: deps.dialogProjectName.value ?? "未归项目",
        status: deps.itemForm.value.status,
        priority: deps.itemForm.value.priority,
        startAt: dateRange.startAt,
        endAt: dateRange.endAt,
        location: itemEffectiveLocation.value,
      })) as { created: boolean; page: PmSiyuanPageRef };
      if (!result?.page) throw new Error("思源页面创建结果为空");
      if (!result.created) {
        await ElMessageBox.confirm(
          `同一路径下已存在页面「${result.page.docTitle}」，是否直接关联这个已有页面？`,
          "页面已存在",
          { type: "warning", confirmButtonText: "关联现有页面", cancelButtonText: "取消" },
        );
      } else {
        ElMessage.success("思源页面已创建。若稍后取消工作项保存，该页面会保留但不会自动绑定。");
      }
      selectPageResult(result.page);
    } catch (err) {
      if ((err as string) !== "cancel") ElMessage.error((err as Error).message);
    } finally {
      pageCreating.value = false;
    }
  }

  async function openSiyuanPage(page: PmSiyuanPageRef | null | undefined) {
    if (!page) return;
    try {
      await invokeToolByChannel("tool:pm:siyuan-open-page", { docId: page.docId });
    } catch (err) {
      ElMessage.error((err as Error).message);
    }
  }

  function openLinkPicker() {
    openPageDialog("primary", deps.itemPrimaryPage.value ? "replace-primary" : "link");
  }

  function openReplacePrimaryDialog() {
    openPageDialog("primary", "replace-primary");
  }

  function handleItemPageCommand(row: ItemSiyuanLinkedRow, cmd: string) {
    if (cmd === "replace") openReplacePrimaryDialog();
    else if (cmd === "remove") removeItemLinkedPage(row.page.docId);
  }

  // ── Watchers ───────────────────────────────────────────
  watch(drawerVisible, (visible) => {
    if (visible) {
      form.baseUrl = getSetting("pm_siyuan_base_url") ?? DEFAULT_BASE_URL;
      form.token = getSetting("pm_siyuan_token") ?? "";
      globalSiyuanLocationDraft.value = cloneLocation(
        getSettingJson<PmSiyuanLocation | null>(PM_SIYUAN_DEFAULT_LOCATION_KEY, null),
      );
    }
  });

  watch(locationDialogVisible, (visible) => {
    if (!visible) locationPickerSearch.value = "";
  });

  watch(pageDialogVisible, (visible, previousVisible) => {
    if (!visible && previousVisible) {
      pageDialogSessionId.value += 1;
      pageSearchingAll.value = false;
    }
  });

  watch(pageFilterKeyword, (keyword, previousKeyword) => {
    if (!pageDialogVisible.value) return;
    if (pageResultSource.value !== "all" || keyword === previousKeyword) return;
    restoreLocationResults();
  });

  watch(
    [pageDialogVisible, itemEffectiveLocation, configReady, directory, directoryFetchedAt],
    ([visible]) => {
      if (!visible) return;
      if (!configReady.value) {
        pageLocationResults.value = [];
        pageLocationState.value = "missing-config";
        pageLocationRefreshError.value = "";
        return;
      }
      if (!itemEffectiveLocation.value) {
        pageLocationResults.value = [];
        pageLocationState.value = "missing-location";
        pageLocationRefreshError.value = "";
        return;
      }
      if (!directoryFetchedAt.value && directory.value.length === 0) return;
      applyPageLocationResultsFromDirectory();
    },
    { flush: "post" },
  );

  return {
    // Config state
    drawerVisible,
    form,
    globalSiyuanLocation,
    globalSiyuanLocationDraft,
    showToken,
    testing,
    testingVersion,
    loadingDirectory,
    directory,
    directoryFetchedAt,
    error,
    errorContext,
    errorTitle,
    configReady,
    treeProps,
    // Location picker state
    locationDialogVisible,
    locationPickerTarget,
    locationPickerValue,
    locationPickerSearch,
    locationPickerTitle,
    locationPickerSearchKeyword,
    locationPickerTreeData,
    locationPickerExpandedKeys,
    locationPickerTreeKey,
    locationPickerCurrentNodeKey,
    locationPickerSelectionTarget,
    locationPickerSelectionPath,
    locationPickerStatusText,
    // Page picker state
    pageDialogVisible,
    pageDialogMode,
    pageDialogIntent,
    pageFilterKeyword,
    pageResultSource,
    pageLocationResults,
    pageAllResults,
    pageSearchingAll,
    pageLocationState,
    pageLocationRefreshError,
    pageCreating,
    pageDialogInputPlaceholder,
    pageCreateTitle,
    pageCanCreateImmediately,
    pageDialogTitle,
    pageCurrentRangeText,
    pageFilterSummary,
    pageShowReturnToLocation,
    pageShowLocationLoading,
    pageShowAllLoading,
    pageEmptyMessage,
    pageDisplayedResults,
    pageFilterKeywordTrimmed,
    // Cross-cutting
    itemEffectiveLocation,
    // Config functions
    openDrawer,
    saveConfig,
    testConnection,
    loadDirectory,
    ensureDirectoryLoaded,
    refreshLocationPickerDirectory,
    // Location picker functions
    openLocationPicker,
    handleLocationTreeNodeClick,
    applyLocationPicker,
    clearLocationPicker,
    clearProjectSiyuanOverride,
    isLocationPickerNodeSelected,
    isLocationPickerNodeDisabled,
    // Page picker functions
    openPageDialog,
    restoreLocationResults,
    expandPagesToAll,
    selectPageResult,
    createPageForItem,
    openSiyuanPage,
    openLinkPicker,
    openReplacePrimaryDialog,
    handleItemPageCommand,
    // Item page management
    applyItemPrimaryPage,
    addItemExtraPage,
    hasItemLinkedPage,
    removeItemLinkedPage,
    // Helpers
    cloneLocation,
    clonePages,
  };
}
