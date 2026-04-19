<template>
  <div class="pm-panel">
    <el-config-provider :locale="zhCn">
      <div class="pm-layout">
      <!-- Left: Project list -->
      <aside class="pm-sidebar">
        <div class="sidebar-header">
          <div>
            <div class="sidebar-eyebrow">项目管理</div>
          </div>
          <el-button class="sidebar-create-btn" type="primary" @click="projectDialogRef?.showCreate()">
            <el-icon><Plus /></el-icon>
            新建
          </el-button>
        </div>

        <div
          class="sidebar-today-card"
          :class="{ 'is-active': viewId === 'today' }"
          @click="showToday"
        >
          <div class="sidebar-today-head">
            <span class="sidebar-today-icon" aria-hidden="true">◷</span>
            <span class="sidebar-today-name">今日</span>
            <span class="sidebar-today-badge">{{ todayBadgeCount }}</span>
          </div>
        </div>

        <div
          class="sidebar-overview-card"
          :class="{ 'is-active': selectedProjectId === 'overview' }"
          @click="selectProject('overview')"
        >
            <div class="sidebar-overview-card-head">
              <div class="sidebar-overview-card-title">
                <span class="project-color overview-color" />
                <span class="project-name">总览</span>
              </div>
            </div>
          <div class="sidebar-overview-metrics">
            <div class="sidebar-overview-metric">
              <span class="sidebar-overview-metric-label">项目数</span>
              <strong class="sidebar-overview-metric-value">{{ projects.length }}</strong>
            </div>
            <div class="sidebar-overview-metric">
              <span class="sidebar-overview-metric-label">待办总数</span>
              <strong class="sidebar-overview-metric-value">{{ overviewUndoneCount }}</strong>
            </div>
          </div>
        </div>

        <div v-if="sidebarProjects.length > 0" class="sidebar-projects">
          <div class="sidebar-projects-head">
            <span class="sidebar-projects-title">全部项目</span>
            <span class="sidebar-projects-sort">活跃项目优先，按任务总数排序</span>
          </div>
          <div
            v-for="p in sidebarProjects"
            :key="p.id"
            class="project-card"
            :class="{
              'is-active': selectedProjectId === p.id,
              'is-drop-target': dropTargetProjectId === p.id,
              'is-archived': p.status === 'archived',
            }"
            @click="selectProject(p.id)"
            @contextmenu.prevent="projectDialogRef?.handleContext($event, p, openCtxMenu)"
            @dragover.prevent="onProjectDragOver(p)"
            @dragleave="onProjectDragLeave(p)"
            @drop.prevent="onProjectDrop(p)"
          >
            <div class="project-card-main">
              <span class="project-color" :style="{ backgroundColor: p.color }" />
              <span class="project-name">{{ p.name }}</span>
              <el-tag v-if="p.status === 'archived'" size="small" effect="plain" class="project-archived-tag">已归档</el-tag>
              <span class="project-pending-badge">{{ p.pendingCount }}</span>
            </div>
          </div>
        </div>

        <div v-if="projects.length === 0" class="empty-hint">
          暂无项目，点击 + 创建
        </div>
      </aside>

      <!-- Center: Kanban / Gantt -->
      <div class="pm-main">
        <div v-if="selectedProject" class="pm-toolbar">
          <div class="pm-toolbar-shell">
            <div class="toolbar-row pm-toolbar-head">
              <div class="toolbar-left pm-toolbar-context">
                <span
                  class="pm-toolbar-context-dot"
                  :class="{ 'is-overview': isOverview }"
                  :style="isOverview ? undefined : { backgroundColor: selectedProject.color }"
                />
                <div class="pm-toolbar-context-copy">
                  <div class="pm-toolbar-context-title-row">
                    <span class="project-title-display" :style="{ color: isOverview ? '' : selectedProject.color }">{{ selectedProject.name }}</span>
                    <el-tag v-if="!isOverview && selectedProject.status === 'archived'" size="small" type="info" class="pm-toolbar-project-tag">已归档</el-tag>
                  </div>
                </div>
              </div>
              <div class="toolbar-right pm-toolbar-head-actions">
                <div class="pm-view-switch">
                  <PmViewSwitcher :model-value="viewId" @update:model-value="setView" />
                </div>
                <el-button
                  class="pm-toolbar-primary-btn"
                  type="primary"
                  :disabled="!canCreateItemsInCurrentContext"
                  :title="createItemBlockedReason || undefined"
                  @click="showCreateItem"
                >
                  <el-icon><Plus /></el-icon>
                  新建工作项
                </el-button>
              </div>
            </div>
            <div class="toolbar-row toolbar-filters pm-toolbar-controls">
              <div class="pm-toolbar-filter-bar">
                <div class="pm-toolbar-search-wrap">
                  <el-input
                    v-model="searchText"
                    class="pm-toolbar-search-input"
                    size="default"
                    placeholder="标题、描述、标签关键词..."
                    clearable
                  >
                    <template #prefix>
                      <el-icon class="pm-toolbar-search-icon"><Search /></el-icon>
                    </template>
                  </el-input>
                </div>
                <div class="pm-toolbar-filter-cluster">
                  <el-select v-model="filterType" class="pm-toolbar-select" size="default" placeholder="类型" clearable>
                    <el-option v-for="(meta, key) in PM_ITEM_TYPE_MAP" :key="key" :label="meta.label" :value="key" />
                  </el-select>
                  <el-select v-model="filterPriority" class="pm-toolbar-select" size="default" placeholder="优先级" clearable>
                    <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key" />
                  </el-select>
                  <div class="pm-status-filter-wrap pm-toolbar-select pm-toolbar-select--status">
                    <span class="pm-status-filter-label">状态筛选</span>
                    <el-select
                      v-model="selectedStatuses"
                      class="pm-status-filter-select"
                      size="default"
                      multiple
                      collapse-tags
                      placeholder="状态筛选"
                    >
                      <el-option v-for="column in PM_STATUS_COLUMNS" :key="column.key" :label="column.label" :value="column.key" />
                    </el-select>
                  </div>
                  <el-button class="pm-toolbar-secondary-btn" type="default" @click="openSiyuanDrawer">思源设置</el-button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <PmKanbanView
          v-if="selectedProject && viewId === 'kanban'"
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
          v-else-if="selectedProject && viewId === 'gantt'"
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
          v-else-if="selectedProject && viewId === 'today'"
          ref="todayViewRef"
          :selected-project-id="selectedProjectId"
          :selected-item-id="selectedItemId"
          :refresh-signal="todayRefreshSignal"
          @select="selectItem"
          @edit="editItem"
          @item-context="onItemContext"
          @items-changed="onTodayItemsChanged"
        />

        <PmListView
          v-else-if="selectedProject && viewId === 'list'"
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

        <PmCalendarView
          v-else-if="selectedProject && viewId === 'calendar'"
          :selected-project-id="selectedProjectId"
          :selected-item-id="selectedItemId"
          @select="selectItem"
          @edit="editItem"
          @item-context="onItemContext"
          @create-at-date="showCreateItemAtDate"
          @items-changed="loadItems"
        />

        <PmMatrixView
          v-else-if="selectedProject && viewId === 'matrix'"
          :selected-project-id="selectedProjectId"
          :selected-item-id="selectedItemId"
          @select="selectItem"
          @edit="editItem"
          @item-context="onItemContext"
          @items-changed="loadItems"
        />

        <div v-else-if="!selectedProject" class="pm-empty">
          <el-empty description="选择一个项目查看看板" />
        </div>

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
      @submit="submitItem"
      @open-siyuan-link-picker="openSiyuanLinkPicker"
      @open-siyuan-page="openSiyuanPage"
      @siyuan-page-command="handleItemSiyuanPageCommand"
      @open-item-link="openItemLink"
    >
      <template #todo-edit-mode>
        <div v-if="editingItem" class="pm-item-section">
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
        <div v-if="!editingItem" class="pm-item-section">
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

    <PmSiyuanDrawer />
    </el-config-provider>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount, reactive, provide } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import { Plus, Close, Search } from "@element-plus/icons-vue";
import { useToolInvoke } from "../composables/useToolInvoke";
import { getSetting, getSettingJson, setSetting, setSettingJson } from "../composables/useSettings";
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
  CtxMenuAction,
  PmTodoCandidateItem,
} from "../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../types/pm";
import PmGanttView from "./PmGanttView.vue";
import PmItemDialog from "./PmItemDialog.vue";
import PmDetailPanel from "./PmDetailPanel.vue";
import PmSiyuanDrawer from "./PmSiyuanDrawer.vue";
import PmProjectDialog from "./PmProjectDialog.vue";
import InlineTodoList from "./InlineTodoList.vue";
import PmContextMenu from "./PmContextMenu.vue";
import PmViewSwitcher from "./PmViewSwitcher.vue";
import PmKanbanView from "./PmKanbanView.vue";
import PmTodayView from "./PmTodayView.vue";
import PmListView from "./PmListView.vue";
import PmCalendarView from "./PmCalendarView.vue";
import PmMatrixView from "./PmMatrixView.vue";
import { usePmSiyuan } from "../composables/usePmSiyuan";
import { PM_SIYUAN_KEY } from "../composables/pmSiyuanKey";
import type { ItemSiyuanLinkedRow } from "../composables/usePmSiyuan";
import {
  formatPmSiyuanLocationLabel,
  isPmSiyuanNotebookDirectory,
} from "../utils/pmSiyuan";
import {
  normalizePmDateRangeForDraft,
} from "../utils/pmDate";
import {
  sortPmProjectsForSidebar,
} from "../utils/pmVisual";
import {
  filterPmItemsBySelectedStatuses,
  getPmDefaultSelectedStatuses,
} from "../utils/pmStatusFilter";
import { usePmTodoLinking } from "../composables/usePmTodoLinking";
import { usePmViewMemory } from "../composables/usePmViewMemory";
import { PM_KANBAN_DRAG_KEY } from "../composables/pmKanbanDragKey";

const { invoke } = useToolInvoke();

// ── Types ────────────────────────────────────────────────


// ── State ────────────────────────────────────────────────

const projects = ref<PmProject[]>([]);
const items = ref<PmItem[]>([]);
const projectItemCounts = ref<Record<number, { total: number; done: number }>>({});
const selectedProjectId = ref<number | "overview" | null>(null);
const selectedItemId = ref<number | null>(null);
const searchText = ref("");
const filterType = ref<PmItemType | "">("");
const filterPriority = ref<PmPriority | "">("");
const { currentView: viewId, setView } = usePmViewMemory(selectedProjectId);
const selectedStatuses = ref<PmItemStatus[]>(getPmDefaultSelectedStatuses());
const todayBadgeCount = ref(0);
const todayRefreshSignal = ref(0);
const todayViewRef = ref<{ refresh: () => Promise<void> } | null>(null);

// Project dialog ref
const projectDialogRef = ref<InstanceType<typeof PmProjectDialog> | null>(null);

// Item dialog
const itemDialogVisible = ref(false);
const editingItem = ref<PmItem | null>(null);
const itemFormProjectId = ref<number | null>(null);
const itemPrimaryPage = ref<PmSiyuanPageRef | null>(null);
const itemExtraPages = ref<PmSiyuanPageRef[]>([]);
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
  } catch {
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

// Context menu (reactive)
const ctxMenuVisible = ref(false);
const ctxMenuX = ref(0);
const ctxMenuY = ref(0);
const ctxMenuActions = ref<CtxMenuAction[]>([]);

const PM_ITEM_STATUS_ORDER: PmItemStatus[] = ["todo", "in_progress", "testing", "done"];

// ── Computed ─────────────────────────────────────────────

const activeProjects = computed(() => projects.value.filter((p) => p.status === "active"));
const sidebarProjects = computed(() => sortPmProjectsForSidebar(projects.value, projectItemCounts.value));
const projectMap = computed(() => new Map(projects.value.map((project) => [project.id, project])));
const overviewUndoneCount = computed(() => {
  let total = 0;
  for (const c of Object.values(projectItemCounts.value)) {
    total += c.total - c.done;
  }
  return total;
});
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

const baseFilteredItems = computed(() => {
  let result = items.value;
  if (searchText.value) {
    const q = searchText.value.toLowerCase();
    result = result.filter(
      (i) =>
        i.title.toLowerCase().includes(q) ||
        i.description.toLowerCase().includes(q) ||
        i.tags.some((t) => t.toLowerCase().includes(q))
    );
  }
  if (filterType.value) {
    result = result.filter((i) => i.itemType === filterType.value);
  }
  if (filterPriority.value) {
    result = result.filter((i) => i.priority === filterPriority.value);
  }
  return result;
});

const statusFilteredItems = computed(() =>
  filterPmItemsBySelectedStatuses(baseFilteredItems.value, selectedStatuses.value),
);

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
  } catch {
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
  } catch {
    // Non-critical, silently ignore
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

function editItem(item: PmItem) {
  editingItem.value = item;
  itemFormProjectId.value = item.projectId;
  itemPrimaryPage.value = cloneSiyuanPage(item.siyuanPrimaryPage);
  itemExtraPages.value = cloneSiyuanPages(item.siyuanExtraPages);
  itemDialogVisible.value = true;
  dialogPmTodo.loadItems(item.id);
  if (siyuanConfigReady.value && itemEffectiveLocation.value) {
    void ensureSiyuanDirectoryLoaded();
  }
}

function resetItemForm() {
  editingItem.value = null;
  itemFormProjectId.value = null;
  itemPrimaryPage.value = null;
  itemExtraPages.value = [];
  dialogPmTodo.reset();
}

function formatItemTimestampValue(value: string | Date | null | undefined): string | null {
  if (!value) return null;
  return value instanceof Date ? value.toISOString() : value;
}

async function submitItem(form: {
  title: string;
  itemType: PmItemType;
  priority: PmPriority;
  status: PmItemStatus;
  startAt: string | null;
  endAt: string | null;
  linkUrl: string;
  description: string;
  startedAt?: string | null;
  testingAt?: string | null;
  completedAt?: string | null;
}) {
  if (!form.title.trim()) {
    ElMessage.warning("请输入标题");
    return;
  }
  try {
    const normalizedDateRange = normalizePmDateRangeForDraft(form.startAt, form.endAt);
    const payload: Record<string, unknown> = {
      title: form.title,
      itemType: form.itemType,
      priority: form.priority,
      status: form.status,
      startAt: normalizedDateRange.startAt,
      endAt: normalizedDateRange.endAt,
      linkUrl: normalizeItemLinkUrl(form.linkUrl) || null,
      description: form.description,
      siyuanPrimaryPage: itemPrimaryPage.value,
      siyuanExtraPages: itemExtraPages.value,
    };
    if (editingItem.value) {
      const targetProjectId = itemFormProjectId.value;
      if (!targetProjectId) {
        ElMessage.warning("请选择所属项目");
        return;
      }
      if (targetProjectId !== editingItem.value.projectId) {
        await invoke("tool:pm:item-move-project", {
          id: editingItem.value.id,
          projectId: targetProjectId,
        });
      }

      const timestampFields = ["startedAt", "testingAt", "completedAt"] as const;
      for (const field of timestampFields) {
        const draftValue = formatItemTimestampValue(form[field]);
        const originalValue = formatItemTimestampValue(editingItem.value[field]);
        if (draftValue !== originalValue) {
          payload[field] = draftValue;
        }
      }

      await invoke("tool:pm:item-update", {
        id: editingItem.value.id,
        ...payload,
      });
    } else {
      const projectId = isOverview.value ? itemFormProjectId.value : selectedProjectId.value;
      if (!projectId || projectId === "overview") {
        ElMessage.warning("请选择所属项目");
        return;
      }
      const result = await invoke<{ id: number }>("tool:pm:item-create", {
        projectId,
        ...payload,
      });
      // Process pending todo data for newly created item
      if (result.id && (pendingTodoCreates.value.length > 0 || pendingTodoLinkIds.value.length > 0)) {
        const tempTodo = usePmTodoLinking(() => result.id);
        for (const c of pendingTodoCreates.value) {
          await tempTodo.quickCreate(c.title, c.priority, c.description);
        }
        if (pendingTodoLinkIds.value.length > 0) {
          await tempTodo.linkBatch(pendingTodoLinkIds.value);
        }
      }
    }
    itemDialogVisible.value = false;
    await loadItems();
  } catch (e) {
    ElMessage.error((e as Error).message);
    if (editingItem.value) {
      await loadItems();
    }
  }
}

async function togglePin() {
  if (!selectedItem.value) return;
  await toggleItemPinFor(selectedItem.value);
}

async function advanceStatus() {
  if (!selectedItem.value) return;
  await advanceItemStatusFor(selectedItem.value);
}

async function quickAdvance(item: PmItem) {
  await advanceItemStatusFor(item);
}

async function deleteItem() {
  if (!selectedItem.value) return;
  await deleteItemRecord(selectedItem.value);
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
    await loadItems();
    ElMessage.error((e as Error).message);
  }
}

function findNextStatus(item: PmItem): PmItemStatus | null {
  const index = PM_ITEM_STATUS_ORDER.indexOf(item.status);
  if (index < 0 || index >= PM_ITEM_STATUS_ORDER.length - 1) return null;
  return PM_ITEM_STATUS_ORDER[index + 1];
}

async function toggleItemPinFor(item: PmItem) {
  try {
    await invoke("tool:pm:item-toggle-pin", { id: item.id });
    await loadItems();
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function advanceItemStatusFor(item: PmItem) {
  const nextStatus = findNextStatus(item);
  if (!nextStatus) return;
  try {
    await invoke("tool:pm:item-change-status", { id: item.id, status: nextStatus });
    await loadItems();
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function deleteItemRecord(item: PmItem) {
  try {
    await ElMessageBox.confirm("确定删除该工作项？", "删除确认", { type: "warning" });
    await invoke("tool:pm:item-delete", { id: item.id });
    if (selectedItemId.value === item.id) {
      selectedItemId.value = null;
    }
    await loadItems();
  } catch (e) {
    if ((e as string) !== "cancel") {
      ElMessage.error((e as Error).message);
    }
  }
}

function buildItemContextActions(item: PmItem): CtxMenuAction[] {
  const actions: CtxMenuAction[] = [{ label: "编辑", action: () => editItem(item) }];

  if (item.linkUrl) {
    actions.push({
      label: "打开链接",
      action: () => void openItemLink(item.linkUrl),
    });
  }

  if (item.siyuanPrimaryPage) {
    actions.push({
      label: "打开思源主页面",
      action: () => void openSiyuanPage(item.siyuanPrimaryPage),
    });
  }

  actions.push({
    label: item.pinned ? "取消置顶" : "置顶",
    action: () => void toggleItemPinFor(item),
  });

  const nextStatus = findNextStatus(item);
  if (nextStatus) {
    const nextLabel = PM_STATUS_COLUMNS.find((entry) => entry.key === nextStatus)?.label ?? nextStatus;
    actions.push({
      label: `推进到「${nextLabel}」`,
      action: () => void advanceItemStatusFor(item),
    });
  }

  actions.push(
    { divider: true, label: "", action: () => {} },
    {
      label: "删除",
      danger: true,
      action: () => void deleteItemRecord(item),
    },
  );

  return actions;
}

function openItemContextMenu(event: MouseEvent, item: PmItem) {
  openItemContextMenuAt(item, event.clientX, event.clientY);
}

function openItemContextMenuAt(item: PmItem, anchorX: number, anchorY: number) {
  ctxMenuActions.value = buildItemContextActions(item);
  ctxMenuX.value = anchorX;
  ctxMenuY.value = anchorY;
  ctxMenuVisible.value = true;
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

// ── Context menu (delegated to PmContextMenu) ───────────

function openCtxMenu(event: MouseEvent, actions: CtxMenuAction[]) {
  ctxMenuActions.value = actions;
  ctxMenuX.value = event.clientX;
  ctxMenuY.value = event.clientY;
  ctxMenuVisible.value = true;
}

function closeCtxMenu() {
  ctxMenuVisible.value = false;
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
  await loadProjects();
  selectProject("overview");
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
  gap: 0;
}

/* Sidebar */
.pm-sidebar {
  width: 200px;
  min-width: 200px;
  border-right: 1px solid var(--el-border-color-lighter);
  padding: 12px 0;
  overflow-y: auto;
  background: var(--el-bg-color);
}
.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px 8px;
}
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
.project-color {
  width: 12px;
  height: 12px;
  border-radius: 2px;
  flex-shrink: 0;
}
.project-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
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
.empty-hint {
  padding: 24px 12px;
  color: var(--el-text-color-secondary);
  font-size: 14px;
  text-align: center;
}
.overview-item {
  margin-bottom: 4px;
  border-bottom: 1px solid var(--el-border-color-extra-light);
  padding-bottom: 8px;
}
.overview-color {
  background: linear-gradient(135deg, #409eff, #67c23a, #e6a23c);
}

/* Main area */
.pm-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
  position: relative;
}
.pm-toolbar {
  display: flex;
  flex-direction: column;
  padding: 8px 16px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  flex-shrink: 0;
  gap: 6px;
}
.toolbar-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.toolbar-filters {
  justify-content: flex-end;
  gap: 8px;
}
.pm-status-filter-wrap {
  position: relative;
  width: 128px;
  flex-shrink: 0;
}
.pm-status-filter-label {
  position: absolute;
  left: 12px;
  top: 50%;
  z-index: 1;
  transform: translateY(-50%);
  color: var(--el-text-color-regular);
  font-size: 13px;
  pointer-events: none;
}
.pm-status-filter-select {
  width: 100%;
}
.pm-status-filter-select :deep(.el-select__wrapper) {
  min-height: 32px;
  border-radius: 8px;
}
.pm-status-filter-select :deep(.el-select__selection),
.pm-status-filter-select :deep(.el-select__selected-item),
.pm-status-filter-select :deep(.el-select__input-wrapper),
.pm-status-filter-select :deep(.el-select__placeholder),
.pm-status-filter-select :deep(.el-tag),
.pm-status-filter-select :deep(.el-select__tags-text) {
  opacity: 0;
}
.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}
.project-title-display {
  font-weight: 600;
  font-size: 16px;
}
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Kanban */
.kanban-board {
  display: flex;
  flex: 1;
  gap: 0;
  overflow-x: auto;
  padding: 12px;
}
.kanban-column {
  flex: 1;
  min-width: 240px;
  display: flex;
  flex-direction: column;
  background: var(--el-fill-color-lighter);
  border-radius: 6px;
  margin: 0 4px;
}
.column-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  font-weight: 600;
  font-size: 15px;
  border-bottom: 2px solid transparent;
  position: relative;
}
.column-count {
  border-radius: 10px;
  padding: 0 8px;
  font-size: 12px;
  font-weight: 600;
}
.column-body {
  flex: 1;
  padding: 8px;
  overflow-y: auto;
  min-height: 120px;
}

/* Cards */
.kanban-card {
  position: relative;
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-left: 3px solid var(--el-color-primary);
  border-radius: 6px;
  padding: 12px 12px 12px 12px;
  margin-bottom: 8px;
  cursor: grab;
  transition: box-shadow 0.15s, border-color 0.15s, margin 0.15s;
}
.kanban-card:hover {
  margin-top: -1px;
  margin-bottom: 9px;
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
}
.kanban-card:hover .card-advance-btn {
  opacity: 1;
}
.kanban-card.is-selected {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 1px var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
}
.kanban-card.is-pinned {
  border-top: 2px solid var(--el-color-warning);
}
.kanban-card.is-p0 {
  border-left-color: #f56c6c;
}
.kanban-card.is-overdue {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.06), var(--el-bg-color) 60%);
}
.kanban-card.is-overdue::after {
  content: '';
  position: absolute;
  top: 0;
  right: 0;
  border-style: solid;
  border-width: 0 16px 16px 0;
  border-color: transparent #f56c6c transparent transparent;
  border-top-right-radius: 5px;
}
.kanban-card.is-overdue:hover {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.10), var(--el-bg-color) 60%);
}
.card-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 4px;
  margin-bottom: 8px;
}
.card-title {
  font-size: 15px;
  font-weight: 500;
  line-height: 1.4;
  word-break: break-all;
}
.card-badges {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}
.badge-pin {
  color: var(--el-color-warning);
  font-size: 14px;
}
.badge-overdue {
  color: var(--lc-danger, #f56c6c);
  font-size: 14px;
}
.card-meta {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  margin-bottom: 4px;
}
.card-meta .el-tag {
  font-size: 12px;
  font-weight: 500;
  height: 18px;
  line-height: 18px;
  padding: 0 6px;
}
.card-tags {
  display: flex;
  gap: 3px;
  flex-wrap: wrap;
  margin-bottom: 4px;
}
.card-tags .el-tag {
  font-size: 12px;
  height: 18px;
}
.card-dates {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.is-overdue-date {
  color: var(--lc-danger, #f56c6c);
  font-weight: 600;
}
.priority-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
  vertical-align: middle;
  flex-shrink: 0;
}
.card-project {
  margin-top: 4px;
}
.card-project-badge {
  display: inline-block;
  font-size: 11px;
  font-weight: 500;
  padding: 1px 6px;
  border-radius: 3px;
  line-height: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}
.card-project-dot {
  width: 8px;
  height: 8px;
  border-radius: 2px;
  flex-shrink: 0;
}
.card-project-name {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Quick action button */
.card-advance-btn {
  position: absolute;
  right: 6px;
  bottom: 6px;
  width: 26px;
  height: 26px;
  border-radius: 13px;
  border: 1px solid var(--el-border-color-light);
  background: var(--el-bg-color);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0.35;
  transition: opacity 0.15s, background 0.15s, color 0.15s;
  color: var(--el-text-color-secondary);
}
.card-advance-btn:hover {
  background: var(--el-color-success-light-9);
  border-color: var(--el-color-success-light-5);
  color: var(--el-color-success);
}

/* Drag */
:deep(.kanban-ghost) {
  opacity: 0.4;
  border: 2px dashed var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
  border-radius: 6px;
  box-shadow: none;
}

:deep(.kanban-drag),
:deep(.kanban-fallback) {
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
  transform: rotate(2deg);
  opacity: 0.92;
  z-index: 100;
}

/* Column drag-over highlight */
.kanban-column.is-drag-over {
  background: var(--el-color-primary-light-9);
  box-shadow: inset 0 0 0 2px var(--el-color-primary-light-5);
  transition: background 0.15s, box-shadow 0.15s;
}
.kanban-column.is-drag-over .column-header {
  color: var(--el-color-primary);
}
.kanban-column.is-drag-over .column-header::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 3px;
  background: var(--el-color-primary);
  animation: drag-bar-in 0.2s ease;
}
@keyframes drag-bar-in {
  from { transform: scaleX(0); }
  to { transform: scaleX(1); }
}

/* Empty column drop hint */
.column-drop-hint {
  text-align: center;
  padding: 16px 8px;
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  border: 2px dashed var(--el-border-color-light);
  border-radius: 6px;
  pointer-events: none;
}
.column-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 24px 8px;
  border: 1px dashed var(--el-border-color-lighter);
  border-radius: 6px;
}
.column-empty-text {
  font-size: 13px;
  color: var(--el-text-color-placeholder);
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
  --pm-surface: #ffffff;
  --pm-page-bg: #f4f7fb;
  --pm-edge: #dbe5f1;
  --pm-edge-soft: #e3ebf5;
  --pm-accent: #4d7df2;
  --pm-accent-soft: rgba(77, 125, 242, 0.12);
  --pm-text-main: #223042;
  --pm-text-muted: #6f8098;
  --pm-shadow-soft: 0 10px 24px rgba(34, 48, 66, 0.06);
  --pm-shadow-strong: 0 18px 34px rgba(34, 48, 66, 0.1);
}

.pm-layout {
  background:
    radial-gradient(circle at top left, rgba(77, 125, 242, 0.08), transparent 28%),
    linear-gradient(180deg, #f7faff 0%, var(--pm-page-bg) 42%, #eff4fa 100%);
}

.pm-sidebar {
  width: 248px;
  min-width: 248px;
  padding: 16px 14px 18px;
  border-right: 1px solid var(--pm-edge);
  background: linear-gradient(180deg, #f8fbff 0%, #f1f5fb 100%);
}

.sidebar-header {
  position: relative;
  padding: 0 2px 14px;
  margin-bottom: 10px;
}

.sidebar-eyebrow {
  margin-bottom: 4px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--pm-text-muted);
}

.sidebar-create-btn {
  position: absolute;
  right: 2px;
  top: 2px;
  min-height: 36px;
  padding-inline: 14px;
  border-radius: 12px;
  box-shadow: 0 10px 22px rgba(77, 125, 242, 0.2);
  opacity: 0;
  visibility: hidden;
  pointer-events: none;
  transform: translateY(-4px);
  transition:
    opacity 0.18s ease,
    visibility 0.18s ease,
    transform 0.18s ease,
    box-shadow 0.18s ease;
}

.sidebar-header:hover .sidebar-create-btn,
.sidebar-header:focus-within .sidebar-create-btn {
  opacity: 1;
  visibility: visible;
  pointer-events: auto;
  transform: translateY(0);
}

.sidebar-today-card {
  margin-bottom: 12px;
  padding: 12px 14px;
  border: 1px solid var(--pm-edge);
  border-radius: 14px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(248, 251, 255, 0.92));
  box-shadow: var(--pm-shadow-soft);
  cursor: pointer;
  transition: border-color 0.18s ease, box-shadow 0.18s ease, transform 0.18s ease, background 0.18s ease;
}

.sidebar-today-card:hover,
.sidebar-today-card.is-active {
  border-color: rgba(77, 125, 242, 0.45);
  box-shadow: 0 14px 24px rgba(77, 125, 242, 0.16);
  transform: translateY(-1px);
}

.sidebar-today-card.is-active {
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(229, 240, 255, 0.95));
}

.sidebar-today-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.sidebar-today-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 10px;
  background: rgba(77, 125, 242, 0.12);
  color: var(--pm-accent);
  font-size: 16px;
  font-family: "Segoe UI Symbol", "Apple Symbols", sans-serif;
}

.sidebar-today-name {
  flex: 1;
  font-size: 14px;
  font-weight: 600;
  color: var(--pm-text-main);
}

.sidebar-today-badge {
  min-width: 24px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--pm-accent);
  color: #ffffff;
  font-size: 12px;
  font-weight: 600;
  text-align: center;
  line-height: 18px;
}

.sidebar-overview-card {
  margin-bottom: 16px;
  padding: 14px;
  border: 1px solid var(--pm-edge);
  border-radius: 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(240, 246, 255, 0.94));
  box-shadow: var(--pm-shadow-soft);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    transform 0.18s ease,
    background 0.18s ease;
}

.sidebar-overview-card:hover,
.sidebar-overview-card.is-active {
  border-color: rgba(77, 125, 242, 0.35);
  box-shadow: 0 16px 28px rgba(77, 125, 242, 0.14);
  transform: translateY(-1px);
}

.sidebar-overview-card.is-active {
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(226, 237, 255, 0.95));
}

.sidebar-overview-card-head,
.sidebar-projects-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.sidebar-overview-card-head {
  margin-bottom: 12px;
  align-items: flex-start;
}

.sidebar-overview-card-title {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.sidebar-overview-metrics {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.sidebar-overview-metric {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, 0.95);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.78);
}

.sidebar-overview-metric-label,
.sidebar-projects-sort {
  font-size: 12px;
  color: var(--pm-text-muted);
}

.sidebar-overview-metric-value {
  font-size: 16px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.sidebar-projects {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.sidebar-projects-head {
  padding: 0 2px;
}

.sidebar-projects-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.project-card {
  padding: 12px 14px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.82);
  box-shadow: 0 8px 18px rgba(34, 48, 66, 0.04);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    transform 0.18s ease,
    background 0.18s ease;
}

.project-card:hover {
  border-color: rgba(77, 125, 242, 0.25);
  background: rgba(255, 255, 255, 0.98);
  box-shadow: 0 14px 24px rgba(34, 48, 66, 0.08);
  transform: translateY(-1px);
}

.project-card.is-active {
  border-color: rgba(77, 125, 242, 0.36);
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(232, 240, 255, 0.94));
  box-shadow: 0 16px 28px rgba(77, 125, 242, 0.14);
}

.project-card.is-archived {
  opacity: 1;
}

.project-card.is-drop-target {
  border-color: rgba(77, 125, 242, 0.45);
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(223, 234, 255, 0.96));
  box-shadow:
    0 0 0 2px rgba(77, 125, 242, 0.16),
    0 18px 30px rgba(77, 125, 242, 0.16);
  animation: none;
}

.project-card-main {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.project-pending-badge {
  display: inline-flex;
  align-items: center;
  margin-left: auto;
  min-width: 24px;
  justify-content: center;
  padding: 4px 8px;
  border-radius: 999px;
  background: rgba(77, 125, 242, 0.10);
  color: var(--pm-accent);
  font-size: 12px;
  font-weight: 700;
  line-height: 1;
}

.project-card .project-color,
.sidebar-overview-card .project-color {
  width: 10px;
  height: 10px;
  border-radius: 999px;
}

.project-card .project-name,
.sidebar-overview-card .project-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--pm-text-main);
}

.overview-color {
  background: linear-gradient(135deg, #4d7df2, #73a0ff 46%, #88c9ff 100%);
}

.empty-hint {
  margin-top: 20px;
  padding: 18px 12px;
  border: 1px dashed var(--pm-edge);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.6);
  color: var(--pm-text-muted);
}

.pm-main {
  background:
    radial-gradient(circle at top center, rgba(77, 125, 242, 0.06), transparent 30%),
    linear-gradient(180deg, rgba(247, 250, 255, 0.96), rgba(240, 245, 251, 0.98));
}

.pm-toolbar {
  padding: 16px 18px 14px;
  border-bottom: 1px solid var(--pm-edge);
  background: rgba(244, 247, 251, 0.72);
  backdrop-filter: blur(16px);
}

.pm-toolbar-shell {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 16px;
  border: 1px solid rgba(255, 255, 255, 0.92);
  border-radius: 22px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.95), rgba(245, 249, 255, 0.9)),
    radial-gradient(circle at top left, rgba(77, 125, 242, 0.09), transparent 36%);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.8),
    0 14px 30px rgba(34, 48, 66, 0.08);
}

.toolbar-row {
  gap: 14px;
  flex-wrap: wrap;
}

.pm-toolbar-head {
  align-items: center;
}

.pm-toolbar-controls {
  align-items: flex-end;
  width: 100%;
}

.pm-toolbar-filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  width: 100%;
  padding: 10px 12px;
  border: 1px solid rgba(219, 229, 241, 0.95);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.62);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.72);
  flex-wrap: wrap;
}

.pm-toolbar-context {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex: 1 1 300px;
}

.pm-toolbar-context-dot {
  width: 14px;
  height: 14px;
  border-radius: 999px;
  flex-shrink: 0;
  box-shadow: 0 0 0 4px rgba(77, 125, 242, 0.12);
}

.pm-toolbar-context-dot.is-overview {
  background: linear-gradient(135deg, #4d7df2, #73a0ff 46%, #88c9ff 100%);
}

.pm-toolbar-context-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.pm-toolbar-context-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}

.pm-toolbar-project-tag {
  border-radius: 999px;
}

.pm-toolbar-head-actions {
  gap: 10px;
  flex-wrap: wrap;
}

.pm-view-switch {
  display: inline-flex;
  align-items: center;
}

.pm-toolbar-primary-btn {
  min-height: 42px;
  padding: 0 16px;
  border: 0;
  border-radius: 14px;
  background: linear-gradient(180deg, #4d7df2, #3d6ef0);
  box-shadow: 0 12px 24px rgba(77, 125, 242, 0.22);
}

.pm-toolbar-primary-btn:hover {
  background: linear-gradient(180deg, #5a88f5, #4674ef);
}

.pm-toolbar-primary-btn :deep(.el-icon) {
  margin-right: 2px;
}

.pm-toolbar-search-wrap {
  display: flex;
  flex-direction: column;
  min-width: 260px;
  flex: 1 1 320px;
}

.pm-toolbar-search-input {
  width: 100%;
}

.pm-toolbar-search-input :deep(.el-input__wrapper) {
  min-height: 42px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.98);
  box-shadow: inset 0 0 0 1px rgba(77, 125, 242, 0.12);
  transition: box-shadow 0.18s ease, border-color 0.18s ease;
}

.pm-toolbar-search-input :deep(.el-input__wrapper.is-focus) {
  box-shadow:
    inset 0 0 0 1px rgba(77, 125, 242, 0.32),
    0 10px 20px rgba(77, 125, 242, 0.10);
}

.pm-toolbar-search-input :deep(.el-input__inner) {
  font-size: 13px;
  color: var(--pm-text-main);
}

.pm-toolbar-search-icon {
  color: #7a90a8;
  font-size: 15px;
}

.pm-toolbar-filter-cluster {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  flex: 0 1 auto;
  flex-wrap: wrap;
}

.pm-toolbar-select {
  width: 108px;
  flex-shrink: 0;
}

.pm-toolbar-select.pm-toolbar-select--status {
  width: 138px;
}

.pm-toolbar-select :deep(.el-select__wrapper) {
  min-height: 42px;
  padding: 0 12px;
  border-radius: 14px;
  border-color: rgba(77, 125, 242, 0.12);
  background: rgba(255, 255, 255, 0.96);
  box-shadow: none;
  transition: box-shadow 0.18s ease, border-color 0.18s ease;
}

.pm-toolbar-select :deep(.el-select__wrapper.is-focused) {
  box-shadow: 0 10px 20px rgba(77, 125, 242, 0.10);
}

.pm-toolbar-select :deep(.el-select__placeholder),
.pm-toolbar-select .pm-status-filter-label {
  color: #5a748f;
  font-weight: 600;
}

.pm-status-filter-wrap.pm-toolbar-select {
  position: relative;
}

.pm-status-filter-wrap.pm-toolbar-select .pm-status-filter-label {
  left: 14px;
  top: 50%;
  font-size: 13px;
}

.pm-status-filter-wrap.pm-toolbar-select :deep(.el-select__wrapper) {
  min-height: 42px;
  border-radius: 14px;
}

.pm-toolbar-secondary-btn {
  min-height: 42px;
  padding: 0 14px;
  border-radius: 14px;
  border-color: rgba(77, 125, 242, 0.12);
  background: rgba(245, 249, 255, 0.9);
  color: var(--pm-text-main);
}

.pm-toolbar-secondary-btn:hover {
  border-color: rgba(77, 125, 242, 0.22);
  background: rgba(240, 246, 255, 0.98);
}

.project-title-display {
  font-size: 18px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.pm-status-filter-label {
  color: var(--pm-text-main);
}

.kanban-board {
  gap: 12px;
  padding: 18px;
}

.kanban-column {
  min-width: 272px;
  margin: 0;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.72);
  box-shadow: var(--pm-shadow-soft);
  overflow: hidden;
}

.column-header {
  padding: 14px 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(244, 247, 251, 0.9));
}

.column-title {
  color: var(--pm-text-main);
}

.column-count {
  min-width: 30px;
  line-height: 22px;
  text-align: center;
}

.column-body {
  padding: 10px;
  background: linear-gradient(180deg, rgba(244, 247, 251, 0.82), rgba(255, 255, 255, 0.58));
}

.kanban-card {
  padding: 12px;
  margin-bottom: 10px;
  border-width: 1px 1px 1px 4px;
  border-radius: 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(248, 251, 255, 0.98));
  box-shadow: 0 10px 22px rgba(34, 48, 66, 0.05);
}

.kanban-card:hover {
  margin-top: 0;
  margin-bottom: 10px;
  border-color: rgba(77, 125, 242, 0.3);
  box-shadow: var(--pm-shadow-strong);
  transform: translateY(-1px);
}

.kanban-card.is-selected {
  border-color: rgba(77, 125, 242, 0.42);
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(233, 241, 255, 0.92));
  box-shadow:
    0 0 0 1px rgba(77, 125, 242, 0.12),
    0 18px 32px rgba(77, 125, 242, 0.14);
}

.kanban-card.is-overdue {
  background: linear-gradient(180deg, rgba(255, 247, 247, 0.96), rgba(255, 255, 255, 1));
}

.kanban-card.is-overdue:hover {
  background: linear-gradient(180deg, rgba(255, 244, 244, 1), rgba(255, 255, 255, 1));
}

.card-topbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
}

.card-topbar-left,
.card-topbar-right {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  min-width: 0;
}

.card-topbar-right {
  justify-content: flex-end;
}

.card-project-badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: 100%;
  padding: 4px 10px;
  border: 1px solid rgba(77, 125, 242, 0.12);
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
}

.card-project-name {
  color: inherit;
  font-size: 12px;
}

.card-meta-pill {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border: 1px solid;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.78);
  font-size: 12px;
  font-weight: 600;
  line-height: 1;
}

.card-date-chip {
  display: inline-flex;
  align-items: center;
  padding: 4px 8px;
  border-radius: 999px;
  background: rgba(219, 229, 241, 0.7);
  color: var(--pm-text-muted);
  font-size: 12px;
  font-weight: 600;
}

.card-title-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
}

.card-title {
  flex: 1;
  min-width: 0;
  font-size: 15px;
  font-weight: 700;
  line-height: 1.45;
  color: var(--pm-text-main);
  word-break: break-word;
}

.card-meta {
  gap: 6px;
  margin-bottom: 8px;
}

.card-tags {
  gap: 6px;
  margin-bottom: 0;
}

.card-tags :deep(.el-tag),
.detail-field-inline :deep(.el-tag) {
  border-radius: 999px;
}

.card-advance-btn {
  position: static;
  width: 28px;
  height: 28px;
  flex-shrink: 0;
  border-radius: 10px;
  border-color: rgba(77, 125, 242, 0.16);
  background: rgba(77, 125, 242, 0.08);
  color: var(--pm-accent);
  opacity: 0;
  visibility: hidden;
  pointer-events: none;
  transform: translateY(2px);
  transition:
    opacity 0.16s ease,
    visibility 0.16s ease,
    transform 0.16s ease,
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease;
}

.kanban-card:hover .card-advance-btn,
.kanban-card:focus-within .card-advance-btn {
  opacity: 1;
  visibility: visible;
  pointer-events: auto;
  transform: translateY(0);
}

.card-advance-btn:hover {
  background: var(--pm-accent);
  border-color: var(--pm-accent);
  color: #ffffff;
}

.column-drop-hint {
  border-radius: 14px;
  border-color: rgba(77, 125, 242, 0.32);
  background: rgba(77, 125, 242, 0.06);
  color: var(--pm-accent);
}

.column-empty-state {
  border-radius: 14px;
  border-color: var(--pm-edge);
  background: rgba(255, 255, 255, 0.68);
}

.column-empty-text {
  color: var(--pm-text-muted);
}

@media (max-width: 1380px) {
  .pm-toolbar-controls {
    align-items: stretch;
  }

  .pm-toolbar-filter-bar {
    align-items: stretch;
  }
}

@media (max-width: 1120px) {
  .pm-toolbar-shell {
    padding: 14px;
    border-radius: 20px;
  }

  .pm-toolbar-head {
    align-items: flex-start;
  }

  .pm-toolbar-head-actions {
    width: 100%;
    justify-content: space-between;
  }

  .pm-toolbar-controls {
    flex-direction: column;
  }

  .pm-toolbar-filter-bar,
  .pm-toolbar-search-wrap,
  .pm-toolbar-filter-cluster {
    width: 100%;
    flex: 1 1 auto;
  }

  .pm-toolbar-filter-cluster {
    justify-content: flex-start;
  }
}

@media (max-width: 820px) {
  .pm-toolbar {
    padding: 14px 14px 12px;
  }

  .pm-toolbar-head-actions {
    flex-direction: column;
    align-items: stretch;
  }

  .pm-view-switch {
    width: 100%;
    justify-content: space-between;
  }

  .pm-toolbar-primary-btn {
    width: 100%;
  }

  .pm-toolbar-filter-bar {
    padding: 10px;
  }

  .pm-toolbar-filter-cluster {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-items: stretch;
  }

  .pm-toolbar-select,
  .pm-toolbar-select.pm-toolbar-select--status,
  .pm-toolbar-secondary-btn {
    width: 100%;
  }
}

@media (max-width: 560px) {
  .pm-toolbar-filter-cluster {
    grid-template-columns: 1fr;
  }

  .pm-view-switch {
    flex-direction: column;
    align-items: stretch;
  }
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
  background: linear-gradient(180deg, rgba(77, 125, 242, 0.9), rgba(121, 179, 255, 0.86));
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
  box-shadow: 0 0 0 5px rgba(77, 125, 242, 0.12);
  flex-shrink: 0;
}

.pm-item-project-card-name {
  min-width: 0;
  font-size: 17px;
  font-weight: 800;
  line-height: 1.25;
  color: #223042;
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
  border: 1px solid rgba(77, 125, 242, 0.16);
  border-radius: 999px;
  background: rgba(77, 125, 242, 0.1);
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
  background: rgba(77, 125, 242, 0.14);
  border-color: rgba(77, 125, 242, 0.24);
  box-shadow: 0 10px 18px rgba(36, 66, 114, 0.12);
}

.pm-item-project-switch-trigger:focus-visible {
  outline: 2px solid rgba(77, 125, 242, 0.34);
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
  color: #6f8098 !important;
}

.pm-item-section {
  border: 1px solid #e3ebf5;
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
  color: #223042;
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
