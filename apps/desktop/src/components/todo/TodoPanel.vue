<template>
  <div class="todo-panel">
    <div class="todo-layout" v-loading="initialLoading">
      <TodoSidebar
        :active-items="activeItems"
        :recent-week-items="recentWeekItems"
        :done-items="doneItems"
        v-model:filter-type="filterType"
        v-model:filter-priority="filterPriority"
        @open-basics="basicsDialogVisible = true"
      />
      <section class="todo-list-pane">
        <div class="toolbar">
          <div class="toolbar-left">
            <el-radio-group v-model="viewMode" size="small">
              <el-radio-button value="list">
                <el-icon><Document /></el-icon>
              </el-radio-button>
              <el-radio-button value="calendar">
                <el-icon><Grid /></el-icon>
              </el-radio-button>
            </el-radio-group>
          </div>
          <div class="toolbar-right">
            <el-select
              v-model="filterProjectId"
              size="default"
              placeholder="全部项目"
              clearable
              style="width: 140px"
            >
              <el-option label="未归项目" value="none" />
              <el-option
                v-for="p in projectOptions"
                :key="p.id"
                :label="p.name"
                :value="p.id"
              />
            </el-select>
            <el-input
              v-model.trim="itemKeyword"
              clearable
              placeholder="搜索标题或描述"
              style="width: 220px"
            />
            <el-button @click="loadItems">刷新</el-button>
            <el-button type="primary" @click="startCreate">新增事项</el-button>
          </div>
        </div>
        <TodoQuickAddBar
          v-if="viewMode === 'list'"
          class="todo-quick-add-bar"
          :context="quickAddContext"
          @created="onQuickAddCreated"
        />
        <div
          v-if="viewMode === 'list'"
          class="todo-list-scroll"
          @scroll.passive="closeTodoContextMenu"
        >
          <div v-if="hasActiveFilter" class="filter-indicator">
            <span class="filter-indicator-text">
              已筛选
              <template v-if="filterType !== null">分类「{{ filterType }}」</template>
              <template v-if="filterType !== null && filterPriority !== null">、</template>
              <template v-if="filterPriority !== null">优先级 {{ filterPriority }}</template>
            </span>
            <el-button size="small" link type="primary" @click="clearAllFilters"
              >清除筛选</el-button
            >
          </div>

          <div class="item-section">
            <div class="item-section-header">
              <div class="item-section-title-wrap">
                <h3 class="item-section-title">任务列表</h3>
                <span class="count-badge">{{ displayActiveItems.length }}</span>
              </div>
            </div>

            <div v-if="displayActiveItems.length === 0" class="todo-empty">
              <div class="todo-empty-icon">
                <svg width="48" height="48" viewBox="0 0 48 48" fill="none">
                  <rect
                    x="8"
                    y="12"
                    width="32"
                    height="28"
                    rx="4"
                    stroke="currentColor"
                    stroke-width="1.5"
                    opacity="0.25"
                  />
                  <path
                    d="M16 8v8M32 8v8"
                    stroke="currentColor"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    opacity="0.25"
                  />
                  <path
                    d="M19 26l3 3 7-7"
                    stroke="var(--lc-accent)"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              </div>
              <span class="todo-empty-text">{{
                hasActiveFilter ? "当前筛选条件下暂无任务" : "一切安好，暂无任务"
              }}</span>
            </div>
            <div v-else class="todo-card-list">
              <div
                v-for="(row, index) in displayActiveItems"
                :key="row.id"
                class="todo-card"
                :class="[
                  'priority-stripe-' + row.priority.toLowerCase(),
                  {
                    'is-pinned': row.pinned,
                    'is-overdue-card': isItemOverdue(row),
                    'is-selected': selectedItemId === row.id,
                    'is-quick-add-highlight': quickAddHighlightId === row.id,
                  },
                ]"
                :style="{ '--item-index': index }"
                @click="selectItem(row)"
                @dblclick="enterEditMode(row)"
                @contextmenu.prevent="openTodoContextMenu($event, row)"
              >
                <div class="todo-card-check" @click.stop>
                  <el-checkbox
                    :model-value="isDoneItem(row)"
                    :disabled="!row.status"
                    @change="onCheckItem(row)"
                  />
                </div>
                <div class="todo-card-body">
                  <div class="todo-card-top">
                    <span class="todo-card-title">{{ row.title }}</span>
                    <span class="todo-card-badges">
                      <span v-if="row.pinned" class="item-badge badge-pinned" title="置顶">
                        <el-icon :size="11"><Top /></el-icon>
                      </span>
                      <span v-if="hasRepeatRule(row)" class="item-badge badge-repeat" title="重复">
                        <el-icon :size="11"><Refresh /></el-icon>
                      </span>
                      <span v-if="isItemOverdue(row)" class="item-badge badge-overdue" title="逾期">
                        <el-icon :size="11"><AlarmClock /></el-icon>
                      </span>
                    </span>
                  </div>
                  <div class="todo-card-meta">
                    <span
                      v-if="relativeTimeLabel(row)"
                      class="meta-chip meta-time"
                      :class="{ 'is-overdue': isItemOverdue(row) }"
                    >
                      <el-icon :size="12"><Calendar /></el-icon>
                      {{ relativeTimeLabel(row) }}
                    </span>
                    <span v-if="row.typeName" class="meta-chip meta-type">
                      <span
                        class="color-dot-sm"
                        :style="{ backgroundColor: row.typeColor || '#909399' }"
                      />
                      {{ row.typeName }}
                    </span>
                    <span v-if="row.projectName" class="meta-chip meta-project">
                      <span
                        class="color-dot-sm"
                        :style="{ backgroundColor: row.projectColor || '#909399' }"
                      />
                      {{ row.projectName }}
                    </span>
                    <span
                      v-if="row.pmItemId && row.pmItemTitle"
                      class="meta-chip meta-pm-link"
                      @click.stop="navigateToPmItem(row.pmItemId!, row.pmItemProjectId ?? null)"
                    >
                      <el-tag size="small" effect="plain" :style="pmItemTagStyle(row.pmItemStatus)">
                        {{ pmStatusLabel(row.pmItemStatus) }}
                      </el-tag>
                      {{ row.pmItemTitle }}
                    </span>
                    <span v-if="row.assignees.length > 0" class="meta-chip meta-assignee">
                      <el-icon :size="12"><User /></el-icon>
                      {{ row.assignees.map((a: TodoAssignee) => a.name).join("、") }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="item-section">
            <div class="item-section-header done-section-header" @click="toggleRecentWeekCollapsed">
              <div class="item-section-title-wrap">
                <h3 class="item-section-title done-title">最近一周已办</h3>
                <span class="count-badge is-muted">{{ displayRecentWeekItems.length }}</span>
              </div>
              <span class="done-toggle-icon" :class="{ 'is-collapsed': recentWeekCollapsed }">
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                  <path
                    d="M4 6l4 4 4-4"
                    stroke="currentColor"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              </span>
            </div>

            <div v-if="displayRecentWeekItems.length === 0" class="todo-empty is-muted">
              <span class="todo-empty-text">{{
                hasActiveFilter ? "当前筛选条件下最近一周暂无已办事项" : "最近一周暂无已办事项"
              }}</span>
            </div>
            <div v-else v-show="!recentWeekCollapsed" class="todo-card-list is-done-list">
              <div
                v-for="(row, index) in displayRecentWeekItems"
                :key="row.id"
                class="todo-card is-done-card"
                :class="[
                  'priority-stripe-' + row.priority.toLowerCase(),
                  { 'is-selected': selectedItemId === row.id },
                ]"
                :style="{ '--item-index': index }"
                @click="selectItem(row)"
                @dblclick="enterEditMode(row)"
              >
                <div class="todo-card-check" @click.stop>
                  <el-checkbox
                    :model-value="isDoneItem(row)"
                    :disabled="!row.status"
                    @change="onCheckItem(row)"
                  />
                </div>
                <div class="todo-card-body">
                  <div class="todo-card-top">
                    <span class="todo-card-title is-done">{{ row.title }}</span>
                    <span class="todo-card-badges">
                      <span v-if="hasRepeatRule(row)" class="item-badge badge-repeat" title="重复">
                        <el-icon :size="11"><Refresh /></el-icon>
                      </span>
                    </span>
                  </div>
                  <div class="todo-card-meta">
                    <span v-if="relativeDoneTimeLabel(row)" class="meta-chip meta-time">
                      <el-icon :size="12"><Calendar /></el-icon>
                      {{ relativeDoneTimeLabel(row) }}
                    </span>
                    <span v-if="row.typeName" class="meta-chip meta-type">
                      <span
                        class="color-dot-sm"
                        :style="{ backgroundColor: row.typeColor || '#909399' }"
                      />
                      {{ row.typeName }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="item-section">
            <div class="item-section-header done-section-header" @click="toggleDoneCollapsed">
              <div class="item-section-title-wrap">
                <h3 class="item-section-title done-title">已办事项</h3>
                <span class="count-badge is-muted">{{ displayDoneItems.length }}</span>
              </div>
              <span class="done-toggle-icon" :class="{ 'is-collapsed': doneCollapsed }">
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                  <path
                    d="M4 6l4 4 4-4"
                    stroke="currentColor"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              </span>
            </div>

            <div v-if="displayDoneItems.length === 0" class="todo-empty is-muted">
              <span class="todo-empty-text">{{
                hasActiveFilter ? "当前筛选条件下暂无已办事项" : "暂无已办事项"
              }}</span>
            </div>
            <div v-else v-show="!doneCollapsed" class="todo-card-list is-done-list">
              <div
                v-for="(row, index) in displayDoneItems"
                :key="row.id"
                class="todo-card is-done-card"
                :class="[
                  'priority-stripe-' + row.priority.toLowerCase(),
                  { 'is-selected': selectedItemId === row.id },
                ]"
                :style="{ '--item-index': index }"
                @click="selectItem(row)"
                @dblclick="enterEditMode(row)"
              >
                <div class="todo-card-check" @click.stop>
                  <el-checkbox
                    :model-value="isDoneItem(row)"
                    :disabled="!row.status"
                    @change="onCheckItem(row)"
                  />
                </div>
                <div class="todo-card-body">
                  <div class="todo-card-top">
                    <span class="todo-card-title is-done">{{ row.title }}</span>
                    <span class="todo-card-badges">
                      <span v-if="hasRepeatRule(row)" class="item-badge badge-repeat" title="重复">
                        <el-icon :size="11"><Refresh /></el-icon>
                      </span>
                    </span>
                  </div>
                  <div class="todo-card-meta">
                    <span v-if="relativeDoneTimeLabel(row)" class="meta-chip meta-time" title="完成时间">
                      <el-icon :size="12"><Calendar /></el-icon>
                      {{ relativeDoneTimeLabel(row) }}
                    </span>
                    <span v-if="row.typeName" class="meta-chip meta-type">
                      <span
                        class="color-dot-sm"
                        :style="{ backgroundColor: row.typeColor || '#909399' }"
                      />
                      {{ row.typeName }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
        <div v-if="viewMode === 'calendar'" class="todo-calendar-view">
          <TodoCalendarGrid
            :items="allItemsForCalendar"
            :month="calendarMonth"
            :selected-item-id="selectedItemId"
            @select-item="selectItem"
            @create-on-date="createOnDate"
            @prev-month="calendarMonth = calPrevMonth(calendarMonth)"
            @next-month="calendarMonth = calNextMonth(calendarMonth)"
            @go-today="calendarMonth = new Date()"
          />
        </div>
      </section>

      <aside class="todo-detail-pane" :key="detailMode">
        <template v-if="detailMode === 'create' || detailMode === 'edit'">
          <TodoDetailEdit
            ref="todoDetailEditRef"
            :mode="detailMode === 'create' ? 'create' : 'edit'"
            :draft="itemDraft"
            :selected-item="selectedItem"
            :show-more-fields="showMoreFields"
            :pm-link-item-id="todoPmLinkItemId"
            :sorted-types="sortedTypes"
            :assignees="assignees"
            :project-options="projectOptions"
            :pm-candidates="todoPmCandidates"
            :priority-options="priorityOptions"
            :reminder-preset-options="reminderPresetOptions"
            :repeat-preset-options="repeatPresetOptions"
            :weekday-options="weekdayOptions"
            :hour-options="hourOptions"
            :minute-options="minuteOptions"
            :time-hour="eventHour"
            :time-minute="eventMinute"
            :action-definitions="actionDefinitions"
            :action-targets="actionTargets"
            @title-enter="onTitleEnter"
            @toggle-more-fields="showMoreFields = !showMoreFields"
            @pm-select-change="handlePmSelectChange"
            @pm-project-change="handlePmProjectChange"
            @pm-create="handlePmCreate"
            @pm-search="handlePmSearch"
            @navigate-to-pm="navigateToPmItem"
            @event-date-change="(v) => { if (!v) clearEventSchedule(); else itemDraft.eventDate = v; }"
            @event-hour-change="(v) => { const { minute } = splitDraftEventTime(itemDraft.eventTime); itemDraft.eventTime = composeDraftEventTime(v, minute); }"
            @event-minute-change="(v) => { const { hour } = splitDraftEventTime(itemDraft.eventTime); itemDraft.eventTime = composeDraftEventTime(hour, v); }"
            @fill-quick-date="fillQuickDate"
            @fill-default-date-time="fillDefaultDateTime"
            @clear-event-schedule="clearEventSchedule"
            @reminder-presets-change="onReminderPresetsChange"
            @repeat-preset-change="onRepeatPresetChange"
            @action-type-change="handleActionTypeChange"
            @navigate-to-tool="navigateToTool"
            @custom-frequency-change="onCustomFrequencyChange"
            @toggle-pin="toggleItemPin"
            @change-status="(id, status) => changeItemStatus(id, status as TodoStatus)"
            @delete="deleteItem"
            @cancel="cancelDetailEdit"
            @save="saveItem"
          />
        </template>
        <template v-else-if="detailMode === 'view' && selectedItem !== null">
          <TodoDetailView
            :item="selectedItem"
            :latest-dispatch="latestDispatch"
            @edit="enterEditMode"
            @toggle-pin="toggleItemPin"
            @change-status="(id, status) => changeItemStatus(id, status as TodoStatus)"
            @delete="deleteItem"
            @copy-title="copyTitle"
            @open-link="openLink"
            @navigate-to-pm="navigateToPmItem"
            @dispatch-action="handleDispatchAction"
          />
        </template>
        <TodoEmptyState
          v-else
          :today-due-count="todayDueCount"
          :overdue-count="overdueCount"
          @create="startCreate"
          @refresh="loadItems"
        />
      </aside>
    </div>

    <TodoBasicsDialog
      v-model="basicsDialogVisible"
      :types="types"
      :assignees="assignees"
      @refresh="onBasicsChanged"
    />

    <el-dialog v-model="pmCreateDialogVisible" title="新建工作项" width="420px" @closed="onPmCreateClosed">
      <el-form>
        <el-form-item v-if="!itemDraft.projectId" label="所属项目">
          <el-select v-model="pmCreateProjectId" placeholder="请选择项目" style="width: 100%">
            <el-option v-for="p in projectOptions" :key="p.id" :label="p.name" :value="p.id" />
          </el-select>
        </el-form-item>
        <el-form-item label="标题">
          <el-input v-model.trim="pmCreateTitle" placeholder="请输入工作项标题" @keyup.enter="onPmCreateConfirm" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="pmCreateDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="onPmCreateConfirm">创建并关联</el-button>
      </template>
    </el-dialog>

    <TodoContextMenu
      :visible="todoContextMenu.visible && !!todoContextMenuItem"
      :x="todoContextMenu.x"
      :y="todoContextMenu.y"
      :pinned="!!todoContextMenuItem?.pinned"
      @close="closeTodoContextMenu"
      @select="handleTodoContextMenuCommand"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, inject, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  AlarmClock,
  Calendar,
  Document,
  Grid,
  Plus,
  Refresh,
  Top,
  User,
} from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../../bridge/tauri";
import { APP_EVENTS } from "../../bridge/events";
import {
  useClipboardSuggestion,
  type PendingToolInput,
} from "../../composables/useClipboardSuggestion";
import type {
  TodoAssignee,
  TodoEndMode,
  TodoItem,
  TodoKind,
  TodoPriority,
  TodoRecurrence,
  TodoReminderPreset,
  TodoRepeatPreset,
  TodoRuleMode,
  TodoSimpleRule,
  TodoStatus,
  TodoType,
} from "../../types";
import { useTabs } from "../../composables/useTabs";
import { usePmNavigation } from "../../composables/usePmNavigation";
import { useTodoNavigation } from "../../composables/useTodoNavigation";
import { useTodoItemFilters } from "../../composables/useTodoItemFilters";
import { useTodoScheduleFields } from "../../composables/useTodoScheduleFields";
import { useTodoCrudActions } from "../../composables/useTodoCrudActions";
import { useTodoPmLink } from "../../composables/useTodoPmLink";
import { useTodoDetailState } from "../../composables/useTodoDetailState";
import { useTodoActionBinding } from "../../composables/useTodoActionBinding";
import type { QuickAddContext } from "../../utils/todoQuickAdd";
import { formatTodoRelativeDateTimeLabel } from "../../utils/todoRelativeDate";
import {
  prevMonth as calPrevMonth,
  nextMonth as calNextMonth,
  formatDateKey,
} from "../../utils/calendarGrid";
import TodoCalendarGrid from "./TodoCalendarGrid.vue";
import TodoDetailView from "./TodoDetailView.vue";
import TodoDetailEdit from "./TodoDetailEdit.vue";
import TodoContextMenu from "./TodoContextMenu.vue";
import type { TodoContextMenuCommand } from "./TodoContextMenu.vue";
import TodoBasicsDialog from "./TodoBasicsDialog.vue";
import TodoSidebar from "./TodoSidebar.vue";
import TodoQuickAddBar from "./TodoQuickAddBar.vue";
import TodoEmptyState from "./TodoEmptyState.vue";
import {
  getCreateDraftDefaultDateTime,
} from "../../utils/todoSchedule";

type SelectTypeValue = number | string | undefined;
type SelectAssigneeValue = number | string;
type ItemDialogMode = "create" | "edit_item";
type DetailMode = "empty" | "view" | "edit" | "create";

const items = ref<TodoItem[]>([]);
const types = ref<TodoType[]>([]);
const assignees = ref<TodoAssignee[]>([]);
const projectOptions = ref<{ id: number; name: string; color: string }[]>([]);
const filterProjectId = ref<number | string | null>(null);
const showMoreFields = ref(false);
const itemKeyword = ref("");
const todoDetailEditRef = ref<{
  focusTitleInput: () => void;
  focusScheduleInput: () => void;
  runAfterSubmit?: (realId: number) => Promise<void>;
  runBeforeSubmit?: () => Promise<void>;
  runOnCancel?: () => Promise<void>;
} | null>(null);
const filterType = ref<string | null>(null);
const filterPriority = ref<TodoPriority | null>(null);
const doneCollapsed = ref(true);
const initialLoading = ref(true);
const recentWeekCollapsed = ref(true);
const basicsDialogVisible = ref(false);
const viewMode = ref<"list" | "calendar">("list");
const calendarMonth = ref(new Date());
const itemDialogMode = ref<ItemDialogMode>("create");
const detailMode = ref<DetailMode>("empty");
const selectedItemId = ref<number | null>(null);
const draftBaseline = ref("");
const editingItemSnapshot = ref<TodoItem | null>(null);
const defaultReminderPresets: TodoReminderPreset[] = ["none"];
const lastReminderPresetSelection = ref<TodoReminderPreset[]>([...defaultReminderPresets]);
const todoContextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  itemId: null as number | null,
});
let reminderUnlisten: UnlistenFn | null = null;
const { watchPendingToolInput } = useClipboardSuggestion();
const { openTab } = useTabs();
const { requestFocus: requestPmFocus } = usePmNavigation();
const { consumeFocus: consumeTodoFocus } = useTodoNavigation();

function normalizePendingText(text: string): string {
  return text.replace(/\r\n?/g, "\n").trim();
}

function deriveTodoTitleFromText(text: string): string {
  const lines = text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length === 0) return "来自收纳箱";
  const firstLine = lines[0];
  return firstLine.length > 80 ? `${firstLine.slice(0, 80)}...` : firstLine;
}

function buildTodoDraftFromPendingInput(input: PendingToolInput): {
  title: string;
  description: string;
} {
  const text = normalizePendingText(input.text || "");
  const explicitTitle = input.todoDraft?.title?.trim() || "";
  const explicitDescription = input.todoDraft?.description?.trim() || "";
  const itemType = typeof input.meta?.itemType === "string" ? input.meta.itemType : "";
  const isPlainLikeContent =
    !itemType || itemType === "text" || itemType === "html" || itemType === "rtf";

  if (explicitTitle || explicitDescription) {
    return {
      title: explicitTitle || deriveTodoTitleFromText(text),
      description: explicitDescription,
    };
  }

  if (!text) {
    const fallbackTitle = (input.label || "").trim() || "来自收纳箱";
    return { title: fallbackTitle, description: "" };
  }

  if (isPlainLikeContent) {
    const lines = text
      .split("\n")
      .map((line) => line.trimEnd())
      .filter((line, index, list) => line.length > 0 || index < list.length - 1);
    const [firstLine = "", ...rest] = lines;
    return {
      title: firstLine.trim() || deriveTodoTitleFromText(text),
      description: rest.join("\n").trim(),
    };
  }

  const descriptionParts: string[] = [];
  const fallbackTitle =
    (input.label || "").trim() ||
    (typeof input.meta?.title === "string" ? input.meta.title.trim() : "") ||
    deriveTodoTitleFromText(text);
  if (text && text !== fallbackTitle) {
    descriptionParts.push(text);
  }
  if (typeof input.meta?.openPath === "string" && input.meta.openPath.trim()) {
    descriptionParts.push(`路径：${input.meta.openPath.trim()}`);
  }
  return {
    title: fallbackTitle,
    description: descriptionParts.join("\n\n").trim(),
  };
}

async function applyPendingTodoInput(input: PendingToolInput) {
  if (!(await ensureDetailCanLeave())) return;
  const draft = buildTodoDraftFromPendingInput(input);
  resetItemDraft();
  itemDraft.title = draft.title;
  itemDraft.description = draft.description;
  itemDialogMode.value = "create";
  detailMode.value = "create";
  selectedItemId.value = null;
  showMoreFields.value = false;
  markDraftBaseline();
  await focusCreateTitleInput();
}

const priorityOptions = [
  { value: "P0", label: "P0 - 紧急" },
  { value: "P1", label: "P1 - 高" },
  { value: "P2", label: "P2 - 中" },
  { value: "P3", label: "P3 - 低" },
];

const initialCreateSchedule = getCreateDraftDefaultDateTime();

const itemDraft = reactive({
  id: 0,
  rootId: 0,
  title: "",
  typeId: undefined as SelectTypeValue,
  priority: "P2" as TodoPriority,
  description: "",
  assigneeIds: [] as SelectAssigneeValue[],
  links: [] as { url: string; title: string }[],
  eventDate: "",
  eventTime: "",
  reminderPresets: [...defaultReminderPresets] as TodoReminderPreset[],
  repeatPreset: "none" as TodoRepeatPreset,
  ruleMode: "simple" as TodoRuleMode,
  timezone: "local",
  cronExpression: "0 0 9 * * Mon-Fri",
  endMode: "never" as TodoEndMode,
  endValueDate: "",
  endValueCount: 1,
  simple: {
    frequency: "daily" as TodoSimpleRule["frequency"],
    interval: 1,
    time: "",
    weekdays: [1, 2, 3, 4, 5] as number[],
    dayOfMonth: 1,
  },
  projectId: null as number | null,
  pmItemId: null as number | null,
  pmItemTitle: null as string | null,
  pmItemProjectId: null as number | null,
  pmItemStatus: null as string | null,
  actionType: null as string | null,
  actionTargetId: null as string | null,
});

const {
  actionDefinitions,
  actionTargets,
  latestDispatch,
  loadDefinitions,
  loadTargets,
  onActionTypeChange,
  loadLatestDispatch,
  clearLatestDispatch,
  isAvailableTarget,
  dispatchTodoAction,
} = useTodoActionBinding(itemDraft);

const {
  reminderPresetOptions,
  hourOptions,
  minuteOptions,
  repeatPresetOptions,
  weekdayOptions,
  splitDraftEventTime,
  composeDraftEventTime,
  isRepeating,
  showRecurrenceFields,
  showCustomRepeatFields,
  showCronRepeatFields,
  eventHour,
  eventMinute,
  buildRulePayload,
  buildEndValue,
  buildEventAt,
  syncSimpleDraftFromRule,
  onRepeatPresetChange,
  onCustomFrequencyChange,
  onReminderPresetsChange,
  clearEventSchedule,
  fillDefaultDateTime,
  fillQuickDate,
} = useTodoScheduleFields({
  itemDraft,
  lastReminderPresetSelection,
  editingItemSnapshot,
  itemDialogMode,
  itemKindOf,
});

const {
  sortedTypes,
  activeItems,
  recentWeekItems,
  doneItems,
  hasActiveFilter,
  displayActiveItems,
  displayRecentWeekItems,
  displayDoneItems,
  todayDueCount,
  overdueCount,
  clearAllFilters,
} = useTodoItemFilters({
  items,
  types,
  itemKeyword,
  filterType,
  filterPriority,
  itemScheduleAt,
  isItemOverdue,
});

const todoContextMenuItem = computed(() =>
  todoContextMenu.itemId == null
    ? null
    : items.value.find((item) => item.id === todoContextMenu.itemId) || null,
);
const allItemsForCalendar = computed(() => items.value);

const quickAddContext = computed<QuickAddContext>(() => ({
  typeId:
    filterType.value === null
      ? null
      : types.value.find((t) => t.name === filterType.value)?.id ?? null,
  projectId: typeof filterProjectId.value === "number" ? filterProjectId.value : null,
  priorityDefault: filterPriority.value ?? "P2",
}));

const quickAddHighlightId = ref<number | null>(null);
let quickAddHighlightTimer: ReturnType<typeof setTimeout> | null = null;

function isItemVisibleInList(id: number) {
  return displayActiveItems.value.some((row) => row.id === id);
}

async function onQuickAddCreated(id: number) {
  await loadItems();
  if (!isItemVisibleInList(id)) {
    ElMessage.info("已添加，当前筛选/搜索条件下不可见");
    return;
  }
  quickAddHighlightId.value = id;
  if (quickAddHighlightTimer) clearTimeout(quickAddHighlightTimer);
  quickAddHighlightTimer = setTimeout(() => {
    quickAddHighlightId.value = null;
    quickAddHighlightTimer = null;
  }, 1500);
}

function closeTodoContextMenu() {
  todoContextMenu.visible = false;
  todoContextMenu.itemId = null;
}

async function openTodoContextMenu(event: MouseEvent, item: TodoItem) {
  event.preventDefault();
  event.stopPropagation();
  closeTodoContextMenu();
  if (!(await prepareItemForInlineAction(item))) return;
  todoContextMenu.itemId = item.id;
  todoContextMenu.visible = true;
  todoContextMenu.x = event.clientX;
  todoContextMenu.y = event.clientY;
}

async function enterEditTimeMode(item?: TodoItem | null) {
  const target = item || selectedItem.value;
  if (!target) return;
  await enterEditMode(target, { focusTitle: false });
  if (selectedItemId.value !== target.id) return;
  showMoreFields.value = true;
  await nextTick();
  todoDetailEditRef.value?.focusScheduleInput();
}

async function handleTodoContextMenuCommand(command: TodoContextMenuCommand) {
  const item = todoContextMenuItem.value;
  if (!item) return;
  closeTodoContextMenu();
  switch (command) {
    case "pin":
      await toggleItemPin(item.id);
      break;
    case "complete":
      await changeItemStatus(item.id, "completed");
      break;
    case "edit-time":
      await enterEditTimeMode(item);
      break;
    case "delete":
      await deleteItem(item);
      break;
  }
}

function onTitleEnter(event: KeyboardEvent) {
  if (event.isComposing) return;
  const isCreateForm = detailMode.value === "create" && itemDialogMode.value === "create";
  const isEditForm = detailMode.value === "edit" && itemDialogMode.value === "edit_item";
  if (!isCreateForm && !isEditForm) return;
  void saveItem();
}

function formatDate(value?: string | null) {
  if (!value) return "-";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleString("zh-CN", {
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      });
}

function isActionableStatus(status: TodoStatus) {
  return status === "pending" || status === "in_progress";
}

function priorityTagType(priority: TodoPriority) {
  return ({ P0: "danger", P1: "warning", P2: "primary", P3: "info" }[priority] || "info") as
    | "danger"
    | "warning"
    | "primary"
    | "info";
}

function priorityCardClass(priority: TodoPriority): "danger" | "warning" | "primary" | "" {
  return ({ P0: "danger", P1: "warning", P2: "primary", P3: "" }[priority] || "") as
    | "danger"
    | "warning"
    | "primary"
    | "";
}

function priorityLabel(priority: TodoPriority): string {
  return { P0: "紧急", P1: "高", P2: "中", P3: "低" }[priority] || "中";
}

function itemKindOf(item: TodoItem): TodoKind {
  return item.kind;
}

function hasRepeatRule(item: TodoItem): boolean {
  return item.kind === "recurring" && !!item.recurrence;
}

function getItemRecurrence(item: TodoItem): TodoRecurrence | null {
  return item.recurrence;
}

function isDoneItem(item: TodoItem) {
  return item.status === "completed";
}

function canPinItem(item: TodoItem) {
  return isActionableStatus(item.status);
}

function truncateDescription(desc: string, maxLen = 40): string {
  if (desc.length <= maxLen) return desc;
  return desc.slice(0, maxLen) + "...";
}

function itemScheduleAt(item: TodoItem) {
  return item.eventAt;
}

function isItemOverdue(item: TodoItem): boolean {
  const time = itemScheduleAt(item);
  if (!time || !isActionableStatus(item.status)) return false;
  return new Date(time).getTime() < Date.now();
}

function todoRowClassName({ row }: { row: TodoItem }) {
  return "todo-row-" + row.priority.toLowerCase();
}

function doneRowClassName({ row }: { row: TodoItem }) {
  return "todo-row-" + row.priority.toLowerCase() + " is-done-row";
}

function relativeTimeLabel(item: TodoItem): string {
  return formatTodoRelativeDateTimeLabel(itemScheduleAt(item));
}

// 最近一周/已办列表以 completedAt 作为真实完成时间展示
function relativeDoneTimeLabel(item: TodoItem): string {
  const doneAt = (item.completedAt || "").trim();
  return formatTodoRelativeDateTimeLabel(doneAt) || (doneAt ? formatDate(doneAt) : "");
}

function toggleDoneCollapsed() {
  doneCollapsed.value = !doneCollapsed.value;
}

function toggleRecentWeekCollapsed() {
  recentWeekCollapsed.value = !recentWeekCollapsed.value;
}

function onCheckItem(item: TodoItem) {
  if (!item.status) return;
  void changeItemStatus(item.id, isDoneItem(item) ? "pending" : "completed");
}

function disabledFiveMinuteMinutes(..._args: unknown[]) {
  return Array.from({ length: 60 }, (_, index) => index).filter((minute) => minute % 5 !== 0);
}

function disabledAllSeconds(..._args: unknown[]) {
  return Array.from({ length: 60 }, (_, index) => index);
}

const {
  loadTypes,
  loadAssignees,
  loadItems,
  loadProjects,
  submitItemChanges,
  changeItemStatus,
  toggleItemPin,
  openLink,
  snoozeItem,
  deleteItem,
  onBasicsChanged,
} = useTodoCrudActions({
  items,
  types,
  assignees,
  projectOptions,
  filterProjectId,
  itemDraft,
  itemDialogMode,
  todoDetailEditRef,
  closeTodoContextMenu,
  isRepeating,
  showRecurrenceFields,
  showCustomRepeatFields,
  showCronRepeatFields,
  buildEventAt,
  buildRulePayload,
  buildEndValue,
  isAvailableActionTarget: isAvailableTarget,
});

const {
  todoPmLinkItemId,
  todoPmCandidates,
  skipProjectWatch,
  todoLinkedPmItem,
  pmCreateDialogVisible,
  pmCreateTitle,
  pmCreateProjectId,
  pmStatusLabel,
  loadTodoPmCandidates,
  onPmCreateConfirm,
  onPmCreateClosed,
  handlePmSelectChange,
  handlePmProjectChange,
  handlePmCreate,
  handlePmSearch,
  navigateToPmItem,
  pmItemTagStyle,
} = useTodoPmLink({
  itemDraft,
  itemDialogMode,
  selectedItemId,
  submitItemChanges,
  loadItems,
  requestPmFocus,
  openTab,
});

const {
  selectedItem,
  isDetailEditing,
  isDraftDirty,
  markDraftBaseline,
  ensureDetailCanLeave,
  finalizeDetailAfterSave,
  selectItem,
  prepareItemForInlineAction,
  focusCreateTitleInput,
  startCreate,
  createOnDate,
  cancelDetailEdit,
  resetItemDraft,
  enterEditMode: enterEditModeBase,
} = useTodoDetailState({
  items,
  itemDraft,
  detailMode,
  itemDialogMode,
  selectedItemId,
  draftBaseline,
  editingItemSnapshot,
  showMoreFields,
  lastReminderPresetSelection,
  defaultReminderPresets,
  todoDetailEditRef,
  todoPmLinkItemId,
  todoPmCandidates,
  todoLinkedPmItem,
  skipProjectWatch,
  loadTodoPmCandidates,
  submitItemChanges,
  syncSimpleDraftFromRule,
  itemKindOf,
  hasRepeatRule,
  getItemRecurrence,
});

async function enterEditMode(
  item?: TodoItem | null,
  options: { focusTitle?: boolean } = {},
) {
  await enterEditModeBase(item, options);
  if (!itemDraft.actionType) return;
  try {
    await loadTargets(itemDraft.actionType);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function handleActionTypeChange(actionType: string | null) {
  try {
    await onActionTypeChange(actionType);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function navigateToTool(toolId: string) {
  if (toolId === "release-package") {
    openTab("release-package", "上线包打包");
  }
}

async function handleDispatchAction(item: TodoItem) {
  try {
    await dispatchTodoAction(item, { triggerEventId: undefined });
    await loadLatestDispatch(item.id);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function copyTitle(title: string) {
  await navigator.clipboard.writeText(title);
  ElMessage.success("标题已复制");
}

async function saveItem() {
  const result = await submitItemChanges(true);
  if (!result.ok) return;
  finalizeDetailAfterSave(result.id);
}

watch(filterProjectId, () => loadItems());

watch(viewMode, () => {
  closeTodoContextMenu();
});

watch(todoContextMenuItem, (item) => {
  if (!todoContextMenu.visible || item) return;
  closeTodoContextMenu();
});

watch(
  selectedItem,
  (item) => {
    if (!item?.actionBinding) {
      clearLatestDispatch();
      return;
    }
    void loadLatestDispatch(item.id).catch((error) => {
      clearLatestDispatch();
      ElMessage.error((error as Error).message);
    });
  },
  { immediate: true },
);

watchPendingToolInput("todo", (input) => applyPendingTodoInput(input));

const pendingTodoCreate = inject<ReturnType<typeof ref<boolean>>>("pendingTodoCreate", ref(false));
watch(pendingTodoCreate, (v) => {
  if (v) {
    pendingTodoCreate.value = false;
    void startCreate();
  }
});

onMounted(async () => {
  await Promise.all([
    loadTypes(),
    loadAssignees(),
    loadItems(),
    loadProjects(),
    loadDefinitions(),
  ]);
  initialLoading.value = false;
  const focus = consumeTodoFocus();
  if (focus) {
    const target = items.value.find((i) => i.id === focus.itemId);
    if (target) {
      selectItem(target);
    } else {
      ElMessage.warning("未找到该任务，可能已被删除");
    }
  }
  try {
    reminderUnlisten = await listen(APP_EVENTS.TODO_REMINDER_FIRED, async () => {
      await loadItems();
    });
  } catch {
    reminderUnlisten = null;
  }
});

onBeforeUnmount(() => {
  closeTodoContextMenu();
  reminderUnlisten?.();
  reminderUnlisten = null;
  if (quickAddHighlightTimer) {
    clearTimeout(quickAddHighlightTimer);
    quickAddHighlightTimer = null;
  }
});
</script>

<style scoped>
.todo-panel {
  height: 100%;
  min-height: 0;
}
.toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.toolbar-right {
  display: flex;
  gap: 10px;
  align-items: center;
}
/* --- Quick add bar --- */
.todo-quick-add-bar {
  margin-bottom: 12px;
}
.todo-card.is-quick-add-highlight {
  animation: quickAddHighlightFade 1.5s var(--lc-ease-out) both;
}
@keyframes quickAddHighlightFade {
  0% {
    background: rgba(52, 211, 153, 0.16);
    border-color: var(--lc-success);
  }
  60% {
    background: rgba(52, 211, 153, 0.08);
  }
}
/* --- Section headers --- */
.item-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}
.item-section:last-child {
  margin-bottom: 0;
}
.item-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 6px;
}
.item-section-title-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}
.item-section-title {
  margin: 0;
  font-family: var(--lc-font-display);
  font-size: 15px;
  font-weight: 600;
  color: var(--lc-text);
  letter-spacing: 0.3px;
}
.done-title {
  color: var(--lc-text-secondary);
}
.count-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 22px;
  height: 20px;
  padding: 0 7px;
  border-radius: 10px;
  font-family: var(--lc-font-body);
  font-size: 12px;
  font-weight: 600;
  color: var(--lc-accent);
  background: var(--lc-accent-dim);
}
.count-badge.is-muted {
  color: var(--lc-text-muted);
  background: rgba(255, 255, 255, 0.04);
}

/* --- Empty state --- */
.todo-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 48px 16px;
  color: var(--lc-text-muted);
}
.todo-empty.is-muted {
  padding: 24px 16px;
}
.todo-empty-icon {
  color: var(--lc-text-muted);
}
.todo-empty-text {
  font-family: var(--lc-font-body);
  font-size: 13px;
  color: var(--lc-text-muted);
}

/* --- Card list --- */
.todo-card-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* --- Card --- */
.todo-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px 10px 12px;
  border-radius: var(--lc-radius-sm);
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  border-left: 3.5px solid var(--lc-text-muted);
  cursor: pointer;
  transition:
    background var(--lc-duration) var(--lc-ease),
    border-color var(--lc-duration) var(--lc-ease),
    box-shadow var(--lc-duration) var(--lc-ease),
    opacity var(--lc-duration) var(--lc-ease),
    transform var(--lc-duration) var(--lc-ease);
  animation: todoCardSlideIn 0.3s var(--lc-ease-out) calc(var(--item-index, 0) * 25ms) both;
}
.todo-card:hover {
  background: var(--lc-surface-2);
  border-color: var(--lc-border-hover);
  box-shadow: var(--lc-shadow-sm);
  transform: translateY(-1px);
}

/* Priority strips */
.todo-card.priority-stripe-p0 {
  border-left-color: var(--lc-danger);
}
.todo-card.priority-stripe-p1 {
  border-left-color: var(--lc-warning);
}
.todo-card.priority-stripe-p2 {
  border-left-color: var(--lc-accent);
}
.todo-card.priority-stripe-p3 {
  border-left-color: var(--lc-text-muted);
}

/* Pinned highlight */
.todo-card.is-pinned {
  background: linear-gradient(135deg, rgba(52, 211, 153, 0.04), var(--lc-surface-1) 70%);
}
.todo-card.is-pinned:hover {
  background: linear-gradient(135deg, rgba(52, 211, 153, 0.06), var(--lc-surface-2) 70%);
}

/* Overdue glow */
.todo-card.is-overdue-card {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.04), var(--lc-surface-1) 70%);
}
.todo-card.is-overdue-card:hover {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.06), var(--lc-surface-2) 70%);
}

/* Done cards */
.todo-card.is-done-card {
  opacity: 0.5;
  border-left-width: 2.5px;
}
.todo-card.is-done-card:hover {
  opacity: 0.75;
}

.todo-card-check {
  flex-shrink: 0;
  display: flex;
  align-items: center;
}
.todo-card-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 12px;
}
.todo-card-top {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1 1 auto;
  max-width: 100%;
}
.todo-card-title {
  font-family: var(--lc-font-body);
  font-size: 14px;
  font-weight: 500;
  color: var(--lc-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
  min-width: 0;
  max-width: 100%;
}
.todo-card-title.is-done {
  text-decoration: line-through;
  color: var(--lc-text-muted);
  font-weight: 400;
}
.todo-card-badges {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

/* --- Badges --- */
.item-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 16px;
  padding: 0 4px;
  border-radius: 4px;
  font-family: var(--lc-font-body);
  font-size: 11px;
  font-weight: 500;
  line-height: 1;
  white-space: nowrap;
}
.badge-pinned {
  color: var(--lc-success);
  background: rgba(52, 211, 153, 0.12);
}
.badge-overdue {
  color: var(--lc-danger);
  background: rgba(248, 113, 113, 0.12);
}
.badge-repeat {
  color: var(--lc-warning);
  background: rgba(251, 191, 36, 0.12);
}

/* --- Meta chips --- */
.todo-card-meta {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: nowrap;
  flex-shrink: 0;
}
.meta-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-family: var(--lc-font-body);
  font-size: 12px;
  color: var(--lc-text-secondary);
  white-space: nowrap;
}
.meta-chip.is-overdue {
  color: var(--lc-danger);
  font-weight: 600;
}
.meta-pm-link {
  cursor: pointer;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.meta-pm-link:hover {
  opacity: 0.8;
}
.meta-pm-link .el-tag {
  flex-shrink: 0;
}
.color-dot-sm {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}
.priority-dot-sm {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

/* --- Card actions (hover reveal) --- */
.todo-card-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  opacity: 0;
  transition: opacity var(--lc-duration) var(--lc-ease);
}
.todo-card:hover .todo-card-actions {
  opacity: 1;
}
.item-more-btn {
  padding: 4px;
  color: var(--el-text-color-secondary);
}

/* --- Done section --- */
.done-section-header {
  cursor: pointer;
  user-select: none;
  border-radius: 6px;
  padding: 4px 8px;
  margin: -4px -8px;
  transition: background var(--lc-duration) var(--lc-ease);
}
.done-section-header:hover {
  background: rgba(255, 255, 255, 0.02);
}
.done-toggle-icon {
  color: var(--lc-text-muted);
  display: inline-flex;
  align-items: center;
  transition: transform 0.25s var(--lc-ease);
}
.done-toggle-icon.is-collapsed {
  transform: rotate(-90deg);
}

/* --- Layout --- */
.todo-layout {
  display: grid;
  grid-template-columns: 260px minmax(360px, 1.2fr) minmax(300px, 1fr);
  gap: 16px;
  height: 100%;
  min-height: 0;
}
.todo-sidebar,
.todo-list-pane,
.todo-detail-pane {
  min-height: 0;
}
.todo-list-pane {
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.todo-list-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}
.todo-detail-pane {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
}

/* --- Filter indicator --- */
.filter-indicator {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  margin-bottom: 8px;
  font-size: 13px;
  background: var(--lc-accent-dim);
  border: 1px solid rgba(56, 189, 248, 0.15);
  border-radius: 6px;
}
.filter-indicator-text {
  color: var(--lc-text);
}

.todo-card.is-selected {
  box-shadow:
    0 0 0 2px rgba(56, 189, 248, 0.15),
    var(--lc-shadow-sm);
  background: linear-gradient(135deg, rgba(56, 189, 248, 0.06), var(--lc-surface-1) 70%);
  transform: translateX(2px);
}
.todo-card.is-selected .todo-card-actions {
  opacity: 1;
}

/* --- Detail Pane Header (Enhanced) --- */
.detail-pane-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--lc-border);
  background: linear-gradient(180deg, var(--lc-surface-0), var(--lc-surface-1));
}
.detail-title-group {
  min-width: 0;
  width: 100%;
}
.detail-eyebrow {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  color: var(--lc-accent);
  margin-bottom: 8px;
}
.detail-title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  line-height: 1.3;
  color: var(--lc-text);
  word-break: break-word;
  flex: 1;
  min-width: 0;
}
.detail-subtitle {
  margin-top: 4px;
  font-size: 12px;
  color: var(--lc-text-muted);
}
.detail-title-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 10px;
}
.detail-header-actions {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  flex-wrap: wrap;
  gap: 8px;
  width: 100%;
}
.detail-edit-btn {
  color: var(--lc-accent) !important;
}
.detail-edit-btn:hover {
  color: var(--el-color-primary-light-3) !important;
}

/* --- Detail Cards (New Card-based Layout) --- */
.detail-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 16px 20px;
}

/* Detail Card Component */
.detail-card {
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  overflow: hidden;
  transition: box-shadow 0.25s var(--lc-ease);
}

.detail-card:hover {
  box-shadow: var(--lc-shadow-sm);
}


/* Project Unified Card — Scheme E */


/* Detail empty hint */

.detail-empty-info-text {
  font-size: 13px;
  line-height: 1.6;
  color: var(--lc-text);
}

.detail-empty-info-hint {
  font-size: 12px;
  line-height: 1.6;
  color: var(--lc-text-muted);
}

.detail-empty-info-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Detail Grid & Fields */


.detail-field--full {
  grid-column: 1 / -1;
}


/* Detail Description Card */


/* Markdown rendered styles */
.md-rendered :deep(h1) {
  font-size: 1.4em;
  margin: 0.4em 0;
  border-bottom: 1px solid var(--el-border-color);
  padding-bottom: 0.2em;
}

.md-rendered :deep(h2) {
  font-size: 1.2em;
  margin: 0.4em 0;
}

.md-rendered :deep(h3) {
  font-size: 1.05em;
  margin: 0.3em 0;
}

.md-rendered :deep(p) {
  margin: 0.3em 0;
}

.md-rendered :deep(pre) {
  background: var(--el-fill-color);
  padding: 8px 12px;
  border-radius: 6px;
  overflow-x: auto;
  margin: 0.4em 0;
}

.md-rendered :deep(code) {
  font-family: monospace;
  font-size: 0.9em;
}

.md-rendered :deep(p code) {
  background: var(--el-fill-color);
  padding: 1px 4px;
  border-radius: 3px;
}

.md-rendered :deep(ul) {
  padding-left: 1.5em;
  margin: 0.3em 0;
}

.md-rendered :deep(a) {
  color: var(--el-color-primary);
  text-decoration: none;
}

.md-rendered :deep(a:hover) {
  text-decoration: underline;
}

.md-rendered :deep(strong) {
  font-weight: 600;
}

/* Markdown toolbar */


/* Priority & Status Badges in Detail */


/* Priority with Dot */

/* Type with Color */

/* Assignee List */


/* Text Muted */
.text-muted {
  color: var(--lc-text-muted);
}

/* Meta Timestamps */

.meta-label {
  color: var(--lc-text-muted);
}

.meta-divider {
  color: var(--lc-border);
}

/* Detail Footer */


.detail-edit,
.detail-view {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
}

/* --- Animation --- */
@keyframes todoCardSlideIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* Detail View Slide Animation */
.detail-view {
  animation: slideInFromRight 0.3s var(--lc-ease-out);
}

@keyframes slideInFromRight {
  from {
    opacity: 0;
    transform: translateX(20px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

/* Detail Card Hover Effects */
.detail-card {
  animation: cardFadeIn 0.35s var(--lc-ease-out) backwards;
}

.detail-card:nth-child(1) {
  animation-delay: 0ms;
}
.detail-card:nth-child(2) {
  animation-delay: 40ms;
}
.detail-card:nth-child(3) {
  animation-delay: 80ms;
}
.detail-card:nth-child(4) {
  animation-delay: 120ms;
}
.detail-card:nth-child(5) {
  animation-delay: 160ms;
}

@keyframes cardFadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* Smooth transitions for detail pane */
.todo-detail-pane {
  transition: box-shadow 0.3s var(--lc-ease);
}

.todo-detail-pane:hover {
  box-shadow: var(--lc-shadow-md);
}

/* --- Responsive --- */
@media (max-width: 1280px) {
  .todo-layout {
    grid-template-columns: 240px minmax(320px, 1.2fr) minmax(320px, 1fr);
    gap: 14px;
  }
}
@media (max-width: 1024px) {
  .todo-layout {
    grid-template-columns: 220px minmax(300px, 1fr) 300px;
    gap: 12px;
  }
}
@media (max-width: 900px) {
  .todo-layout {
    grid-template-columns: 1fr;
    grid-template-areas:
      "list"
      "detail"
      "stats";
  }
  .todo-list-pane {
    grid-area: list;
  }
  .todo-detail-pane {
    grid-area: detail;
    min-height: 480px;
    border-radius: var(--lc-radius-lg);
  }
  .todo-sidebar {
    grid-area: stats;
    width: 100%;
    overflow: visible;
    padding-right: 0;
  }
}
@media (max-width: 640px) {
  .detail-grid {
    grid-template-columns: 1fr;
  }
  .detail-pane-header,
  .detail-scroll {
    padding: 14px;
  }
}

/* --- Quick date presets --- */


/* --- Custom scrollbar --- */
.todo-list-scroll::-webkit-scrollbar,
.detail-scroll::-webkit-scrollbar {
  width: 4px;
}
.todo-list-scroll::-webkit-scrollbar-thumb,
.detail-scroll::-webkit-scrollbar-thumb {
  background: var(--lc-border);
  border-radius: 2px;
}
.todo-list-scroll::-webkit-scrollbar-thumb:hover,
.detail-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--lc-border-hover);
}
.todo-list-scroll::-webkit-scrollbar-track,
.detail-scroll::-webkit-scrollbar-track {
  background: transparent;
}

/* Link styles */


/* Calendar view */
.todo-calendar-view {
  flex: 1;
  overflow: hidden;
}

/* Toolbar left */
.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

</style>

<style>
.todo-delete-scope-dialog {
  --el-messagebox-width: 580px;
}
</style>
