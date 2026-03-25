<template>
  <div class="pm-panel">
    <div class="pm-layout">
      <!-- Left: Project list -->
      <aside class="pm-sidebar">
        <div class="sidebar-header">
          <span class="sidebar-title">项目</span>
          <el-button size="small" type="primary" link @click="showCreateProject">
            <el-icon><Plus /></el-icon>
          </el-button>
        </div>

        <div
          class="project-item overview-item"
          :class="{ 'is-active': selectedProjectId === 'overview' }"
          @click="selectProject('overview')"
        >
          <span class="project-color overview-color" />
          <span class="project-name">总览</span>
        </div>

        <div v-if="activeProjects.length > 0" class="project-group">
          <div class="project-group-label">进行中</div>
          <div
            v-for="p in activeProjects"
            :key="p.id"
            class="project-item"
            :class="{
              'is-active': selectedProjectId === p.id,
              'is-drop-target': dropTargetProjectId === p.id,
            }"
            @click="selectProject(p.id)"
            @contextmenu.prevent="onProjectContext($event, p)"
            @dragover.prevent="onProjectDragOver(p)"
            @dragleave="onProjectDragLeave(p)"
            @drop.prevent="onProjectDrop(p)"
          >
            <span class="project-color" :style="{ backgroundColor: p.color }" />
            <span class="project-name">{{ p.name }}</span>
          </div>
        </div>

        <div v-if="archivedProjects.length > 0" class="project-group">
          <div class="project-group-label">已归档</div>
          <div
            v-for="p in archivedProjects"
            :key="p.id"
            class="project-item is-archived"
            :class="{ 'is-active': selectedProjectId === p.id }"
            @click="selectProject(p.id)"
            @contextmenu.prevent="onProjectContext($event, p)"
          >
            <span class="project-color" :style="{ backgroundColor: p.color, opacity: 0.5 }" />
            <span class="project-name">{{ p.name }}</span>
          </div>
        </div>

        <div v-if="projects.length === 0" class="empty-hint">
          暂无项目，点击 + 创建
        </div>
      </aside>

      <!-- Center: Kanban / Gantt -->
      <div class="pm-main">
        <div v-if="selectedProject" class="pm-toolbar">
          <div class="toolbar-left">
            <span class="project-title-display" :style="{ color: isOverview ? '' : selectedProject.color }">{{ selectedProject.name }}</span>
            <el-tag v-if="!isOverview && selectedProject.status === 'archived'" size="small" type="info">已归档</el-tag>
          </div>
          <div class="toolbar-right">
            <el-radio-group v-model="viewMode" size="small">
              <el-radio-button value="kanban">看板</el-radio-button>
              <el-radio-button value="gantt">甘特图</el-radio-button>
            </el-radio-group>
            <el-input
              v-model="searchText"
              size="small"
              placeholder="搜索工作项..."
              clearable
              style="width: 180px"
            />
            <el-select v-model="filterType" size="small" placeholder="类型" clearable style="width: 100px">
              <el-option v-for="(meta, key) in PM_ITEM_TYPE_MAP" :key="key" :label="meta.label" :value="key" />
            </el-select>
            <el-select v-model="filterPriority" size="small" placeholder="优先级" clearable style="width: 100px">
              <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key" />
            </el-select>
            <el-button size="small" type="primary" @click="showCreateItem">新建工作项</el-button>
          </div>
        </div>

        <div v-if="selectedProject && viewMode === 'kanban'" class="kanban-board">
          <div v-for="col in PM_STATUS_COLUMNS" :key="col.key" class="kanban-column" :class="{ 'is-drag-over': draggingOverColumn === col.key }">
            <div class="column-header">
              <span class="column-title">{{ col.label }}</span>
              <span class="column-count">{{ columnItems(col.key).length }}</span>
            </div>
            <div
              :ref="(el) => setColumnRef(col.key, el)"
              class="column-body"
              :data-status="col.key"
            >
              <div
                v-for="item in columnItems(col.key)"
                :key="item.id"
                class="kanban-card"
                :class="{
                  'is-selected': selectedItemId === item.id,
                  'is-pinned': item.pinned,
                  'is-overdue': isOverdue(item),
                }"
                :style="{ borderLeftColor: PM_PRIORITY_MAP[item.priority]?.color }"
                :data-id="item.id"
                @click="selectItem(item)"
                @dblclick="editItem(item)"
                @contextmenu.prevent="onItemContext($event, item)"
              >
                <el-icon class="kanban-drag-handle" :size="14" title="拖拽排序"><Rank /></el-icon>
                <div class="card-header">
                  <span class="card-title">{{ item.title }}</span>
                  <div class="card-badges">
                    <el-icon v-if="item.pinned" class="badge-pin" title="已置顶"><Top /></el-icon>
                    <el-icon v-if="isOverdue(item)" class="badge-overdue" title="已逾期"><AlarmClock /></el-icon>
                  </div>
                </div>
                <div class="card-meta">
                  <el-tag size="small" :color="PM_ITEM_TYPE_MAP[item.itemType]?.color" effect="dark" round>
                    {{ PM_ITEM_TYPE_MAP[item.itemType]?.label }}
                  </el-tag>
                  <el-tag size="small" :color="PM_PRIORITY_MAP[item.priority]?.color" effect="dark" round>
                    {{ item.priority }}
                  </el-tag>
                </div>
                <div v-if="item.tags.length > 0" class="card-tags">
                  <el-tag v-for="tag in item.tags" :key="tag" size="small" type="info">{{ tag }}</el-tag>
                </div>
                <div v-if="item.startAt || item.endAt" class="card-dates">
                  <span v-if="item.startAt">{{ formatShortDate(item.startAt) }}</span>
                  <span v-if="item.startAt && item.endAt"> ~ </span>
                  <span v-if="item.endAt" :class="{ 'is-overdue-date': isOverdue(item) }">{{ formatShortDate(item.endAt) }}</span>
                </div>
                <div v-if="isOverview && item.projectName" class="card-project">
                  <span class="card-project-dot" :style="{ backgroundColor: item.projectColor || '#909399' }" />
                  <span class="card-project-name">{{ item.projectName }}</span>
                </div>
                <!-- Quick action: advance status -->
                <button
                  v-if="item.status !== 'done'"
                  class="card-advance-btn"
                  :title="'推进到「' + nextStatusLabel(item) + '」'"
                  @click.stop="quickAdvance(item)"
                >
                  <el-icon :size="12"><CaretRight /></el-icon>
                </button>
              </div>
              <div v-if="columnItems(col.key).length === 0 && draggingItemId" class="column-drop-hint">
                拖放到此列
              </div>
            </div>
          </div>
        </div>

        <PmGanttView
          v-if="selectedProject && viewMode === 'gantt'"
          :items="filteredItems"
          @select="selectItem"
          @date-change="onGanttDateChange"
        />

        <div v-if="!selectedProject" class="pm-empty">
          <el-empty description="选择一个项目查看看板" />
        </div>

        <!-- Right: Detail panel (floating) -->
        <Transition name="pm-detail-slide">
          <aside v-if="selectedItem" class="pm-detail">
            <div class="detail-header">
              <span class="detail-title">详情</span>
              <el-button size="small" link @click="selectedItemId = null">
                <el-icon><Close /></el-icon>
              </el-button>
            </div>
            <el-form label-position="top" size="small" class="detail-form">
              <el-form-item label="所属项目">
                <el-select
                  :model-value="selectedItem.projectId"
                  style="width: 100%"
                  @change="moveItemToProject($event as number)"
                >
                  <el-option
                    v-for="p in activeProjects"
                    :key="p.id"
                    :label="p.name"
                    :value="p.id"
                  />
                </el-select>
              </el-form-item>
              <el-form-item label="标题">
                <el-input v-model="detailForm.title" @change="saveDetail" />
              </el-form-item>
              <el-form-item label="类型">
                <el-select v-model="detailForm.itemType" @change="saveDetail">
                  <el-option v-for="(meta, key) in PM_ITEM_TYPE_MAP" :key="key" :label="meta.label" :value="key" />
                </el-select>
              </el-form-item>
              <el-form-item label="优先级">
                <el-select v-model="detailForm.priority" @change="saveDetail">
                  <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key" />
                </el-select>
              </el-form-item>
              <el-form-item label="状态">
                <el-select v-model="detailForm.status" @change="saveDetail">
                  <el-option v-for="col in PM_STATUS_COLUMNS" :key="col.key" :label="col.label" :value="col.key" />
                </el-select>
              </el-form-item>
              <el-form-item label="开始日期">
                <el-date-picker v-model="detailForm.startAt" type="date" value-format="YYYY-MM-DD" clearable style="width:100%" @change="saveDetail" />
              </el-form-item>
              <el-form-item label="截止日期">
                <el-date-picker v-model="detailForm.endAt" type="date" value-format="YYYY-MM-DD" clearable style="width:100%" @change="saveDetail" />
              </el-form-item>
              <el-form-item label="标签">
                <div class="tag-editor">
                  <el-tag
                    v-for="tag in detailForm.tags"
                    :key="tag"
                    closable
                    size="small"
                    @close="removeTag(tag)"
                  >{{ tag }}</el-tag>
                  <el-input
                    v-if="tagInputVisible"
                    ref="tagInputRef"
                    v-model="tagInputValue"
                    size="small"
                    style="width: 80px"
                    @keyup.enter="confirmTag"
                    @blur="confirmTag"
                  />
                  <el-button v-else size="small" link @click="showTagInput">+ 标签</el-button>
                </div>
              </el-form-item>
              <el-form-item label="描述">
                <el-input v-model="detailForm.description" type="textarea" :rows="4" @change="saveDetail" />
              </el-form-item>
              <el-form-item v-if="selectedItem.completedAt" label="完成时间">
                <span class="detail-readonly">{{ formatDateTime(selectedItem.completedAt) }}</span>
              </el-form-item>
              <el-form-item label="创建时间">
                <span class="detail-readonly">{{ formatDateTime(selectedItem.createdAt) }}</span>
              </el-form-item>

              <div class="detail-actions">
                <el-button size="small" @click="togglePin">{{ selectedItem.pinned ? '取消置顶' : '置顶' }}</el-button>
                <el-button v-if="selectedItem.status !== 'done'" size="small" type="success" @click="advanceStatus">
                  推进状态
                </el-button>
                <el-button size="small" type="danger" @click="deleteItem">删除</el-button>
              </div>
            </el-form>
          </aside>
        </Transition>
      </div>
    </div>
    <el-dialog v-model="projectDialogVisible" :title="editingProject ? '编辑项目' : '新建项目'" width="420px" @close="resetProjectForm">
      <el-form :model="projectForm" label-width="60px" size="small">
        <el-form-item label="名称">
          <el-input v-model="projectForm.name" placeholder="项目名称" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="projectForm.description" type="textarea" :rows="2" placeholder="项目描述（可选）" />
        </el-form-item>
        <el-form-item label="颜色">
          <el-color-picker v-model="projectForm.color" :predefine="presetColors" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button size="small" @click="projectDialogVisible = false">取消</el-button>
        <el-button size="small" type="primary" @click="submitProject">确定</el-button>
      </template>
    </el-dialog>

    <!-- Item dialog -->
    <el-dialog v-model="itemDialogVisible" :title="editingItem ? '编辑工作项' : '新建工作项'" width="480px" @close="resetItemForm">
      <el-form :model="itemForm" label-width="80px" size="small">
        <el-form-item v-if="isOverview && !editingItem" label="所属项目">
          <el-select v-model="itemFormProjectId" placeholder="选择项目" style="width: 100%">
            <el-option v-for="p in activeProjects" :key="p.id" :label="p.name" :value="p.id" />
          </el-select>
        </el-form-item>
        <el-form-item label="标题">
          <el-input v-model="itemForm.title" placeholder="工作项标题" />
        </el-form-item>
        <el-form-item label="类型">
          <el-select v-model="itemForm.itemType">
            <el-option v-for="(meta, key) in PM_ITEM_TYPE_MAP" :key="key" :label="meta.label" :value="key" />
          </el-select>
        </el-form-item>
        <el-form-item label="优先级">
          <el-select v-model="itemForm.priority">
            <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key" />
          </el-select>
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="itemForm.status">
            <el-option v-for="col in PM_STATUS_COLUMNS" :key="col.key" :label="col.label" :value="col.key" />
          </el-select>
        </el-form-item>
        <el-form-item label="开始日期">
          <el-date-picker v-model="itemForm.startAt" type="date" value-format="YYYY-MM-DD" clearable style="width:100%" />
        </el-form-item>
        <el-form-item label="截止日期">
          <el-date-picker v-model="itemForm.endAt" type="date" value-format="YYYY-MM-DD" clearable style="width:100%" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="itemForm.description" type="textarea" :rows="3" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button size="small" @click="itemDialogVisible = false">取消</el-button>
        <el-button size="small" type="primary" @click="submitItem">确定</el-button>
      </template>
    </el-dialog>

    <!-- Context menu (Vue reactive) -->
    <Teleport to="body">
      <Transition name="ctx-fade">
        <div
          v-if="ctxMenuVisible"
          class="pm-ctx-menu"
          :style="{ left: ctxMenuX + 'px', top: ctxMenuY + 'px' }"
          @contextmenu.prevent
        >
          <template v-for="(act, idx) in ctxMenuActions" :key="idx">
            <div v-if="act.divider" class="pm-ctx-divider" />
            <div
              v-else
              class="pm-ctx-item"
              :class="{ 'is-danger': act.danger }"
              @click="executeCtxAction(act)"
            >
              {{ act.label }}
            </div>
          </template>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Plus, Close, Top, CaretRight, AlarmClock, Rank } from "@element-plus/icons-vue";
import { useToolInvoke } from "../composables/useToolInvoke";
import type { PmProject, PmItem, PmItemType, PmPriority, PmItemStatus } from "../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../types/pm";
import Sortable from "sortablejs";
import PmGanttView from "./PmGanttView.vue";

const { invoke } = useToolInvoke();

// ── Types ────────────────────────────────────────────────

interface CtxMenuAction {
  label: string;
  action: () => void;
  danger?: boolean;
  divider?: boolean;
}

// ── State ────────────────────────────────────────────────

const projects = ref<PmProject[]>([]);
const items = ref<PmItem[]>([]);
const selectedProjectId = ref<number | "overview" | null>(null);
const selectedItemId = ref<number | null>(null);
const searchText = ref("");
const filterType = ref<PmItemType | "">("");
const filterPriority = ref<PmPriority | "">("");
const viewMode = ref<"kanban" | "gantt">("kanban");

// Project dialog
const projectDialogVisible = ref(false);
const editingProject = ref<PmProject | null>(null);
const projectForm = ref({ name: "", description: "", color: "#409eff" });
const presetColors = ["#409eff", "#67c23a", "#e6a23c", "#f56c6c", "#909399", "#00bcd4", "#9c27b0", "#ff5722"];

// Item dialog
const itemDialogVisible = ref(false);
const editingItem = ref<PmItem | null>(null);
const itemFormProjectId = ref<number | null>(null);
const itemForm = ref({
  title: "",
  itemType: "task" as PmItemType,
  priority: "P2" as PmPriority,
  status: "todo" as PmItemStatus,
  startAt: null as string | null,
  endAt: null as string | null,
  description: "",
});

// Detail form
const detailForm = ref({
  title: "",
  itemType: "task" as PmItemType,
  priority: "P2" as PmPriority,
  status: "todo" as PmItemStatus,
  startAt: null as string | null,
  endAt: null as string | null,
  description: "",
  tags: [] as string[],
});

// Tag input
const tagInputVisible = ref(false);
const tagInputValue = ref("");
const tagInputRef = ref<{ focus: () => void } | null>(null);

// Sortable instances
const sortableInstances = ref<Map<string, Sortable>>(new Map());
const columnRefs = ref<Map<string, HTMLElement>>(new Map());

// Drag state (cross-project)
const draggingItemId = ref<number | null>(null);
const dropTargetProjectId = ref<number | null>(null);
const dragConsumed = ref(false);
const draggingOverColumn = ref<PmItemStatus | null>(null);

// Context menu (reactive)
const ctxMenuVisible = ref(false);
const ctxMenuX = ref(0);
const ctxMenuY = ref(0);
const ctxMenuActions = ref<CtxMenuAction[]>([]);

// ── Computed ─────────────────────────────────────────────

const activeProjects = computed(() => projects.value.filter((p) => p.status === "active"));
const archivedProjects = computed(() => projects.value.filter((p) => p.status === "archived"));
const isOverview = computed(() => selectedProjectId.value === "overview");
const selectedProject = computed(() => {
  if (isOverview.value) {
    return { id: 0, name: "总览", color: "#606266", status: "active", description: "", sortOrder: 0, createdAt: "", updatedAt: "" } as PmProject;
  }
  return projects.value.find((p) => p.id === selectedProjectId.value) ?? null;
});
const selectedItem = computed(() => items.value.find((i) => i.id === selectedItemId.value) ?? null);

const filteredItems = computed(() => {
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

function columnItems(status: PmItemStatus) {
  return filteredItems.value.filter((i) => i.status === status);
}

// ── Helpers ──────────────────────────────────────────────

function isOverdue(item: PmItem): boolean {
  if (!item.endAt || item.status === "done") return false;
  const end = new Date(item.endAt);
  end.setHours(23, 59, 59, 999);
  return end.getTime() < Date.now();
}

function nextStatusLabel(item: PmItem): string {
  const idx = PM_STATUS_COLUMNS.findIndex((c) => c.key === item.status);
  return idx >= 0 && idx < PM_STATUS_COLUMNS.length - 1 ? PM_STATUS_COLUMNS[idx + 1].label : "";
}

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
}

function selectProject(id: number | "overview") {
  selectedProjectId.value = id;
  selectedItemId.value = null;
}

function selectItem(item: PmItem) {
  selectedItemId.value = item.id;
  syncDetailForm(item);
}

function syncDetailForm(item: PmItem) {
  detailForm.value = {
    title: item.title,
    itemType: item.itemType,
    priority: item.priority,
    status: item.status,
    startAt: item.startAt,
    endAt: item.endAt,
    description: item.description,
    tags: [...item.tags],
  };
}

watch(selectedProjectId, () => {
  loadItems();
});

// ── Project CRUD ─────────────────────────────────────────

function showCreateProject() {
  editingProject.value = null;
  projectForm.value = { name: "", description: "", color: "#409eff" };
  projectDialogVisible.value = true;
}

function showEditProject(p: PmProject) {
  editingProject.value = p;
  projectForm.value = { name: p.name, description: p.description, color: p.color };
  projectDialogVisible.value = true;
}

function resetProjectForm() {
  editingProject.value = null;
}

async function submitProject() {
  if (!projectForm.value.name.trim()) {
    ElMessage.warning("请输入项目名称");
    return;
  }
  try {
    if (editingProject.value) {
      await invoke("tool:pm:project-update", {
        id: editingProject.value.id,
        ...projectForm.value,
        sortOrder: editingProject.value.sortOrder,
      });
    } else {
      await invoke("tool:pm:project-create", projectForm.value);
    }
    projectDialogVisible.value = false;
    await loadProjects();
    if (!editingProject.value && projects.value.length > 0) {
      const latest = projects.value.filter((p) => p.status === "active");
      if (latest.length > 0) {
        selectProject(latest[latest.length - 1].id);
      }
    }
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function archiveProject(p: PmProject) {
  try {
    await invoke("tool:pm:project-archive", { id: p.id });
    await loadProjects();
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function restoreProject(p: PmProject) {
  try {
    await invoke("tool:pm:project-restore", { id: p.id });
    await loadProjects();
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function deleteProject(p: PmProject) {
  try {
    await ElMessageBox.confirm(`确定删除项目「${p.name}」？此操作会同时删除所有工作项。`, "删除确认", {
      type: "warning",
    });
    await invoke("tool:pm:project-delete", { id: p.id });
    if (selectedProjectId.value === p.id) {
      selectedProjectId.value = null;
    }
    await loadProjects();
  } catch (e) {
    if ((e as string) !== "cancel") {
      ElMessage.error((e as Error).message);
    }
  }
}

function onProjectContext(event: MouseEvent, p: PmProject) {
  const actions: CtxMenuAction[] = p.status === "active"
    ? [
        { label: "编辑", action: () => showEditProject(p) },
        { label: "归档", action: () => archiveProject(p) },
        { divider: true, label: "", action: () => {} },
        { label: "删除", action: () => deleteProject(p), danger: true },
      ]
    : [
        { label: "编辑", action: () => showEditProject(p) },
        { label: "恢复", action: () => restoreProject(p) },
        { divider: true, label: "", action: () => {} },
        { label: "删除", action: () => deleteProject(p), danger: true },
      ];
  openCtxMenu(event, actions);
}

// ── Item CRUD ────────────────────────────────────────────

function showCreateItem() {
  editingItem.value = null;
  itemFormProjectId.value = isOverview.value ? (activeProjects.value[0]?.id ?? null) : null;
  itemForm.value = {
    title: "",
    itemType: "task",
    priority: "P2",
    status: "todo",
    startAt: null,
    endAt: null,
    description: "",
  };
  itemDialogVisible.value = true;
}

function editItem(item: PmItem) {
  editingItem.value = item;
  itemForm.value = {
    title: item.title,
    itemType: item.itemType,
    priority: item.priority,
    status: item.status,
    startAt: item.startAt,
    endAt: item.endAt,
    description: item.description,
  };
  itemDialogVisible.value = true;
}

function resetItemForm() {
  editingItem.value = null;
}

async function submitItem() {
  if (!itemForm.value.title.trim()) {
    ElMessage.warning("请输入标题");
    return;
  }
  try {
    if (editingItem.value) {
      await invoke("tool:pm:item-update", {
        id: editingItem.value.id,
        ...itemForm.value,
      });
    } else {
      const projectId = isOverview.value ? itemFormProjectId.value : selectedProjectId.value;
      if (!projectId || projectId === "overview") {
        ElMessage.warning("请选择所属项目");
        return;
      }
      await invoke("tool:pm:item-create", {
        projectId,
        ...itemForm.value,
      });
    }
    itemDialogVisible.value = false;
    await loadItems();
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function saveDetail() {
  if (!selectedItemId.value) return;
  try {
    await invoke("tool:pm:item-update", {
      id: selectedItemId.value,
      ...detailForm.value,
    });
    await loadItems();
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function togglePin() {
  if (!selectedItemId.value) return;
  try {
    await invoke("tool:pm:item-toggle-pin", { id: selectedItemId.value });
    await loadItems();
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function advanceStatus() {
  if (!selectedItem.value) return;
  const order: PmItemStatus[] = ["todo", "in_progress", "testing", "done"];
  const idx = order.indexOf(selectedItem.value.status);
  if (idx < order.length - 1) {
    const newStatus = order[idx + 1];
    try {
      await invoke("tool:pm:item-change-status", { id: selectedItem.value.id, status: newStatus });
      detailForm.value.status = newStatus;
      await loadItems();
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }
}

async function quickAdvance(item: PmItem) {
  const order: PmItemStatus[] = ["todo", "in_progress", "testing", "done"];
  const idx = order.indexOf(item.status);
  if (idx < order.length - 1) {
    try {
      await invoke("tool:pm:item-change-status", { id: item.id, status: order[idx + 1] });
      await loadItems();
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }
}

async function deleteItem() {
  if (!selectedItemId.value) return;
  try {
    await ElMessageBox.confirm("确定删除该工作项？", "删除确认", { type: "warning" });
    await invoke("tool:pm:item-delete", { id: selectedItemId.value });
    selectedItemId.value = null;
    await loadItems();
  } catch (e) {
    if ((e as string) !== "cancel") {
      ElMessage.error((e as Error).message);
    }
  }
}

function onItemContext(event: MouseEvent, item: PmItem) {
  const statusOrder: PmItemStatus[] = ["todo", "in_progress", "testing", "done"];
  const idx = statusOrder.indexOf(item.status);
  const actions: CtxMenuAction[] = [
    { label: "编辑", action: () => editItem(item) },
    { label: item.pinned ? "取消置顶" : "置顶", action: async () => {
      await invoke("tool:pm:item-toggle-pin", { id: item.id });
      await loadItems();
    }},
  ];
  if (idx < statusOrder.length - 1) {
    actions.push({
      label: `推进到「${PM_STATUS_COLUMNS[idx + 1].label}」`,
      action: async () => {
        await invoke("tool:pm:item-change-status", { id: item.id, status: statusOrder[idx + 1] });
        await loadItems();
      },
    });
  }
  actions.push(
    { divider: true, label: "", action: () => {} },
    {
      label: "删除",
      danger: true,
      action: async () => {
        try {
          await ElMessageBox.confirm("确定删除？", "删除", { type: "warning" });
          await invoke("tool:pm:item-delete", { id: item.id });
          if (selectedItemId.value === item.id) selectedItemId.value = null;
          await loadItems();
        } catch {}
      },
    },
  );
  openCtxMenu(event, actions);
}

// ── Move to project ──────────────────────────────────────

async function moveItemToProject(newProjectId: number) {
  if (!selectedItemId.value || !selectedItem.value) return;
  if (selectedItem.value.projectId === newProjectId) return;
  try {
    await invoke("tool:pm:item-move-project", {
      id: selectedItemId.value,
      projectId: newProjectId,
    });
    const proj = projects.value.find((p) => p.id === newProjectId);
    ElMessage.success(`已移至「${proj?.name}」`);
    await loadItems();
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

// ── Gantt date change ────────────────────────────────────

async function onGanttDateChange(item: PmItem, start: string, end: string) {
  // 乐观更新本地数据，避免全量刷新导致甘特图重建
  const target = items.value.find((i) => i.id === item.id);
  if (target) {
    target.startAt = start;
    target.endAt = end;
  }
  try {
    await invoke("tool:pm:item-update", {
      id: item.id,
      startAt: start,
      endAt: end,
    });
  } catch (e) {
    await loadItems();
    ElMessage.error((e as Error).message);
  }
}

// ── Tag editor ───────────────────────────────────────────

function showTagInput() {
  tagInputVisible.value = true;
  nextTick(() => tagInputRef.value?.focus());
}

function confirmTag() {
  const val = tagInputValue.value.trim();
  if (val && !detailForm.value.tags.includes(val)) {
    detailForm.value.tags.push(val);
    saveDetail();
  }
  tagInputVisible.value = false;
  tagInputValue.value = "";
}

function removeTag(tag: string) {
  detailForm.value.tags = detailForm.value.tags.filter((t) => t !== tag);
  saveDetail();
}

// ── Sortable (drag & drop) ───────────────────────────────

function setColumnRef(status: string, el: unknown) {
  if (el instanceof HTMLElement) {
    columnRefs.value.set(status, el);
  }
}

function initSortable() {
  destroySortable();
  for (const col of PM_STATUS_COLUMNS) {
    const el = columnRefs.value.get(col.key);
    if (!el) continue;
    const instance = Sortable.create(el, {
      group: "kanban",
      animation: 150,
      forceFallback: true,
      ghostClass: "kanban-ghost",
      dragClass: "kanban-drag",
      fallbackClass: "kanban-fallback",
      onStart: (evt) => {
        draggingItemId.value = parseInt(evt.item.dataset.id ?? "0", 10);
        document.body.classList.add("pm-is-dragging");
      },
      onMove: (evt) => {
        draggingOverColumn.value = (evt.to as HTMLElement).dataset.status as PmItemStatus || null;
      },
      onEnd: async (evt) => {
        draggingItemId.value = null;
        draggingOverColumn.value = null;
        dropTargetProjectId.value = null;
        document.body.classList.remove("pm-is-dragging");

        // Skip reorder if the drag was consumed by sidebar drop
        if (dragConsumed.value) {
          dragConsumed.value = false;
          return;
        }

        const itemId = parseInt(evt.item.dataset.id ?? "0", 10);
        const newStatus = (evt.to as HTMLElement).dataset.status as PmItemStatus;
        if (!itemId || !newStatus) return;

        const children = Array.from(evt.to.children) as HTMLElement[];
        const reorderItems = children
          .filter((c) => c.dataset.id)
          .map((child, idx) => ({
            id: parseInt(child.dataset.id ?? "0", 10),
            sortOrder: idx,
            status: newStatus,
          }));

        try {
          const oldStatus = (evt.from as HTMLElement).dataset.status;
          await invoke("tool:pm:item-reorder", { items: reorderItems });
          await loadItems();
          if (oldStatus && oldStatus !== newStatus) {
            const label = PM_STATUS_COLUMNS.find((c) => c.key === newStatus)?.label ?? newStatus;
            ElMessage.success({ message: `已移至「${label}」`, duration: 1500 });
          }
        } catch (e) {
          ElMessage.error((e as Error).message);
          await loadItems();
        }
      },
    });
    sortableInstances.value.set(col.key, instance);
  }
}

function destroySortable() {
  for (const inst of sortableInstances.value.values()) {
    inst.destroy();
  }
  sortableInstances.value.clear();
}

// 项目/视图切换 → 立即重建 Sortable
watch(
  () => [selectedProjectId.value, viewMode.value],
  () => { nextTick(() => { if (!draggingItemId.value) initSortable(); }); }
);

// 过滤条件变化 → 延迟重建，跳过拖拽中
watch(
  [searchText, filterType, filterPriority],
  () => { nextTick(() => { if (!draggingItemId.value) initSortable(); }); },
  { flush: 'post' }
);

// ── Cross-project drag (sidebar drop) ────────────────────

function onProjectDragOver(p: PmProject) {
  if (draggingItemId.value) {
    dropTargetProjectId.value = p.id;
  }
}

function onProjectDragLeave(p: PmProject) {
  if (dropTargetProjectId.value === p.id) {
    dropTargetProjectId.value = null;
  }
}

function onProjectDrop(p: PmProject) {
  if (!draggingItemId.value) return;

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

// ── Context menu (Vue reactive) ──────────────────────────

function openCtxMenu(event: MouseEvent, actions: CtxMenuAction[]) {
  closeCtxMenu();
  ctxMenuX.value = Math.min(event.clientX, window.innerWidth - 160);
  ctxMenuY.value = Math.min(event.clientY, window.innerHeight - actions.length * 34 - 16);
  ctxMenuActions.value = actions;
  ctxMenuVisible.value = true;
  setTimeout(() => {
    document.addEventListener("pointerdown", handleCtxClickAway);
  }, 0);
}

function closeCtxMenu() {
  ctxMenuVisible.value = false;
  document.removeEventListener("pointerdown", handleCtxClickAway);
}

function handleCtxClickAway(e: PointerEvent) {
  const target = e.target as HTMLElement;
  if (!target.closest(".pm-ctx-menu")) {
    closeCtxMenu();
  }
}

function executeCtxAction(act: CtxMenuAction) {
  act.action();
  closeCtxMenu();
}

// ── Formatting ───────────────────────────────────────────

function formatShortDate(dateStr: string): string {
  if (!dateStr) return "";
  const d = new Date(dateStr);
  return `${(d.getMonth() + 1).toString().padStart(2, "0")}-${d.getDate().toString().padStart(2, "0")}`;
}

function formatDateTime(dateStr: string): string {
  if (!dateStr) return "";
  const d = new Date(dateStr);
  return d.toLocaleString("zh-CN");
}

// ── Lifecycle ────────────────────────────────────────────

function isDetailDirty(): boolean {
  const item = selectedItem.value;
  if (!item) return false;
  const f = detailForm.value;
  return (
    f.title !== item.title ||
    f.itemType !== item.itemType ||
    f.priority !== item.priority ||
    f.status !== item.status ||
    (f.startAt ?? "") !== (item.startAt ?? "") ||
    (f.endAt ?? "") !== (item.endAt ?? "") ||
    f.description !== item.description ||
    JSON.stringify(f.tags) !== JSON.stringify(item.tags)
  );
}

let closingDetail = false;

async function tryCloseDetail() {
  if (closingDetail) return;
  if (!selectedItem.value) return;
  if (!isDetailDirty()) {
    selectedItemId.value = null;
    return;
  }
  closingDetail = true;
  try {
    await ElMessageBox.confirm("详情有未保存的修改，是否保存？", "提示", {
      confirmButtonText: "保存",
      cancelButtonText: "放弃",
      type: "warning",
    });
    await saveDetail();
  } catch {
    // 用户选择放弃
  }
  selectedItemId.value = null;
  closingDetail = false;
}

function onDetailClickAway(e: PointerEvent) {
  if (!selectedItem.value) return;
  const target = e.target as HTMLElement;
  // 忽略详情面板内部、弹窗遮罩、下拉菜单等 Element Plus 弹出层的点击
  if (
    target.closest(".pm-detail") ||
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
  destroySortable();
  closeCtxMenu();
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
  font-size: 14px;
}
.project-group {
  margin-bottom: 8px;
}
.project-group-label {
  padding: 4px 12px;
  font-size: 11px;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
}
.project-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  cursor: pointer;
  font-size: 13px;
  transition: background 0.15s, box-shadow 0.15s;
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
}
.project-color {
  width: 10px;
  height: 10px;
  border-radius: 2px;
  flex-shrink: 0;
}
.project-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.empty-hint {
  padding: 24px 12px;
  color: var(--el-text-color-secondary);
  font-size: 13px;
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
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  flex-shrink: 0;
}
.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}
.project-title-display {
  font-weight: 600;
  font-size: 15px;
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
  min-width: 220px;
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
  padding: 8px 12px;
  font-weight: 600;
  font-size: 13px;
  border-bottom: 1px solid var(--el-border-color-extra-light);
}
.column-count {
  background: var(--el-fill-color);
  border-radius: 10px;
  padding: 0 8px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
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
  padding: 10px 10px 10px 10px;
  margin-bottom: 6px;
  cursor: grab;
  transition: box-shadow 0.15s, border-color 0.15s;
}
.kanban-card:hover {
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.06);
}
.kanban-card:hover .card-advance-btn {
  opacity: 1;
}
.kanban-card.is-selected {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 1px var(--el-color-primary-light-5);
}
.kanban-card.is-pinned {
  border-top: 2px solid var(--el-color-warning);
}
.kanban-card.is-overdue {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.06), var(--el-bg-color) 60%);
}
.kanban-card.is-overdue:hover {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.10), var(--el-bg-color) 60%);
}
.card-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 4px;
  margin-bottom: 6px;
}
.card-title {
  font-size: 13px;
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
  font-size: 10px;
  height: 18px;
  line-height: 18px;
  padding: 0 6px;
  border: none;
}
.card-tags {
  display: flex;
  gap: 3px;
  flex-wrap: wrap;
  margin-bottom: 4px;
}
.card-tags .el-tag {
  font-size: 10px;
  height: 18px;
}
.card-dates {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}
.is-overdue-date {
  color: var(--lc-danger, #f56c6c);
  font-weight: 600;
}
.card-project {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
}
.card-project-dot {
  width: 8px;
  height: 8px;
  border-radius: 2px;
  flex-shrink: 0;
}
.card-project-name {
  font-size: 11px;
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
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 1px solid var(--el-border-color-light);
  background: var(--el-bg-color);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0;
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
  opacity: 0.35;
  border: 2px dashed var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
  border-radius: 6px;
  box-shadow: none;
}
:deep(.kanban-ghost) > * { visibility: hidden; }

:deep(.kanban-drag),
:deep(.kanban-fallback) {
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
  transform: rotate(2deg);
  opacity: 0.92;
  z-index: 100;
}

/* Drag handle */
.kanban-drag-handle {
  position: absolute;
  top: 8px;
  left: 2px;
  cursor: grab;
  color: var(--el-text-color-placeholder);
  opacity: 0;
  transition: opacity 0.15s;
  z-index: 1;
  padding: 2px;
}
.kanban-card:hover .kanban-drag-handle { opacity: 0.6; }
.kanban-drag-handle:hover { opacity: 1 !important; color: var(--el-text-color-secondary); }
.kanban-drag-handle:active { cursor: grabbing; }

/* Column drag-over highlight */
.kanban-column.is-drag-over {
  background: var(--el-color-primary-light-9);
  box-shadow: inset 0 0 0 2px var(--el-color-primary-light-5);
  transition: background 0.15s, box-shadow 0.15s;
}
.kanban-column.is-drag-over .column-header {
  color: var(--el-color-primary);
}

/* Empty column drop hint */
.column-drop-hint {
  text-align: center;
  padding: 16px 8px;
  color: var(--el-text-color-placeholder);
  font-size: 12px;
  border: 2px dashed var(--el-border-color-light);
  border-radius: 6px;
  pointer-events: none;
}

/* Detail panel (floating overlay) */
.pm-detail {
  position: absolute;
  top: 0;
  right: 0;
  width: 300px;
  height: 100%;
  border-left: 1px solid var(--el-border-color-lighter);
  padding: 12px;
  overflow-y: auto;
  background: var(--el-bg-color);
  box-shadow: -4px 0 12px rgba(0, 0, 0, 0.08);
  z-index: 10;
}
.detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.detail-title {
  font-weight: 600;
  font-size: 14px;
}
.detail-form .el-form-item {
  margin-bottom: 12px;
}
.detail-form .el-form-item__label {
  font-size: 12px;
  padding-bottom: 2px;
}
.detail-readonly {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.detail-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 12px;
}
.tag-editor {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  align-items: center;
}

/* Detail panel transition */
.pm-detail-slide-enter-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.pm-detail-slide-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.pm-detail-slide-enter-from,
.pm-detail-slide-leave-to {
  opacity: 0;
  transform: translateX(20px);
}

/* Empty state */
.pm-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>

<style>
/* Context menu (global because of Teleport to body) */
.pm-ctx-menu {
  position: fixed;
  z-index: 9999;
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 6px;
  padding: 4px 0;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12);
  min-width: 140px;
}
.pm-ctx-item {
  padding: 6px 16px;
  font-size: 13px;
  cursor: pointer;
  transition: background 0.15s;
}
.pm-ctx-item:hover {
  background: var(--el-fill-color-light);
}
.pm-ctx-item.is-danger {
  color: var(--el-color-danger);
}
.pm-ctx-item.is-danger:hover {
  background: var(--el-color-danger-light-9);
}
.pm-ctx-divider {
  height: 1px;
  margin: 4px 8px;
  background: var(--el-border-color-extra-light);
}

/* Context menu transition */
.ctx-fade-enter-active {
  transition: opacity 0.1s ease, transform 0.1s ease;
}
.ctx-fade-leave-active {
  transition: opacity 0.08s ease;
}
.ctx-fade-enter-from {
  opacity: 0;
  transform: scale(0.95);
}
.ctx-fade-leave-to {
  opacity: 0;
}

/* Global drag cursor */
body.pm-is-dragging,
body.pm-is-dragging * {
  cursor: grabbing !important;
}
</style>
