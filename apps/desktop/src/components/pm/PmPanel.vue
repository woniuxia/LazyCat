<template>
  <div class="pm-panel">
    <el-config-provider :locale="zhCn">
      <div class="pm-layout" v-loading="initialLoading">
      <!-- Left: Project list -->
      <PmSidebar
        :projects="projects"
        :project-item-counts="projectItemCounts"
        :selected-project-id="selectedProjectId"
        :today-badge-count="todayBadgeCount"
        :drop-target-project-id="dropTargetProjectId"
        :view-id="viewId"
        @select-project="selectProject"
        @show-today="showToday"
        @open-settings="showSettingsDrawer = true"
        @create-project="projectDialogRef?.showCreate()"
        @project-context="(event, p) => projectDialogRef?.handleContext(event, p, openCtxMenu)"
        @project-drag-over="onProjectDragOver"
        @project-drag-leave="onProjectDragLeave"
        @project-drop="onProjectDrop"
      />

      <!-- Center: Kanban / Gantt -->
      <div class="pm-main">
        <PmToolbar
          v-if="selectedProject"
          :selected-project="selectedProject"
          :is-overview="isOverview"
          :view-id="viewId"
          :can-create-items="canCreateItemsInCurrentContext"
          :create-blocked-reason="createItemBlockedReason"
          v-model:search-input="searchInput"
          v-model:filter-type="filterType"
          v-model:filter-priority="filterPriority"
          v-model:selected-statuses="selectedStatuses"
          @update:view-id="setView"
          @create-item="showCreateItem"
        />

        <div v-if="!selectedProject" class="pm-empty">
          <el-empty description="选择一个项目查看看板" />
        </div>

        <template v-else>
          <PmKanbanView
            v-if="viewId === 'kanban'"
            :items="statusFilteredItems"
            :selected-item-id="selectedItemId"
            :selected-statuses="selectedStatuses"
            :is-overview="isOverview"
            :can-create-items="canCreateItemsInCurrentContext"
            :create-blocked-reason="createItemBlockedReason"
            :enabled="true"
            @select="selectItem"
            @edit="editItem"
            @item-context="onItemContext"
            @quick-advance="quickAdvance"
            @create-item="showCreateItem"
            @items-changed="loadItems"
          />

          <PmGanttView
            v-else-if="viewId === 'gantt'"
            ref="ganttViewRef"
            :items="statusFilteredItems"
            :selected-item-id="selectedItemId"
            :show-project-meta="isOverview"
            @select="selectItem"
            @edit="editItem"
            @item-context="onGanttItemContext"
            @date-change="onGanttDateChange"
            @view-change="closeCtxMenu"
            @viewport-scroll="closeCtxMenu"
          />

          <PmTodayView
            v-else-if="viewId === 'today'"
            ref="todayViewRef"
            :selected-project-id="selectedProjectId"
            :selected-item-id="selectedItemId"
            :refresh-signal="todayRefreshSignal"
            @select="selectItem"
            @edit="editItem"
            @item-context="onItemContext"
            @items-changed="onTodayItemsChanged"
          />

          <PmCalendarView
            v-else-if="viewId === 'calendar'"
            :selected-project-id="selectedProjectId"
            :selected-item-id="selectedItemId"
            @select="selectItem"
            @edit="editItem"
            @item-context="onItemContext"
            @create-at-date="showCreateItemAtDate"
            @items-changed="loadItems"
          />

          <PmMatrixView
            v-else-if="viewId === 'matrix'"
            :selected-project-id="selectedProjectId"
            :selected-item-id="selectedItemId"
            @select="selectItem"
            @edit="editItem"
            @item-context="onItemContext"
            @items-changed="loadItems"
          />

          <!-- 列表视图单独缓存：dropdown/popover 的挂载开销较大，避免每次切换都重建 -->
          <KeepAlive>
            <PmListView
              v-if="viewId === 'list'"
              :items="statusFilteredItems"
              :projects="projects"
              :selected-item-id="selectedItemId"
              :is-overview="isOverview"
              :selected-project-id="selectedProjectId"
              @select="selectItem"
              @edit="editItem"
              @item-context="onItemContext"
              @items-changed="loadItems"
            />
          </KeepAlive>
        </template>

        <!-- Right: Detail panel (floating) -->
        <PmDetailPanel v-if="selectedItem" :project="selectedItemProject" :item="selectedItem"
          @close="selectedItemId = null" @toggle-pin="togglePin" @advance-status="advanceStatus" @delete="deleteItem" />
      </div>
    </div>

    <PmProjectDialog ref="projectDialogRef" @projects-changed="onProjectsChanged" />

    <!-- Item dialog -->
    <PmItemDialog
      ref="itemDialogRef"
      v-model:visible="itemDialogVisible"
      v-model:form-project-id="itemFormProjectId"
      :editing-item="editingItem"
      :projects="projects"
      :selected-project-id="selectedProjectId"
      :is-overview="isOverview"
      :primary-page="itemPrimaryPage"
      :extra-pages="itemExtraPages"
      :global-siyuan-location="globalSiyuanLocation"
      :siyuan-config-ready="siyuanConfigReady"
      :submitting="itemSubmitting"
      @submit="submitItem"
      @open-siyuan-link-picker="openSiyuanLinkPicker"
      @open-siyuan-page="openSiyuanPage"
      @siyuan-page-command="handleItemSiyuanPageCommand"
      @open-item-link="openItemLink"
    >
      <template #todo-edit-mode>
        <div v-if="editingItem" class="pm-item-card pm-item-section">
          <div class="pm-item-section-title">执行任务</div>
          <InlineTodoList
            :pm-item-id="() => editingItem?.id"
            :items="dialogPmTodo.items"
            :summary="dialogPmTodo.summary"
            :loading="dialogPmTodo.loading"
            mode="edit"
            :candidates="dialogPmTodo.candidates"
            :candidates-loading="dialogPmTodo.candidateLoading"
            @create="dialogPmTodo.quickCreate"
            @toggle="dialogPmTodo.toggleCompleteById"
            @unlink="dialogPmTodo.unlink"
            @link="dialogPmTodo.linkBatch"
            @search-candidates="dialogPmTodo.searchCandidates"
          />
        </div>
      </template>

      <template #todo-create-mode>
        <div v-if="!editingItem" class="pm-item-card pm-item-section">
          <div class="pm-item-section-title">执行任务</div>
          <InlineTodoList
            ref="createModeTodoRef"
            :pm-item-id="() => undefined"
            :items="[]"
            :summary="null"
            :loading="false"
            mode="edit"
            :candidates="createModeCandidates"
            :candidates-loading="createModeCandidatesLoading"
            @create="onCreateModeTodoCreate"
            @toggle="() => {}"
            @unlink="() => {}"
            @link="onCreateModeTodoLink"
            @search-candidates="onCreateModeSearchCandidates"
            @pending-change="onPendingChange"
          />
        </div>
      </template>
    </PmItemDialog>

    <!-- Context menu -->
    <PmContextMenu
      :visible="ctxMenuVisible"
      :x="ctxMenuX"
      :y="ctxMenuY"
      :actions="ctxMenuActions"
      @close="closeCtxMenu"
    />

    <PmSettingsDrawer
      v-model="showSettingsDrawer"
      @imported="onImported"
    />

    <PmSiyuanDrawer />

    </el-config-provider>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount, reactive, provide } from "vue";
import { ElMessage } from "element-plus";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import { useToolInvoke } from "../../composables/useToolInvoke";
import { getSetting, getSettingJson, setSetting, setSettingJson } from "../../composables/useSettings";
import type {
  PmProject,
  PmItem,
  PmItemType,
  PmPriority,
  PmItemStatus,
  PmSiyuanLocation,
  PmSiyuanPageRef,
  PmSiyuanNotebookDirectory,
  PmSiyuanDirectoryResult,
  PmSiyuanSearchResult,
  PmSiyuanTreeNode,
  PmTodoCandidateItem,
} from "../../types/pm";

import PmGanttView from "./PmGanttView.vue";
import PmItemDialog from "./PmItemDialog.vue";
import PmDetailPanel from "./PmDetailPanel.vue";
import PmProjectDialog from "./PmProjectDialog.vue";
import InlineTodoList from "../InlineTodoList.vue";
import PmContextMenu from "./PmContextMenu.vue";
import PmToolbar from "./PmToolbar.vue";
import PmKanbanView from "./PmKanbanView.vue";
import PmTodayView from "./PmTodayView.vue";
import PmListView from "./PmListView.vue";
import PmCalendarView from "./PmCalendarView.vue";
import PmMatrixView from "./PmMatrixView.vue";
import PmSettingsDrawer from "./PmSettingsDrawer.vue";
import PmSiyuanDrawer from "./PmSiyuanDrawer.vue";
import PmSidebar from "./PmSidebar.vue";
import { usePmSiyuan } from "../../composables/usePmSiyuan";
import { PM_SIYUAN_KEY } from "../../composables/pmSiyuanKey";
import type { ItemSiyuanLinkedRow } from "../../composables/usePmSiyuan";
import {
  formatPmSiyuanLocationLabel,
  isPmSiyuanNotebookDirectory,
} from "../../utils/pmSiyuan";
import {
  normalizePmDateRangeForDraft,
} from "../../utils/pmDate";
import { ensureDataDir } from "../../rich/data-dir";
import {
  getPmDefaultSelectedStatuses,
} from "../../utils/pmStatusFilter";
import { usePmTodoLinking } from "../../composables/usePmTodoLinking";
import { usePmViewMemory } from "../../composables/usePmViewMemory";
import { usePmNavigation } from "../../composables/usePmNavigation";
import { usePmContextMenu } from "../../composables/usePmContextMenu";
import { usePmItemActions } from "../../composables/usePmItemActions";
import { usePmItemFilters } from "../../composables/usePmItemFilters";
import { PM_KANBAN_DRAG_KEY } from "../../composables/pmKanbanDragKey";

const { invoke } = useToolInvoke();

// ── Types ────────────────────────────────────────────────


// ── State ────────────────────────────────────────────────

const projects = ref<PmProject[]>([]);
const items = ref<PmItem[]>([]);
const projectItemCounts = ref<Record<number, { total: number; done: number }>>({});
const selectedProjectId = ref<number | "overview" | null>(null);
const selectedItemId = ref<number | null>(null);
const initialLoading = ref(true);
const filterType = ref<PmItemType | "">("");
const filterPriority = ref<PmPriority | "">("");
const { currentView: viewId, setView } = usePmViewMemory(selectedProjectId);
const { consumeFocus: consumePmFocus } = usePmNavigation();
const selectedStatuses = ref<PmItemStatus[]>(getPmDefaultSelectedStatuses());
const { searchInput, statusFilteredItems } = usePmItemFilters({
  items,
  filterType,
  filterPriority,
  selectedStatuses,
});
const todayBadgeCount = ref(0);
const todayRefreshSignal = ref(0);
const todayViewRef = ref<{ refresh: () => Promise<void> } | null>(null);
const ganttViewRef = ref<{ forceRefresh: () => void } | null>(null);

// Project dialog ref
const projectDialogRef = ref<InstanceType<typeof PmProjectDialog> | null>(null);

// Item dialog
const itemDialogVisible = ref(false);
const showSettingsDrawer = ref(false);
const editingItem = ref<PmItem | null>(null);
const itemFormProjectId = ref<number | null>(null);
const itemPrimaryPage = ref<PmSiyuanPageRef | null>(null);
const itemExtraPages = ref<PmSiyuanPageRef[]>([]);
const itemSubmitting = ref(false);
const itemDialogRef = ref<InstanceType<typeof PmItemDialog> | null>(null);
const itemForm = computed(() => itemDialogRef.value?.form ?? {
  title: "",
  itemType: "task" as PmItemType,
  priority: "P2" as PmPriority,
  status: "todo" as PmItemStatus,
  startAt: null as string | null,
  endAt: null as string | null,
  linkUrl: "",
  description: "",
  startedAt: null as string | null,
  testingAt: null as string | null,
  completedAt: null as string | null,
});

// PM-Todo linking composables
const dialogPmTodo = reactive(usePmTodoLinking(() => editingItem.value?.id));

// Create-mode todo state (for InlineTodoList in create mode)
const createModeTodoRef = ref<InstanceType<typeof InlineTodoList> | null>(null);
const createModeCandidates = ref<PmTodoCandidateItem[]>([]);
const createModeCandidatesLoading = ref(false);
const pendingTodoCreates = ref<Array<{ title: string; priority: string; description: string }>>([]);
const pendingTodoLinkIds = ref<number[]>([]);

function onCreateModeTodoCreate(title: string, priority: string) {
  // InlineTodoList handles local state internally
}

async function onCreateModeSearchCandidates(keyword: string) {
  if (!itemDialogProjectId.value) return;
  createModeCandidatesLoading.value = true;
  try {
    const result = await invoke<{ items: PmTodoCandidateItem[]; reason?: string }>(
      "tool:pm:todo-candidates",
      { projectId: itemDialogProjectId.value, keyword },
    );
    createModeCandidates.value = result.items ?? [];
  } catch (e) {
    console.warn("[pm] todo candidates load failed:", e);
    createModeCandidates.value = [];
  } finally {
    createModeCandidatesLoading.value = false;
  }
}

function onCreateModeTodoLink(ids: number[]) {
  // InlineTodoList handles local state internally
}

function onPendingChange(creates: Array<{ title: string; priority: string; description: string }>, links: number[]) {
  pendingTodoCreates.value = creates;
  pendingTodoLinkIds.value = links;
}

function resetPendingTodos() {
  pendingTodoCreates.value = [];
  pendingTodoLinkIds.value = [];
  createModeTodoRef.value?.resetLocal();
}

// ── SiYuan composable ─────────────────────────────────────
const itemDialogProjectId = computed<number | null>(() => {
  if (itemFormProjectId.value !== null) {
    return itemFormProjectId.value;
  }
  if (editingItem.value) {
    return editingItem.value.projectId;
  }
  return typeof selectedProjectId.value === "number" ? selectedProjectId.value : null;
});
const itemDialogProject = computed(() =>
  projects.value.find((project) => project.id === itemDialogProjectId.value) ?? null,
);
const dialogProjectSiyuanOverride = computed(() => itemDialogProject.value?.siyuanLocationOverride ?? null);
const dialogProjectName = computed(() => itemDialogProject.value?.name ?? "未归项目");
const siyuan = usePmSiyuan({
  dialogProjectSiyuanOverride,
  editingItem,
  itemForm,
  dialogProjectName,
  itemPrimaryPage,
  itemExtraPages,
  getProjectFormSiyuanOverride: () => projectDialogRef.value?.getSiyuanOverride?.() ?? null,
  setProjectFormSiyuanOverride: (useOverride, location) => {
    projectDialogRef.value?.setSiyuanOverride?.(useOverride, location);
  },
});
provide(PM_SIYUAN_KEY, siyuan);
// Aliases still used by PmPanel template/script
const globalSiyuanLocation = siyuan.globalSiyuanLocation;
const itemEffectiveLocation = siyuan.itemEffectiveLocation;
const siyuanConfigReady = siyuan.configReady;
// Function aliases
const openSiyuanDrawer = siyuan.openDrawer;
const openSiyuanLocationPicker = siyuan.openLocationPicker;
const openSiyuanPage = siyuan.openSiyuanPage;
const openSiyuanLinkPicker = siyuan.openLinkPicker;
const openReplacePrimarySiyuanDialog = siyuan.openReplacePrimaryDialog;
const openSiyuanPageDialog = siyuan.openPageDialog;
const handleItemSiyuanPageCommand = siyuan.handleItemPageCommand;
const applyItemPrimaryPage = siyuan.applyItemPrimaryPage;
const addItemExtraPage = siyuan.addItemExtraPage;
const hasItemLinkedPage = siyuan.hasItemLinkedPage;
const removeItemLinkedPage = siyuan.removeItemLinkedPage;
const ensureSiyuanDirectoryLoaded = siyuan.ensureDirectoryLoaded;
const cloneSiyuanPages = siyuan.clonePages;
function cloneSiyuanPage(page: PmSiyuanPageRef | null | undefined): PmSiyuanPageRef | null {
  return page ? { ...page } : null;
}

// Click debounce — for project card click (sidebar selects project)
// Card-level click debounce moved into PmKanbanView

// Drag state (cross-project, shared with PmKanbanView via provide)
const draggingItemId = ref<number | null>(null);
const dropTargetProjectId = ref<number | null>(null);
const dragConsumed = ref(false);
const draggingOverColumn = ref<PmItemStatus | null>(null);

provide(PM_KANBAN_DRAG_KEY, {
  draggingItemId,
  dropTargetProjectId,
  dragConsumed,
  draggingOverColumn,
});

// ── Computed ─────────────────────────────────────────────

const activeProjects = computed(() => projects.value.filter((p) => p.status === "active"));
const projectMap = computed(() => new Map(projects.value.map((project) => [project.id, project])));
const isOverview = computed(() => selectedProjectId.value === "overview");
const selectedProject = computed(() => {
  if (isOverview.value) {
    return {
      id: 0,
      name: "总览",
      color: "#606266",
      status: "active",
      description: "",
      siyuanLocationOverride: null,
      sortOrder: 0,
      createdAt: "",
      updatedAt: "",
    } as PmProject;
  }
  return projects.value.find((p) => p.id === selectedProjectId.value) ?? null;
});
const selectedItem = computed(() => items.value.find((i) => i.id === selectedItemId.value) ?? null);
const selectedItemProject = computed(() =>
  selectedItem.value ? projectMap.value.get(selectedItem.value.projectId) ?? null : null,
);

const canCreateItemsInCurrentContext = computed(() => {
  if (isOverview.value) {
    return activeProjects.value.length > 0;
  }
  return selectedProject.value?.status !== "archived";
});
const createItemBlockedReason = computed(() => {
  if (isOverview.value && activeProjects.value.length === 0) {
    return "暂无可接收工作项的项目";
  }
  if (!isOverview.value && selectedProject.value?.status === "archived") {
    return "归档项目不能接收工作项";
  }
  return "";
});

// ── Item actions ─────────────────────────────────────────

const {
  editItem,
  submitItem,
  togglePin,
  advanceStatus,
  quickAdvance,
  deleteItem,
  findNextStatus,
  toggleItemPinFor,
  advanceItemStatusFor,
  deleteItemRecord,
} = usePmItemActions({
  invoke,
  items,
  selectedItem,
  selectedItemId,
  selectedProjectId,
  isOverview,
  editingItem,
  itemFormProjectId,
  itemPrimaryPage,
  itemExtraPages,
  itemSubmitting,
  itemDialogVisible,
  pendingTodoCreates,
  pendingTodoLinkIds,
  resetPendingTodos,
  dialogPmTodo,
  siyuanConfigReady,
  itemEffectiveLocation,
  ensureSiyuanDirectoryLoaded,
  cloneSiyuanPage,
  cloneSiyuanPages,
  normalizeItemLinkUrl,
  runDialogBeforeSubmit: () => itemDialogRef.value?.runBeforeSubmit?.(),
  runDialogAfterSubmit: (id: number) => itemDialogRef.value?.runAfterSubmit?.(id),
  loadItems,
  loadItemCounts,
  loadTodayCounts,
  todayRefreshSignal,
});

// Context menu (reactive)
const {
  ctxMenuVisible,
  ctxMenuX,
  ctxMenuY,
  ctxMenuActions,
  openItemContextMenu,
  openItemContextMenuAt,
  openCtxMenu,
  closeCtxMenu,
} = usePmContextMenu({
  editItem,
  openItemLink,
  openSiyuanPage,
  toggleItemPinFor,
  findNextStatus,
  advanceItemStatusFor,
  deleteItemRecord,
});

// ── Helpers ──────────────────────────────────────────────

function getPmLightTagStyle(color?: string | null) {
  const resolvedColor = color ?? "#409eff";
  return {
    "--el-tag-bg-color": `${resolvedColor}14`,
    "--el-tag-border-color": `${resolvedColor}33`,
    "--el-tag-text-color": resolvedColor,
  };
}

function normalizeItemLinkUrl(value: string | null | undefined): string {
  let url = (value ?? "").trim();
  if (!url) return "";
  if (/^https?:\/\//i.test(url)) {
    // Already has http/https scheme, keep as-is
  } else if (url.includes("://")) {
    // Non-http scheme (e.g. ftp://), reject like backend does
    return "";
  } else {
    url = `http://${url}`;
  }
  return url;
}

async function openItemLink(url: string | null | undefined) {
  const normalized = normalizeItemLinkUrl(url);
  if (!normalized) return;
  try {
    await invoke("tool:pm:open-link", { url: normalized });
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function selectProject(id: number | "overview") {
  selectedProjectId.value = id;
  selectedItemId.value = null;
}


function selectItem(item: PmItem) {
  selectedItemId.value = item.id;
}

watch(selectedProjectId, () => {
  loadItems();
});

// ── Data loading ─────────────────────────────────────────

async function loadProjects() {
  try {
    projects.value = (await invoke<PmProject[]>("tool:pm:project-list", {})) ?? [];
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function loadItems() {
  if (!selectedProjectId.value) {
    items.value = [];
    return;
  }
  try {
    const params = isOverview.value ? {} : { projectId: selectedProjectId.value };
    items.value = (await invoke<PmItem[]>("tool:pm:item-list", params)) ?? [];
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
  loadItemCounts();
  void loadTodayCounts();
  todayRefreshSignal.value++;
}

function formatLocalDate(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

async function loadTodayCounts() {
  try {
    const payload: Record<string, unknown> = { todayDate: formatLocalDate(new Date()) };
    if (typeof selectedProjectId.value === "number") {
      payload.projectId = selectedProjectId.value;
    }
    const counts = (await invoke<{
      overdue: number;
      dueToday: number;
      inProgress: number;
      completedToday: number;
      totalActive: number;
    }>("tool:pm:item-today-counts", payload)) ?? {
      overdue: 0,
      dueToday: 0,
      inProgress: 0,
      completedToday: 0,
      totalActive: 0,
    };
    todayBadgeCount.value = (counts.overdue ?? 0) + (counts.dueToday ?? 0) + (counts.inProgress ?? 0);
  } catch (e) {
    console.warn("[pm] today counts load failed:", e);
    todayBadgeCount.value = 0;
  }
}

function showToday() {
  if (selectedProjectId.value === null) {
    selectedProjectId.value = "overview";
  }
  setView("today");
}

function onTodayItemsChanged() {
  void loadItems();
}

async function loadItemCounts() {
  try {
    const rows = (await invoke<{ projectId: number; total: number; done: number }[]>("tool:pm:item-counts", {})) ?? [];
    const map: Record<number, { total: number; done: number }> = {};
    for (const r of rows) {
      map[r.projectId] = { total: r.total, done: r.done };
    }
    projectItemCounts.value = map;
  } catch (e) {
    console.warn("[pm] project item counts load failed:", e);
    // 项目计数失败不打断主流程，仅记录日志
  }
}

async function onProjectsChanged({ deletedProjectId }: { deletedProjectId?: number }) {
  await loadProjects();
  await loadItemCounts();
  if (deletedProjectId && selectedProjectId.value === deletedProjectId) {
    selectedProjectId.value = null;
  }
}

// ── Item CRUD ────────────────────────────────────────────

async function onImported() {
  await loadProjects();
  await loadItems();
  await loadItemCounts();
}

function showCreateItem() {
  if (!canCreateItemsInCurrentContext.value) {
    ElMessage.warning(createItemBlockedReason.value || "当前项目不能接收工作项");
    return;
  }
  editingItem.value = null;
  itemFormProjectId.value = isOverview.value ? (activeProjects.value[0]?.id ?? null) : null;
  itemPrimaryPage.value = null;
  itemExtraPages.value = [];
  resetPendingTodos();
  itemDialogVisible.value = true;
  if (siyuanConfigReady.value && itemEffectiveLocation.value) {
    void ensureSiyuanDirectoryLoaded();
  }
}

function showCreateItemAtDate(date: string) {
  if (!canCreateItemsInCurrentContext.value) {
    ElMessage.warning(createItemBlockedReason.value || "当前项目不能接收工作项");
    return;
  }
  editingItem.value = null;
  itemFormProjectId.value = isOverview.value ? (activeProjects.value[0]?.id ?? null) : null;
  itemPrimaryPage.value = null;
  itemExtraPages.value = [];
  resetPendingTodos();
  itemDialogVisible.value = true;
  if (siyuanConfigReady.value && itemEffectiveLocation.value) {
    void ensureSiyuanDirectoryLoaded();
  }
  void nextTick(() => {
    const form = itemDialogRef.value?.form;
    if (form) {
      form.startAt = date;
      form.endAt = date;
    }
  });
}

function resetItemForm() {
  editingItem.value = null;
  itemFormProjectId.value = null;
  itemPrimaryPage.value = null;
  itemExtraPages.value = [];
  dialogPmTodo.reset();
}

function onItemContext(event: MouseEvent, item: PmItem) {
  openItemContextMenu(event, item);
}

function onGanttItemContext(payload: { item: PmItem; anchorX: number; anchorY: number }) {
  openItemContextMenuAt(payload.item, payload.anchorX, payload.anchorY);
}

// ── Gantt date change ────────────────────────────────────

async function onGanttDateChange(item: PmItem, start: string, end: string) {
  const normalizedDateRange = normalizePmDateRangeForDraft(start, end);
  // 乐观更新本地数据，避免全量刷新导致甘特图重建
  const target = items.value.find((i) => i.id === item.id);
  const oldStart = target?.startAt ?? null;
  const oldEnd = target?.endAt ?? null;
  if (target) {
    target.startAt = normalizedDateRange.startAt;
    target.endAt = normalizedDateRange.endAt;
  }
  try {
    await invoke("tool:pm:item-update", {
      id: item.id,
      startAt: normalizedDateRange.startAt,
      endAt: normalizedDateRange.endAt,
    });
  } catch (e) {
    // 失败时还原乐观更新，并强制甘特图重渲染（绕过 skipNextRefresh 标志）
    if (target) {
      target.startAt = oldStart;
      target.endAt = oldEnd;
    }
    ganttViewRef.value?.forceRefresh();
    await loadItems();
    ElMessage.error((e as Error).message);
  }
}

// ── View change side-effects ───────────────────────────

watch(
  () => [selectedProjectId.value, viewId.value],
  () => closeCtxMenu(),
);

// ── Cross-project drag (sidebar drop) ────────────────────

function onProjectDragOver(p: PmProject) {
  if (!draggingItemId.value) return;
  dropTargetProjectId.value = p.status === "active" ? p.id : null;
}

function onProjectDragLeave(p: PmProject) {
  if (dropTargetProjectId.value === p.id) {
    dropTargetProjectId.value = null;
  }
}

function onProjectDrop(p: PmProject) {
  if (!draggingItemId.value) return;
  if (p.status === "archived") {
    dropTargetProjectId.value = null;
    ElMessage.warning("归档项目不能接收工作项");
    return;
  }

  const item = items.value.find((i) => i.id === draggingItemId.value);
  if (!item || item.projectId === p.id) {
    dropTargetProjectId.value = null;
    return;
  }

  dragConsumed.value = true;
  dropTargetProjectId.value = null;

  const itemId = draggingItemId.value;
  invoke("tool:pm:item-move-project", { id: itemId, projectId: p.id })
    .then(() => {
      ElMessage.success(`已移至「${p.name}」`);
      loadItems();
    })
    .catch((e: unknown) => {
      ElMessage.error((e as Error).message);
      loadItems();
    });
}

// ── Lifecycle ────────────────────────────────────────────

async function tryCloseDetail() {
  if (!selectedItem.value) return;
  selectedItemId.value = null;
}

function onDetailClickAway(e: PointerEvent) {
  if (!selectedItem.value) return;
  const target = e.target;
  if (!(target instanceof Element)) return;
  if (
    target.closest(".pm-detail") ||
    target.closest(".pm-ctx-menu") ||
    target.closest(".el-overlay") ||
    target.closest(".el-popper") ||
    target.closest(".el-picker__popper") ||
    target.closest(".el-select-dropdown") ||
    target.closest(".el-message-box")
  ) {
    return;
  }
  tryCloseDetail();
}

onMounted(async () => {
  document.addEventListener("pointerdown", onDetailClickAway);
  void ensureDataDir();
  await loadProjects();
  const focus = consumePmFocus();
  if (focus) {
    selectedProjectId.value = focus.projectId ?? "overview";
    if (focus.view) setView(focus.view);
    await loadItems();
    const target = items.value.find((i) => i.id === focus.itemId);
    if (target) {
      editItem(target);
    } else {
      ElMessage.warning("未找到该工作项，可能已被删除");
    }
  } else {
    selectedProjectId.value = "overview";
    selectedItemId.value = null;
    await loadItems();
  }
  initialLoading.value = false;
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDetailClickAway);
});
</script>

<style scoped>
.pm-panel {
  height: 100%;
  overflow: hidden;
}
.pm-layout {
  display: flex;
  height: 100%;
  gap: 8px;
  padding: 8px;
}

/* Sidebar 样式已移至 PmSidebar.vue */
.sidebar-title {
  font-weight: 600;
  font-size: 16px;
}
.project-group {
  margin-bottom: 8px;
}
.project-group-label {
  padding: 4px 12px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
}
.project-item:hover {
  background: var(--el-fill-color-light);
}
.project-item.is-active {
  background: var(--el-color-primary-light-9);
  font-weight: 500;
}
.project-item.is-archived {
  opacity: 0.6;
}
.project-item.is-drop-target {
  background: var(--el-color-primary-light-8);
  box-shadow: inset 0 0 0 2px var(--el-color-primary-light-5);
  border-radius: 4px;
  animation: drop-pulse 1s ease infinite;
}
@keyframes drop-pulse {
  0%, 100% { box-shadow: inset 0 0 0 2px var(--el-color-primary-light-5); }
  50% { box-shadow: inset 0 0 0 2px var(--el-color-primary-light-3); }
}
.project-count {
  flex-shrink: 0;
  font-size: 11px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  background: var(--el-fill-color);
  border-radius: 8px;
  padding: 0 6px;
  line-height: 16px;
}
.project-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  cursor: pointer;
  font-size: 15px;
  transition: background 0.15s, box-shadow 0.15s;
  flex-wrap: wrap;
  position: relative;
}
.project-progress {
  width: 100%;
  height: 2px;
  background: var(--el-fill-color-light);
  border-radius: 1px;
  overflow: hidden;
  margin-top: -2px;
}
.project-progress-bar {
  height: 100%;
  border-radius: 1px;
  transition: width 0.3s;
}
.overview-item {
  margin-bottom: 4px;
  border-bottom: 1px solid var(--el-border-color-extra-light);
  padding-bottom: 8px;
}

/* Main area */
.pm-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
  position: relative;
  background: var(--lc-surface-0);
  border-radius: var(--lc-radius-lg);
}

/* Empty state */
.pm-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* PM visual unification */
.pm-panel {
  --pm-page-bg: var(--lc-bg);
  --pm-edge: var(--lc-border);
  --pm-edge-soft: var(--lc-border-subtle);
  --pm-accent: var(--lc-accent);
  --pm-accent-soft: var(--lc-accent-dim);
  --pm-text-main: var(--lc-text);
  --pm-text-muted: var(--lc-text-secondary);
  --pm-shadow-soft: var(--lc-shadow-sm);
  --pm-shadow-strong: var(--lc-shadow-md);
}

.pm-layout {
  background: var(--lc-surface-1);
}

/* Sidebar 重设计层样式已移至 PmSidebar.vue */

.pm-main {
  border-radius: var(--lc-radius-lg);
}

</style>

<style>
/* Global drag cursor */
body.pm-is-dragging,
body.pm-is-dragging * {
  cursor: grabbing !important;
}

.pm-form-item-top .el-form-item__content {
  align-items: flex-start;
}

.pm-item-dialog-form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.pm-item-project-card {
  position: relative;
  overflow: hidden;
  border: 1px solid #dde7f2;
  border-radius: 16px;
  padding: 14px 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(242, 247, 255, 0.94));
  box-shadow: 0 10px 22px rgba(34, 48, 66, 0.05);
}

.pm-item-project-card::before {
  content: "";
  position: absolute;
  inset: 0 auto 0 0;
  width: 4px;
  background: linear-gradient(180deg, rgba(14, 165, 233, 0.9), rgba(121, 179, 255, 0.86));
}

.pm-item-project-card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 14px;
  position: relative;
  min-height: 32px;
}

.pm-item-project-card-body {
  min-width: 0;
}

.pm-item-project-card--switchable .pm-item-project-card-body {
  padding-right: 110px;
}

.pm-item-section-eyebrow {
  margin-bottom: 8px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: #76859b;
}

.pm-item-project-card-main {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.pm-item-project-card-dot {
  width: 12px;
  height: 12px;
  border-radius: 999px;
  box-shadow: 0 0 0 5px rgba(14, 165, 233, 0.12);
  flex-shrink: 0;
}

.pm-item-project-card-name {
  min-width: 0;
  font-size: 17px;
  font-weight: 800;
  line-height: 1.25;
  color: var(--lc-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pm-item-project-card-actions {
  position: absolute;
  top: 0;
  right: 0;
}

.pm-item-project-switch-trigger {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 34px;
  padding: 0 13px;
  border: 1px solid rgba(14, 165, 233, 0.16);
  border-radius: 999px;
  background: rgba(14, 165, 233, 0.1);
  color: #4464ad;
  font-size: 12px;
  font-weight: 800;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  box-shadow: 0 8px 16px rgba(36, 66, 114, 0.08);
  transition:
    opacity 0.18s ease,
    transform 0.18s ease,
    background-color 0.18s ease,
    border-color 0.18s ease,
    box-shadow 0.18s ease;
}

.pm-item-project-switch-trigger:hover {
  background: rgba(14, 165, 233, 0.14);
  border-color: rgba(14, 165, 233, 0.24);
  box-shadow: 0 10px 18px rgba(36, 66, 114, 0.12);
}

.pm-item-project-switch-trigger:focus-visible {
  outline: 2px solid rgba(14, 165, 233, 0.34);
  outline-offset: 2px;
}

.pm-item-project-switch-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 200px;
}

.pm-item-project-switch-item-name {
  min-width: 0;
  color: #2f4158;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-item-project-switch-item-meta {
  flex-shrink: 0;
  color: #7a8798;
  font-size: 12px;
  font-weight: 600;
}

.pm-item-project-switch-menu {
  min-width: 228px;
}

.pm-item-project-switch-menu .el-dropdown-menu__item.is-disabled .pm-item-project-switch-item-name,
.pm-item-project-switch-menu .el-dropdown-menu__item.is-disabled .pm-item-project-switch-item-meta {
  color: #98a3b2;
}

@media (hover: hover) and (pointer: fine) {
  .pm-item-project-card--switchable:not(.pm-item-project-card--switch-open) .pm-item-project-switch-trigger {
    opacity: 0;
    transform: translateY(-6px);
    pointer-events: none;
    box-shadow: none;
  }

  .pm-item-project-card--switchable:hover .pm-item-project-switch-trigger,
  .pm-item-project-card--switchable:focus-within .pm-item-project-switch-trigger,
  .pm-item-project-card--switch-open .pm-item-project-switch-trigger {
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
  }
}

.project-archived-tag,
.detail-project-archived-tag {
  border-color: rgba(111, 128, 152, 0.24) !important;
  background: rgba(111, 128, 152, 0.1) !important;
  color: var(--lc-text-secondary) !important;
}

.pm-item-section {
  border: 1px solid var(--lc-border-subtle);
  border-radius: 16px;
  padding: 14px 16px 2px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(246, 249, 253, 0.92));
  box-shadow: 0 8px 18px rgba(34, 48, 66, 0.04);
}

.pm-item-section--muted {
  border-style: dashed;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.88), rgba(243, 247, 252, 0.82));
}

.pm-item-section--inline {
  padding: 12px 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.99), rgba(248, 251, 255, 0.94));
}

.pm-item-section-title {
  margin-bottom: 12px;
  font-size: 15px;
  font-weight: 700;
  color: var(--lc-text);
}

.pm-item-dialog-core-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.pm-item-dialog-inline-field {
  min-width: 0;
}

.pm-item-dialog-inline-row {
  display: flex;
  align-items: center;
  gap: 16px;
}

.pm-item-dialog-inline-row--link {
  align-items: flex-start;
}

.pm-item-dialog-inline-label {
  display: flex;
  align-items: center;
  flex: 0 0 84px;
  width: 84px;
  min-height: 40px;
  font-size: 12px;
  font-weight: 700;
  line-height: 1.5;
  letter-spacing: 0.03em;
  color: #5a6b82;
}

.pm-item-dialog-inline-row--link .pm-item-dialog-inline-label {
  align-items: flex-start;
  padding-top: 8px;
}

.pm-item-dialog-inline-content {
  flex: 1;
  min-width: 0;
}

.pm-item-dialog-inline-content--link {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.pm-item-dialog-inline-form-item {
  margin-bottom: 0;
}

.pm-item-dialog-inline-form-item--grow {
  flex: 1;
}

.pm-item-dialog-inline-picker {
  width: 100%;
}

.pm-item-dialog-link-main {
  display: flex;
  align-items: stretch;
  gap: 8px;
}

.pm-item-dialog-link-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.pm-item-dialog-link-open-btn {
  flex-shrink: 0;
  min-width: 78px;
  height: 40px;
  padding-inline: 16px;
  border-radius: 12px;
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
}

.pm-item-dialog-link-clear-btn {
  flex-shrink: 0;
  min-width: 72px;
  height: 40px;
  padding-inline: 14px;
  border-radius: 12px;
  color: #66778d;
  border-color: rgba(133, 150, 177, 0.34);
  background: rgba(247, 250, 253, 0.95);
}

.pm-item-dialog-link-open-btn:not(.is-disabled):hover {
  transform: translateY(-1px);
  box-shadow: 0 8px 16px rgba(64, 96, 160, 0.12);
}

.pm-item-dialog-link-clear-btn:not(.is-disabled):hover {
  border-color: rgba(109, 129, 160, 0.44);
  background: rgba(241, 246, 252, 0.98);
  color: #55677f;
}

.pm-item-dialog-core-grid .el-select,
.pm-item-dialog-core-grid .el-date-editor,
.pm-item-dialog-inline-form-item .el-input,
.pm-item-dialog-inline-form-item .el-date-editor {
  width: 100%;
}

.pm-item-section .el-form-item__label {
  padding-bottom: 8px !important;
  font-weight: 600;
  color: #33445a;
}

.pm-item-section .el-form-item:last-child {
  margin-bottom: 0;
}

.pm-item-dialog-inline-form-item :deep(.el-input__wrapper),
.pm-item-dialog-inline-form-item :deep(.el-date-editor.el-input__wrapper) {
  min-height: 40px;
  border-radius: 12px;
}

@media (max-width: 900px) {
  .pm-item-project-card-head,
  .detail-resource-card {
    flex-direction: column;
    align-items: stretch;
  }

  .pm-item-project-card-actions {
    position: static;
    width: 100%;
  }

  .pm-item-project-card--switchable .pm-item-project-card-body {
    padding-right: 0;
  }

  .pm-item-project-switch-trigger {
    align-self: flex-start;
  }

  .pm-item-dialog-core-grid {
    grid-template-columns: 1fr;
    gap: 0;
  }

  .pm-item-dialog-inline-row,
  .pm-item-dialog-link-main {
    flex-direction: column;
    align-items: stretch;
  }

  .pm-item-dialog-inline-label {
    width: auto;
    flex-basis: auto;
    min-height: 0;
    padding-top: 0;
  }

  .pm-item-dialog-link-actions {
    justify-content: flex-start;
  }

  .pm-item-dialog-link-clear-btn,
  .pm-item-dialog-link-open-btn {
    width: auto;
    align-self: flex-start;
  }

  .pm-sidebar {
    width: 224px;
    min-width: 224px;
  }
}

/* ── PM-Todo Linking Styles ────────────────────────────── */

.pm-todo-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 4px 0;
}

.pm-todo-summary {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 13px;
  color: #606266;
}

.pm-todo-stat {
  white-space: nowrap;
}

.pm-todo-all-done-hint {
  color: #67c23a;
  font-weight: 500;
  white-space: nowrap;
}

.pm-todo-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.pm-todo-loading,
.pm-todo-empty {
  padding: 16px 0;
  text-align: center;
  color: #909399;
  font-size: 13px;
}

.pm-todo-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.pm-todo-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border: 1px solid #ebeef5;
  border-radius: 6px;
  background: #fafafa;
  transition: background 0.15s;
}

.pm-todo-item:hover {
  background: #f0f5ff;
}

.pm-todo-item--completed {
  opacity: 0.65;
}

.pm-todo-item-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.pm-todo-item-title {
  flex: 1;
  font-size: 13px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-todo-item-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-left: 30px;
  font-size: 12px;
  color: #909399;
}

.pm-todo-item-priority {
  font-size: 11px;
}

.pm-todo-item-date {
  font-size: 12px;
}

.pm-todo-candidate-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.pm-todo-candidate-hint {
  font-size: 12px;
  color: #909399;
}

.pm-todo-candidate-list {
  max-height: 320px;
  overflow-y: auto;
}

.pm-todo-candidate-item {
  padding: 6px 0;
  border-bottom: 1px solid #f0f0f0;
}

.pm-todo-candidate-item:last-child {
  border-bottom: none;
}
</style>
