<template>
  <div class="todo-panel">
    <el-tabs v-model="activeTab">
      <el-tab-pane label="事项" name="items">
        <div class="toolbar">
          <el-select v-model="itemKindFilter" placeholder="事项视图" style="width: 140px">
            <el-option label="全部事项" value="all" />
            <el-option label="待处理事项" value="actionable" />
            <el-option label="周期事项" value="recurring_root" />
            <el-option label="周期实例" value="recurring_occurrence" />
          </el-select>
          <el-select v-model="itemStatusFilter" clearable placeholder="状态筛选" style="width: 140px">
            <el-option label="待办" value="pending" />
            <el-option label="进行中" value="in_progress" />
            <el-option label="已完成" value="completed" />
            <el-option label="已取消" value="canceled" />
          </el-select>
          <el-input v-model.trim="itemKeyword" clearable placeholder="搜索标题或描述" style="width: 220px" />
          <el-button @click="loadItems">刷新</el-button>
          <el-button type="primary" @click="openCreateItemDialog()">新增事项</el-button>
        </div>

        <el-table :data="filteredItems" border stripe>
          <el-table-column prop="title" label="事项" min-width="220">
            <template #default="{ row }">
              <div class="title-cell">
                <span>{{ row.title }}</span>
                <el-tag v-if="isRootItem(row)" size="small" type="warning">周期事项</el-tag>
                <el-tag v-else-if="row.isOverdue" size="small" type="danger">已逾期</el-tag>
              </div>
            </template>
          </el-table-column>
          <el-table-column label="类型" width="110">
            <template #default="{ row }">
              <el-tag :type="itemTagType(row)" effect="plain">
                {{ itemKindLabel(row) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="分类" width="120">
            <template #default="{ row }">{{ row.typeName || "-" }}</template>
          </el-table-column>
          <el-table-column label="优先级" width="90">
            <template #default="{ row }">
              <el-tag :type="priorityTagType(row.priority)">{{ row.priority }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="执行人" min-width="150">
            <template #default="{ row }">{{ row.assignees.map((assignee) => assignee.name).join("、") || "-" }}</template>
          </el-table-column>
          <el-table-column label="规则" min-width="220">
            <template #default="{ row }">{{ isRootItem(row) ? recurrenceSummary(row) : "-" }}</template>
          </el-table-column>
          <el-table-column label="状态" width="100">
            <template #default="{ row }">{{ itemStateLabel(row) }}</template>
          </el-table-column>
          <el-table-column label="时间" min-width="170">
            <template #default="{ row }">{{ itemTimeLabel(row) }}</template>
          </el-table-column>
          <el-table-column label="提醒" width="220">
            <template #default="{ row }">{{ itemReminderLabel(row) }}</template>
          </el-table-column>
          <el-table-column label="操作" min-width="330" fixed="right">
            <template #default="{ row }">
              <template v-if="isRootItem(row)">
                <el-button size="small" text type="primary" class="edit-action-btn" @click="editItem(row)">编辑</el-button>
                <el-button size="small" text @click="toggleRootActive(row)">{{ recurrenceActive(row) ? "停用" : "启用" }}</el-button>
                <el-button size="small" text type="danger" @click="deleteItem(row)">删除</el-button>
              </template>
              <template v-else>
                <el-button v-if="row.status === 'pending'" size="small" text @click="changeItemStatus(row.id, 'in_progress')">开始</el-button>
                <el-button size="small" text type="primary" class="edit-action-btn" @click="editItem(row)">编辑</el-button>
                <el-button v-if="row.status !== 'completed' && row.status !== 'canceled'" size="small" text type="success" @click="changeItemStatus(row.id, 'completed')">完成</el-button>
                <el-button v-if="row.status !== 'completed' && row.status !== 'canceled'" size="small" text type="warning" @click="snoozeItem(row.id, row.nextTaskReminderId)">稍后10分钟</el-button>
                <el-button v-if="row.canEditFuture" size="small" text @click="stopFutureItems(row)">停后续</el-button>
                <el-button size="small" text type="danger" @click="deleteItem(row)">删除</el-button>
              </template>
            </template>
          </el-table-column>
        </el-table>
      </el-tab-pane>

      <el-tab-pane label="提醒中心" name="reminders">
        <div class="toolbar">
          <el-button @click="loadReminders">刷新</el-button>
          <el-button :disabled="reminders.length === 0" @click="markAllRead">全部已读</el-button>
        </div>

        <el-empty v-if="reminders.length === 0" description="暂无未读提醒" />
        <el-table v-else :data="reminders" border stripe>
          <el-table-column prop="title" label="提醒标题" min-width="180" />
          <el-table-column prop="body" label="提醒内容" min-width="220" />
          <el-table-column label="触发时间" min-width="170">
            <template #default="{ row }">{{ formatDate(row.fireAt) }}</template>
          </el-table-column>
          <el-table-column label="操作" width="240" fixed="right">
            <template #default="{ row }">
              <el-button size="small" text type="success" @click="completeFromReminder(row)">完成</el-button>
              <el-button size="small" text type="warning" @click="snoozeFromReminder(row)">稍后10分钟</el-button>
              <el-button size="small" text @click="markRead(row.id)">标记已读</el-button>
            </template>
          </el-table-column>
        </el-table>
      </el-tab-pane>
      <el-tab-pane label="基础数据" name="basics">
        <div class="basic-grid">
          <el-card>
            <template #header>
              <div class="card-header">
                <span>事项分类</span>
                <el-button text type="primary" @click="addType">新增</el-button>
              </div>
            </template>
            <el-table :data="types" size="small" border>
              <el-table-column prop="name" label="名称" min-width="120" />
              <el-table-column prop="color" label="颜色" width="110">
                <template #default="{ row }">
                  <span class="color-dot" :style="{ backgroundColor: row.color || '#409eff' }" />
                  {{ row.color || "-" }}
                </template>
              </el-table-column>
              <el-table-column label="操作" width="160">
                <template #default="{ row }">
                  <el-button size="small" text @click="renameType(row)">编辑</el-button>
                  <el-button size="small" text type="danger" :disabled="row.builtin" @click="removeType(row)">删除</el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-card>

          <el-card>
            <template #header>
              <div class="card-header">
                <span>执行人</span>
                <el-button text type="primary" @click="addAssignee">新增</el-button>
              </div>
            </template>
            <el-table :data="assignees" size="small" border>
              <el-table-column prop="name" label="名称" min-width="120" />
              <el-table-column label="操作" width="160">
                <template #default="{ row }">
                  <el-button size="small" text @click="renameAssignee(row)">编辑</el-button>
                  <el-button size="small" text type="danger" @click="removeAssignee(row)">删除</el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-card>
        </div>
      </el-tab-pane>
    </el-tabs>

    <el-dialog v-model="itemDialogVisible" :title="itemDialogTitle" width="860px" :close-on-click-modal="false" @closed="resetItemDraft">
      <el-form label-width="96px" class="dialog-form">
        <div class="dialog-form-grid">
          <el-form-item v-if="showScopeSelector" label="编辑范围" class="dialog-form-span-2">
            <el-radio-group v-model="itemDraft.scope" @change="onEditScopeChange">
              <el-radio value="this_instance">仅当前一次</el-radio>
              <el-radio value="future_instances">此后未发生项</el-radio>
            </el-radio-group>
          </el-form-item>

          <el-form-item label="标题" class="dialog-form-span-2">
            <el-input v-model.trim="itemDraft.title" placeholder="请输入事项标题" />
          </el-form-item>
          <el-form-item label="分类">
            <el-select v-model="itemDraft.typeId" clearable filterable allow-create default-first-option placeholder="可输入新分类" style="width: 100%">
              <el-option v-for="item in types" :key="item.id" :label="item.name" :value="item.id" />
            </el-select>
          </el-form-item>
          <el-form-item label="优先级">
            <el-select v-model="itemDraft.priority" style="width: 100%">
              <el-option label="P0" value="P0" />
              <el-option label="P1" value="P1" />
              <el-option label="P2" value="P2" />
              <el-option label="P3" value="P3" />
            </el-select>
          </el-form-item>
          <el-form-item label="执行人" class="dialog-form-span-2">
            <el-select v-model="itemDraft.assigneeIds" multiple clearable filterable allow-create default-first-option :reserve-keyword="false" placeholder="可输入新执行人" style="width: 100%">
              <el-option v-for="item in assignees" :key="item.id" :label="item.name" :value="item.id" />
            </el-select>
          </el-form-item>

          <template v-if="showEventTimeField">
            <el-form-item label="日期">
              <el-date-picker
                v-model="itemDraft.singleDate"
                type="date"
                value-format="YYYY-MM-DD"
                style="width: 100%"
              />
            </el-form-item>
            <el-form-item label="时间">
              <el-time-picker
                v-model="itemDraft.singleTime"
                value-format="HH:mm"
                format="HH:mm"
                :disabled-minutes="disabledFiveMinuteMinutes"
                :disabled-seconds="disabledAllSeconds"
                style="width: 100%"
              />
            </el-form-item>
          </template>

          <template v-if="showRecurrenceFields">
            <el-form-item label="开始日期">
              <el-date-picker
                v-model="itemDraft.recurrenceStartDate"
                type="date"
                value-format="YYYY-MM-DD"
                style="width: 100%"
                @change="onRecurrenceStartDateChange"
              />
            </el-form-item>
            <el-form-item label="时间">
              <el-time-picker
                v-model="itemDraft.recurrenceTime"
                value-format="HH:mm"
                format="HH:mm"
                :disabled-minutes="disabledFiveMinuteMinutes"
                :disabled-seconds="disabledAllSeconds"
                style="width: 100%"
                @change="onRecurrenceTimeChange"
              />
            </el-form-item>
          </template>

          <el-form-item label="提醒">
            <el-select v-model="itemDraft.reminderPresets" multiple clearable collapse-tags collapse-tags-tooltip style="width: 100%" @change="onReminderPresetsChange">
              <el-option v-for="option in reminderPresetOptions" :key="option.value" :label="option.label" :value="option.value" />
            </el-select>
          </el-form-item>

          <el-form-item label="事项类型" class="dialog-form-span-2">
            <el-radio-group v-model="itemDraft.isRecurring" :disabled="itemDialogMode !== 'create'" @change="onItemKindChange">
              <el-radio :value="false">单次事项</el-radio>
              <el-radio :value="true">周期事项</el-radio>
            </el-radio-group>
          </el-form-item>

          <template v-if="showRecurrenceFields">
            <el-form-item label="重复" class="dialog-form-span-2">
              <el-radio-group v-model="itemDraft.repeatPreset" class="repeat-radio-group" @change="onRepeatPresetChange">
                <el-radio-button v-for="option in recurrencePresetOptions" :key="option.value" :value="option.value">
                  {{ option.label }}
                </el-radio-button>
              </el-radio-group>
            </el-form-item>
            <template v-if="showCustomRepeatFields">
              <el-form-item label="频率">
                <el-select v-model="itemDraft.simple.frequency" style="width: 100%" @change="onCustomFrequencyChange">
                  <el-option label="每天" value="daily" />
                  <el-option label="每周" value="weekly" />
                  <el-option label="每月" value="monthly" />
                </el-select>
              </el-form-item>
              <el-form-item label="间隔">
                <el-input-number v-model="itemDraft.simple.interval" :min="1" :max="365" style="width: 100%" />
              </el-form-item>
            </template>
            <el-form-item v-if="showWeeklyWeekdays" label="周几" class="dialog-form-span-2">
                <el-checkbox-group v-model="itemDraft.simple.weekdays">
                  <el-checkbox v-for="option in weekdayOptions" :key="option.value" :value="option.value">
                    {{ option.label }}
                  </el-checkbox>
                </el-checkbox-group>
            </el-form-item>
            <el-form-item v-if="showMonthlyDayOfMonth" label="每月几号">
              <el-input-number v-model="itemDraft.simple.dayOfMonth" :min="1" :max="31" style="width: 100%" />
            </el-form-item>
            <template v-if="showCronRepeatFields">
              <el-form-item label="Cron" class="dialog-form-span-2">
                <el-input v-model.trim="itemDraft.cronExpression" placeholder="例如：0 0 9 * * 1-5" />
              </el-form-item>
              <el-form-item label="时区">
                <el-select v-model="itemDraft.timezone" filterable allow-create default-first-option style="width: 100%">
                  <el-option label="本地时区" value="local" />
                  <el-option label="UTC" value="UTC" />
                  <el-option label="Asia/Shanghai" value="Asia/Shanghai" />
                </el-select>
              </el-form-item>
            </template>
            <el-form-item label="结束条件">
              <el-select v-model="itemDraft.endMode" style="width: 100%">
                <el-option label="持续生成" value="never" />
                <el-option label="结束时间" value="until_date" />
                <el-option label="生成次数" value="after_count" />
              </el-select>
            </el-form-item>
            <el-form-item v-if="itemDraft.endMode === 'until_date'" label="结束时间">
              <el-date-picker
                v-model="itemDraft.endValueDate"
                type="datetime"
                value-format="YYYY-MM-DDTHH:mm:ssZ"
                :disabled-minutes="disabledFiveMinuteMinutes"
                :disabled-seconds="disabledAllSeconds"
                style="width: 100%"
              />
            </el-form-item>
            <el-form-item v-else-if="itemDraft.endMode === 'after_count'" label="生成次数">
              <el-input-number v-model="itemDraft.endValueCount" :min="1" :max="9999" style="width: 100%" />
            </el-form-item>
            <el-form-item class="dialog-form-span-2">
              <div class="form-tip">{{ showCronRepeatFields ? "Cron 表达式决定实际触发时间；开始日期只作为首次生效下界。" : "周期事项会从开始日期起按规则生成实例；选择“此后未发生项”时，保存的是周期事项根记录规则。" }}</div>
            </el-form-item>
          </template>

          <el-form-item label="描述" class="dialog-form-span-2">
            <el-input v-model="itemDraft.description" type="textarea" :rows="4" placeholder="可补充事项说明" />
          </el-form-item>

        </div>
      </el-form>
      <template #footer>
        <el-button @click="itemDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="saveItem">{{ itemDialogSubmitText }}</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="typeDialogVisible" :title="typeDialogTitle" width="480px" @closed="resetTypeDraft">
      <el-form label-width="72px">
        <el-form-item label="名称"><el-input v-model.trim="typeDraft.name" placeholder="请输入分类名称" /></el-form-item>
        <el-form-item label="颜色"><el-input v-model.trim="typeDraft.color" placeholder="例如：#409eff" /></el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="typeDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="saveType">保存</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="assigneeDialogVisible" :title="assigneeDialogTitle" width="420px" @closed="resetAssigneeDraft">
      <el-form label-width="72px">
        <el-form-item label="名称"><el-input v-model.trim="assigneeDraft.name" placeholder="请输入执行人名称" /></el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="assigneeDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="saveAssignee">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { TodoAssignee, TodoEditScope, TodoEndMode, TodoItem, TodoItemUpsertPayload, TodoKind, TodoPriority, TodoRecordRole, TodoRecurrence, TodoReminderEvent, TodoReminderPreset, TodoRepeatPreset, TodoRule, TodoRuleMode, TodoSimpleRule, TodoStatus, TodoType } from "../types";
import {
  TODO_REPEAT_PRESET_OPTIONS,
  TODO_WEEKDAY_OPTIONS,
  buildSimpleRuleFromPreset,
  combineLocalDateTime,
  deriveRepeatPreset,
  getTodayDateString,
  isFiveMinuteDateTime,
  isFiveMinuteTime,
  normalizeEndMode,
  splitDateTime,
  summarizeRecurrenceRule,
} from "../utils/todoSchedule";

type SelectTypeValue = number | string | undefined;
type SelectAssigneeValue = number | string;
type ItemDialogMode = "create" | "edit_item" | "edit_root";

interface TodoTypeDraft {
  id: number;
  name: string;
  color: string;
  sortOrder: number;
}

interface TodoAssigneeDraft {
  id: number;
  name: string;
}



const activeTab = ref("items");
const items = ref<TodoItem[]>([]);
const reminders = ref<TodoReminderEvent[]>([]);
const types = ref<TodoType[]>([]);
const assignees = ref<TodoAssignee[]>([]);
const itemKindFilter = ref("all");
const itemStatusFilter = ref("");
const itemKeyword = ref("");
const itemDialogVisible = ref(false);
const itemDialogMode = ref<ItemDialogMode>("create");
const typeDialogVisible = ref(false);
const assigneeDialogVisible = ref(false);
const editingItemSnapshot = ref<TodoItem | null>(null);
const editingRootSnapshot = ref<TodoItem | null>(null);
const defaultReminderPresets: TodoReminderPreset[] = ["0m"];
const lastReminderPresetSelection = ref<TodoReminderPreset[]>([...defaultReminderPresets]);
let reminderUnlisten: UnlistenFn | null = null;

const reminderPresetOptions: Array<{ label: string; value: TodoReminderPreset }> = [
  { label: "准时提醒", value: "0m" },
  { label: "提前五分钟", value: "5m" },
  { label: "提前十分钟", value: "10m" },
  { label: "提前半个小时", value: "30m" },
  { label: "提前一个小时", value: "1h" },
  { label: "提前一天", value: "1d" },
  { label: "提前两天", value: "2d" },
  { label: "不提醒", value: "none" },
];

const reminderPresetToMinutesMap: Record<TodoReminderPreset, number | null> = {
  "0m": 0,
  none: null,
  "5m": 5,
  "10m": 10,
  "30m": 30,
  "1h": 60,
  "1d": 24 * 60,
  "2d": 2 * 24 * 60,
};

const recurrencePresetOptions = TODO_REPEAT_PRESET_OPTIONS.filter((option) => option.value !== "none");
const weekdayOptions = TODO_WEEKDAY_OPTIONS;

const itemDraft = reactive({
  id: 0,
  rootId: 0,
  title: "",
  typeId: undefined as SelectTypeValue,
  priority: "P2" as TodoPriority,
  description: "",
  assigneeIds: [] as SelectAssigneeValue[],
  singleDate: "",
  singleTime: "09:00",
  reminderPresets: [...defaultReminderPresets] as TodoReminderPreset[],
  isRecurring: false,
  scope: "this_instance" as TodoEditScope,
  repeatPreset: "daily" as TodoRepeatPreset,
  recurrenceStartDate: "",
  recurrenceTime: "09:00",
  ruleMode: "simple" as TodoRuleMode,
  timezone: "local",
  cronExpression: "0 0 9 * * 1-5",
  endMode: "never" as TodoEndMode,
  endValueDate: "",
  endValueCount: 1,
  simple: {
    frequency: "daily" as TodoSimpleRule["frequency"],
    interval: 1,
    time: "09:00",
    weekdays: [1, 2, 3, 4, 5] as number[],
    dayOfMonth: 1,
  },
});

const typeDraft = reactive<TodoTypeDraft>({ id: 0, name: "", color: "", sortOrder: 0 });
const assigneeDraft = reactive<TodoAssigneeDraft>({ id: 0, name: "" });

const filteredItems = computed(() => {
  const keyword = itemKeyword.value.trim().toLowerCase();
  return items.value.filter((item) => {
    if (itemKindFilter.value === "actionable" && isRootItem(item)) return false;
    if (itemKindFilter.value === "recurring_root" && !isRootItem(item)) return false;
    if (itemKindFilter.value === "recurring_occurrence" && !isOccurrenceItem(item)) return false;
    if (itemStatusFilter.value && item.status !== itemStatusFilter.value) return false;
    if (!keyword) return true;
    return item.title.toLowerCase().includes(keyword) || item.description.toLowerCase().includes(keyword);
  });
});

const showScopeSelector = computed(() => itemDialogMode.value === "edit_item" && itemDraft.isRecurring);
const showEventTimeField = computed(() => {
  if (itemDialogMode.value === "edit_root") return false;
  if (itemDialogMode.value === "create") return !itemDraft.isRecurring;
  return !itemDraft.isRecurring || itemDraft.scope === "this_instance";
});
const showRecurrenceFields = computed(() => {
  if (itemDialogMode.value === "edit_root") return true;
  if (itemDialogMode.value === "create") return itemDraft.isRecurring;
  return itemDraft.isRecurring && itemDraft.scope === "future_instances";
});
const showWeeklyWeekdays = computed(() => showRecurrenceFields.value && (
  itemDraft.repeatPreset === "weekly"
  || (itemDraft.repeatPreset === "custom" && itemDraft.simple.frequency === "weekly")
));
const showMonthlyDayOfMonth = computed(() => showRecurrenceFields.value && (
  itemDraft.repeatPreset === "monthly"
  || (itemDraft.repeatPreset === "custom" && itemDraft.simple.frequency === "monthly")
));
const showCustomRepeatFields = computed(() => showRecurrenceFields.value && itemDraft.repeatPreset === "custom");
const showCronRepeatFields = computed(() => showRecurrenceFields.value && itemDraft.repeatPreset === "cron");
const itemDialogTitle = computed(() => {
  if (itemDialogMode.value === "create") return "新增事项";
  if (itemDialogMode.value === "edit_root") return "编辑周期事项";
  return itemDraft.scope === "future_instances" ? "编辑后续周期事项" : "编辑事项实例";
});
const itemDialogSubmitText = computed(() => (itemDialogMode.value === "create" ? "创建事项" : "保存"));
const typeDialogTitle = computed(() => (typeDraft.id ? "编辑分类" : "新增分类"));
const assigneeDialogTitle = computed(() => (assigneeDraft.id ? "编辑执行人" : "新增执行人"));

function formatDate(value?: string | null) {
  if (!value) return "-";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function statusLabel(status: TodoStatus | null) {
  if (!status) return "-";
  return { pending: "待办", in_progress: "进行中", completed: "已完成", canceled: "已取消" }[status] || status;
}

function priorityTagType(priority: TodoPriority) {
  return ({ P0: "danger", P1: "warning", P2: "primary", P3: "info" }[priority] || "info") as "danger" | "warning" | "primary" | "info";
}

function itemKindOf(item: TodoItem): TodoKind {
  return item.kind;
}

function itemRoleOf(item: TodoItem): TodoRecordRole {
  return item.recordRole;
}

function isRootItem(item: TodoItem) {
  return itemKindOf(item) === "recurring" && itemRoleOf(item) === "root";
}

function isOccurrenceItem(item: TodoItem) {
  return itemKindOf(item) === "recurring" && itemRoleOf(item) === "occurrence";
}

function recurrenceActive(item: TodoItem) {
  return item.recurrence?.active ?? true;
}

function recurrenceSummary(item: TodoItem) {
  return summarizeRecurrenceRule(item.recurrence, formatDate);
}

function itemKindLabel(item: TodoItem) {
  if (isRootItem(item)) return "周期事项";
  if (isOccurrenceItem(item)) return "周期实例";
  return "单次事项";
}

function itemTagType(item: TodoItem) {
  if (isRootItem(item)) return "warning";
  if (isOccurrenceItem(item)) return "success";
  return "info";
}

function itemStateLabel(item: TodoItem) {
  return isRootItem(item) ? (recurrenceActive(item) ? "启用" : "停用") : (item.status ? statusLabel(item.status) : "-");
}

function itemTimeLabel(item: TodoItem) {
  if (isRootItem(item)) return formatDate(item.recurrence?.nextOccurrenceAt || item.displayAt);
  return formatDate(item.eventAt || item.displayAt);
}

function itemReminderLabel(item: TodoItem) {
  if (isRootItem(item)) return reminderPresetLabels(item.reminderPresets);
  if (item.snoozeUntil) return `已稍后至 ${formatDate(item.snoozeUntil)}`;
  return reminderPresetLabels(item.reminderPresets);
}

function reminderPresetLabel(preset: TodoReminderPreset) {
  return reminderPresetOptions.find((option) => option.value === preset)?.label || "不提醒";
}

function reminderPresetLabels(presets: TodoReminderPreset[]) {
  const effectivePresets = normalizeReminderPresets(presets).filter((preset) => preset !== "none");
  if (effectivePresets.length === 0) return reminderPresetLabel("none");
  return effectivePresets.map((preset) => reminderPresetLabel(preset)).join("、");
}

function normalizeReminderPreset(value: unknown): TodoReminderPreset | null {
  if (typeof value !== "string") return null;
  const normalized = value.trim().toLowerCase();
  if (["0m", "none", "5m", "10m", "30m", "1h", "1d", "2d"].includes(normalized)) {
    return normalized as TodoReminderPreset;
  }
  return null;
}

function sortReminderPresets(presets: TodoReminderPreset[]) {
  const order: TodoReminderPreset[] = ["0m", "5m", "10m", "30m", "1h", "1d", "2d", "none"];
  presets.sort((left, right) => order.indexOf(left) - order.indexOf(right));
}

function normalizeReminderPresets(values: unknown[]) {
  const presets: TodoReminderPreset[] = [];
  let hasNone = false;
  for (const value of values) {
    const normalized = normalizeReminderPreset(value);
    if (!normalized) continue;
    if (normalized === "none") {
      hasNone = true;
      continue;
    }
    if (!presets.includes(normalized)) presets.push(normalized);
  }
  sortReminderPresets(presets);
  if (hasNone && presets.length === 0) return ["none"] as TodoReminderPreset[];
  return presets;
}

function effectiveReminderPresets(values: TodoReminderPreset[]) {
  return normalizeReminderPresets(values).filter((preset) => preset !== "none");
}

function toDraftReminderPresets(values?: TodoReminderPreset[] | null) {
  const normalized = normalizeReminderPresets(values || []);
  return normalized.length > 0 ? normalized : (["none"] as TodoReminderPreset[]);
}

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : {};
}

function readUnknown(record: Record<string, unknown>, keys: string[]) {
  for (const key of keys) {
    if (key in record) return record[key];
  }
  return undefined;
}

function readString(record: Record<string, unknown>, keys: string[], fallback = "") {
  const value = readUnknown(record, keys);
  return typeof value === "string" ? value : fallback;
}

function readNullableString(record: Record<string, unknown>, keys: string[]) {
  const value = readUnknown(record, keys);
  if (typeof value === "string") return value;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  return null;
}

function readNumber(record: Record<string, unknown>, keys: string[], fallback = 0) {
  const value = readUnknown(record, keys);
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
}

function readNullableNumber(record: Record<string, unknown>, keys: string[]) {
  const value = readUnknown(record, keys);
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

function readBoolean(record: Record<string, unknown>, keys: string[], fallback = false) {
  const value = readUnknown(record, keys);
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (["true", "1", "yes", "enabled", "active"].includes(normalized)) return true;
    if (["false", "0", "no", "disabled", "inactive"].includes(normalized)) return false;
  }
  return fallback;
}

function readArray(record: Record<string, unknown>, keys: string[]) {
  const value = readUnknown(record, keys);
  return Array.isArray(value) ? value : [];
}

function getResponseItems(payload: unknown) {
  const record = asRecord(payload);
  const items = readUnknown(record, ["items", "list", "data"]);
  return Array.isArray(items) ? items : [];
}

function normalizePriority(value: string): TodoPriority {
  return ["P0", "P1", "P2", "P3"].includes(value) ? (value as TodoPriority) : "P2";
}

function normalizeStatus(value: string): TodoStatus {
  return ["pending", "in_progress", "completed", "canceled"].includes(value) ? (value as TodoStatus) : "pending";
}

function normalizeKind(value: unknown): TodoKind {
  if (value === "recurring") return "recurring";
  return "one_off";
}

function normalizeRecordRole(value: unknown): TodoRecordRole {
  if (value === "occurrence") return "occurrence";
  return "root";
}

function normalizeRuleMode(value: string): TodoRuleMode {
  return value === "cron" ? "cron" : "simple";
}

function reminderPresetFromMinutes(minutes: number | null): TodoReminderPreset {
  if (minutes == null) return "none";
  const matched = Object.entries(reminderPresetToMinutesMap).find(([, value]) => value === minutes)?.[0];
  return (matched as TodoReminderPreset | undefined) || "none";
}

function reminderPresetToMinutes(preset: TodoReminderPreset) {
  return reminderPresetToMinutesMap[preset] ?? null;
}

function computeLegacyRemindAt(eventAt?: string | null, presets: TodoReminderPreset[] = []) {
  const effectivePresets = effectiveReminderPresets(presets);
  const offsetMinutes = effectivePresets
    .map((preset) => reminderPresetToMinutes(preset))
    .filter((value): value is number => value != null);
  if (!eventAt || offsetMinutes.length === 0) return null;
  const eventDate = new Date(eventAt);
  if (Number.isNaN(eventDate.getTime())) return null;
  return new Date(eventDate.getTime() - Math.max(...offsetMinutes) * 60 * 1000).toISOString();
}

function deriveReminderPresets(record: Record<string, unknown>, eventAt?: string | null): TodoReminderPreset[] {
  const presetValues = readUnknown(record, ["reminderPresets"]);
  if (Array.isArray(presetValues)) {
    return effectiveReminderPresets(presetValues as TodoReminderPreset[]);
  }
  const presetValue = readUnknown(record, ["reminderPreset", "reminderType", "reminder"]);
  if (typeof presetValue === "string") {
    if ((reminderPresetToMinutesMap as Record<string, number | null>)[presetValue] !== undefined) {
      return presetValue === "none" ? [] : [presetValue as TodoReminderPreset];
    }
    const parsed = Number(presetValue);
    if (Number.isFinite(parsed)) {
      const preset = reminderPresetFromMinutes(parsed);
      return preset === "none" ? [] : [preset];
    }
  }
  const offsetMinutes = readNullableNumber(record, ["reminderOffsetMinutes", "reminderMinutes", "offsetMinutes"]);
  if (offsetMinutes != null) {
    const preset = reminderPresetFromMinutes(offsetMinutes);
    return preset === "none" ? [] : [preset];
  }
  const remindAt = readNullableString(record, ["remindAt", "reminderAt"]);
  if (eventAt && remindAt) {
    const eventDate = new Date(eventAt);
    const remindDate = new Date(remindAt);
    if (!Number.isNaN(eventDate.getTime()) && !Number.isNaN(remindDate.getTime())) {
      const preset = reminderPresetFromMinutes(Math.round((eventDate.getTime() - remindDate.getTime()) / 60000));
      return preset === "none" ? [] : [preset];
    }
  }
  return [];
}

function normalizeAssignees(value: unknown): TodoAssignee[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => {
      if (typeof item === "string") {
        return { id: 0, name: item, createdAt: "", updatedAt: "" } satisfies TodoAssignee;
      }
      const record = asRecord(item);
      const name = readString(record, ["name", "label", "assigneeName"]);
      if (!name) return null;
      return {
        id: readNumber(record, ["id", "assigneeId", "userId"], 0),
        name,
        createdAt: readString(record, ["createdAt"], ""),
        updatedAt: readString(record, ["updatedAt"], ""),
      } satisfies TodoAssignee;
    })
    .filter((item): item is TodoAssignee => Boolean(item));
}

function normalizeRule(rawRule: unknown, ruleMode: TodoRuleMode, fallbackCronExpression = ""): TodoRule {
  if (ruleMode === "cron") {
    const expressionRecord = asRecord(rawRule);
    const expression = typeof rawRule === "string"
      ? rawRule
      : readString(expressionRecord, ["expression", "cronExpression"], fallbackCronExpression);
    return { expression };
  }

  const source = typeof rawRule === "string"
    ? (() => {
        try {
          return asRecord(JSON.parse(rawRule));
        } catch {
          return {};
        }
      })()
    : asRecord(rawRule);

  const frequency = (["daily", "weekly", "monthly"] as const).includes(readString(source, ["frequency"], "daily") as "daily" | "weekly" | "monthly")
    ? (readString(source, ["frequency"], "daily") as "daily" | "weekly" | "monthly")
    : "daily";
  const interval = Math.max(1, readNumber(source, ["interval"], 1));
  const time = readString(source, ["time"], "09:00");
  const weekdays = readArray(source, ["weekdays"]).map((item) => Number(item)).filter((day) => Number.isInteger(day) && day >= 1 && day <= 7);
  const dayOfMonth = Math.min(31, Math.max(1, readNumber(source, ["dayOfMonth"], 1)));
  if (frequency === "weekly") return { frequency, interval, time, weekdays: weekdays.length > 0 ? weekdays : [1, 2, 3, 4, 5] };
  if (frequency === "monthly") return { frequency, interval, time, dayOfMonth };
  return { frequency, interval, time };
}

function getRootItemId(item: TodoItem) {
  return item.rootId || item.id;
}

function normalizeTodoItem(raw: unknown): TodoItem {
  const record = asRecord(raw);
  const eventAt = readNullableString(record, ["eventAt", "eventTime", "dueAt"]);
  const kind = normalizeKind(readUnknown(record, ["kind", "seriesKind"]));
  const recordRole = normalizeRecordRole(readUnknown(record, ["recordRole"]));
  const recurrenceSource = asRecord(readUnknown(record, ["recurrence"]));
  const recurrenceRecord = Object.keys(recurrenceSource).length > 0 ? recurrenceSource : record;
  const hasRecurrence = kind === "recurring" && (
    "ruleMode" in recurrenceSource
    || "rule" in recurrenceSource
    || "cronExpression" in recurrenceSource
    || "nextOccurrenceAt" in recurrenceSource
    || recordRole === "root"
  );
  const recurrenceRuleMode = normalizeRuleMode(readString(recurrenceRecord, ["ruleMode"], "simple"));
  const recurrenceCronExpression = readString(recurrenceRecord, ["cronExpression", "cron", "expression"]);
  const recurrence = hasRecurrence
    ? ({
        startAt: readNullableString(recurrenceRecord, ["startAt", "start_at", "firstOccurrenceAt"]),
        ruleMode: recurrenceRuleMode,
        rule: normalizeRule(readUnknown(recurrenceRecord, ["rule", "ruleJson", "schedule"]), recurrenceRuleMode, recurrenceCronExpression),
        cronExpression: recurrenceCronExpression,
        timezone: readString(recurrenceRecord, ["timezone", "tz"], "local"),
        endMode: normalizeEndMode(readString(recurrenceRecord, ["endMode"], "never")),
        endValue: readUnknown(recurrenceRecord, ["endValue", "until", "count"]) as string | number | null,
        nextOccurrenceAt: readNullableString(recurrenceRecord, ["nextOccurrenceAt", "nextEventAt", "nextRunAt"]),
        generatedCount: readNumber(recurrenceRecord, ["generatedCount", "generated"], 0),
        active: readBoolean(recurrenceRecord, ["active", "enabled"], true),
      } satisfies TodoRecurrence)
    : null;
  const rootId = readNullableNumber(record, ["rootId", "seriesId", "templateId", "sourceTemplateId"]) ?? readNumber(record, ["id", "taskId"]);
  const rawStatus = readUnknown(record, ["status"]);
  return {
    id: readNumber(record, ["id", "taskId"]),
    rootId,
    kind,
    recordRole,
    title: readString(record, ["title", "name"]),
    typeId: readNullableNumber(record, ["typeId", "categoryId"]),
    typeName: readNullableString(record, ["typeName", "categoryName"]),
    typeColor: readNullableString(record, ["typeColor", "categoryColor", "color"]),
    priority: normalizePriority(readString(record, ["priority"], "P2")),
    description: readString(record, ["description", "detail"]),
    status: typeof rawStatus === "string" ? normalizeStatus(rawStatus) : null,
    eventAt,
    reminderPresets: deriveReminderPresets(record, eventAt),
    snoozeUntil: readNullableString(record, ["snoozeUntil"]),
    lastNotifiedAt: readNullableString(record, ["lastNotifiedAt"]),
    displayAt: readNullableString(record, ["displayAt", "eventAt", "eventTime", "dueAt", "remindAt", "nextOccurrenceAt"]),
    assignees: normalizeAssignees(readUnknown(record, ["assignees", "owners", "members"])),
    isOverdue: readBoolean(record, ["isOverdue"]),
    recurrence,
    canEditFuture: readBoolean(record, ["canEditFuture"], kind === "recurring" && recordRole === "occurrence"),
    nextTaskReminderId: readNullableNumber(record, ["nextTaskReminderId"]),
    nextReminderPreset: normalizeReminderPreset(readUnknown(record, ["nextReminderPreset"])),
    createdAt: readString(record, ["createdAt"], ""),
    updatedAt: readString(record, ["updatedAt"], ""),
  };
}

function disabledFiveMinuteMinutes(..._args: unknown[]) {
  return Array.from({ length: 60 }, (_, index) => index).filter((minute) => minute % 5 !== 0);
}

function disabledAllSeconds(..._args: unknown[]) {
  return Array.from({ length: 60 }, (_, index) => index);
}

function toDraftAssigneeValues(assigneeList: TodoAssignee[]): SelectAssigneeValue[] {
  return assigneeList
    .map((assignee) => (typeof assignee.id === "number" && assignee.id > 0 ? assignee.id : assignee.name))
    .filter((value): value is SelectAssigneeValue => (typeof value === "number" && value > 0) || (typeof value === "string" && value.trim().length > 0));
}

function normalizeName(value: string) {
  return value.trim().toLocaleLowerCase();
}

function getNextTypeSortOrder() {
  return types.value.reduce((max, item) => Math.max(max, item.sortOrder), 0) + 10;
}

function buildRulePayload(): TodoRule {
  if (itemDraft.repeatPreset === "cron" || itemDraft.ruleMode === "cron") {
    return { expression: itemDraft.cronExpression.trim() };
  }
  return buildSimpleRuleFromPreset({
    preset: itemDraft.repeatPreset,
    startDate: itemDraft.recurrenceStartDate,
    time: itemDraft.recurrenceTime,
    currentRule: {
      frequency: itemDraft.simple.frequency,
      interval: Math.max(1, Number(itemDraft.simple.interval || 1)),
      time: itemDraft.recurrenceTime || itemDraft.simple.time || "09:00",
      weekdays: itemDraft.simple.weekdays,
      dayOfMonth: Math.min(31, Math.max(1, Number(itemDraft.simple.dayOfMonth || 1))),
    },
  });
}

function buildEndValue() {
  if (itemDraft.endMode === "until_date") return itemDraft.endValueDate || null;
  if (itemDraft.endMode === "after_count") return Math.max(1, Number(itemDraft.endValueCount || 1));
  return null;
}

function buildSingleEventAt() {
  return combineLocalDateTime(itemDraft.singleDate, itemDraft.singleTime);
}

function buildRecurrenceStartAt() {
  return combineLocalDateTime(itemDraft.recurrenceStartDate, itemDraft.recurrenceTime);
}

function syncSimpleDraftFromRule(rule: TodoRule) {
  if (!("frequency" in rule)) return;
  itemDraft.simple.frequency = rule.frequency;
  itemDraft.simple.interval = Math.max(1, Number(rule.interval || 1));
  itemDraft.simple.time = rule.time || "09:00";
  itemDraft.recurrenceTime = rule.time || itemDraft.recurrenceTime || "09:00";
  itemDraft.simple.weekdays = Array.isArray(rule.weekdays) && rule.weekdays.length > 0 ? [...rule.weekdays] : [1, 2, 3, 4, 5];
  itemDraft.simple.dayOfMonth = Math.min(31, Math.max(1, Number(rule.dayOfMonth || 1)));
}

function applyRepeatPresetRule(preset: TodoRepeatPreset) {
  itemDraft.repeatPreset = preset;
  if (preset === "cron") {
    itemDraft.ruleMode = "cron";
    if (!itemDraft.recurrenceStartDate) itemDraft.recurrenceStartDate = getTodayDateString();
    return;
  }
  itemDraft.ruleMode = "simple";
  const nextRule = buildSimpleRuleFromPreset({
    preset,
    startDate: itemDraft.recurrenceStartDate,
    time: itemDraft.recurrenceTime,
    currentRule: {
      frequency: itemDraft.simple.frequency,
      interval: itemDraft.simple.interval,
      time: itemDraft.recurrenceTime,
      weekdays: itemDraft.simple.weekdays,
      dayOfMonth: itemDraft.simple.dayOfMonth,
    },
  });
  syncSimpleDraftFromRule(nextRule);
}

function onItemKindChange(nextRecurring: boolean) {
  itemDraft.isRecurring = nextRecurring;
  if (!nextRecurring) {
    if (!itemDraft.singleDate) itemDraft.singleDate = itemDraft.recurrenceStartDate;
    itemDraft.singleTime = itemDraft.recurrenceTime || itemDraft.singleTime || "09:00";
    itemDraft.repeatPreset = "none";
    return;
  }
  if (!itemDraft.recurrenceStartDate) itemDraft.recurrenceStartDate = itemDraft.singleDate || getTodayDateString();
  itemDraft.recurrenceTime = itemDraft.recurrenceTime || itemDraft.singleTime || "09:00";
  if (itemDraft.repeatPreset === "none") {
    applyRepeatPresetRule("daily");
    return;
  }
  applyRepeatPresetRule(itemDraft.repeatPreset);
}

function onRepeatPresetChange(nextPreset: TodoRepeatPreset) {
  if (nextPreset === "none") {
    if (itemDialogMode.value !== "create") {
      ElMessage.warning("现有周期事项不能直接改成不重复，请改用单次事项新建或删除该周期");
      itemDraft.repeatPreset = deriveRepeatPreset(editingRootSnapshot.value?.recurrence || null);
      return;
    }
    onItemKindChange(false);
    return;
  }
  applyRepeatPresetRule(nextPreset);
}

function onCustomFrequencyChange() {
  if (itemDraft.repeatPreset !== "custom") return;
  applyRepeatPresetRule("custom");
}

function onRecurrenceStartDateChange() {
  if (!showRecurrenceFields.value) return;
  if (itemDraft.repeatPreset === "weekly" || itemDraft.repeatPreset === "monthly") {
    applyRepeatPresetRule(itemDraft.repeatPreset);
  }
}

function onRecurrenceTimeChange(value: string) {
  itemDraft.recurrenceTime = value;
  itemDraft.simple.time = value;
}

function onReminderPresetsChange(values: TodoReminderPreset[]) {
  const previousSelection = lastReminderPresetSelection.value;
  const nextHasNone = values.includes("none");
  const previousHasNone = previousSelection.includes("none");

  let normalized = normalizeReminderPresets(values);
  if (nextHasNone && !previousHasNone) {
    normalized = ["none"];
  } else if (nextHasNone && previousHasNone) {
    normalized = normalizeReminderPresets(values.filter((value) => value !== "none"));
  }

  itemDraft.reminderPresets = normalized;
  lastReminderPresetSelection.value = [...normalized];
}

function resetItemDraft() {
  itemDraft.id = 0;
  itemDraft.rootId = 0;
  itemDraft.title = "";
  itemDraft.typeId = undefined;
  itemDraft.priority = "P2";
  itemDraft.description = "";
  itemDraft.assigneeIds = [];
  itemDraft.singleDate = "";
  itemDraft.singleTime = "09:00";
  itemDraft.reminderPresets = [...defaultReminderPresets];
  itemDraft.isRecurring = false;
  itemDraft.scope = "this_instance";
  itemDraft.repeatPreset = "daily";
  itemDraft.recurrenceStartDate = "";
  itemDraft.recurrenceTime = "09:00";
  itemDraft.ruleMode = "simple";
  itemDraft.timezone = "local";
  itemDraft.cronExpression = "0 0 9 * * 1-5";
  itemDraft.endMode = "never";
  itemDraft.endValueDate = "";
  itemDraft.endValueCount = 1;
  itemDraft.simple.frequency = "daily";
  itemDraft.simple.interval = 1;
  itemDraft.simple.time = "09:00";
  itemDraft.simple.weekdays = [1, 2, 3, 4, 5];
  itemDraft.simple.dayOfMonth = 1;
  lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
  editingItemSnapshot.value = null;
  editingRootSnapshot.value = null;
  itemDialogMode.value = "create";
}

function applyItemToDraft(item: TodoItem) {
  const { date, time } = splitDateTime(item.eventAt);
  itemDraft.id = item.id;
  itemDraft.rootId = getRootItemId(item);
  itemDraft.title = item.title;
  itemDraft.typeId = item.typeId ?? undefined;
  itemDraft.priority = item.priority;
  itemDraft.description = item.description;
  itemDraft.assigneeIds = toDraftAssigneeValues(item.assignees);
  itemDraft.singleDate = date;
  itemDraft.singleTime = time;
  itemDraft.reminderPresets = toDraftReminderPresets(item.reminderPresets);
  lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
  itemDraft.isRecurring = itemKindOf(item) === "recurring";
}

function applyRootItemToDraft(item: TodoItem) {

  const recurrence = item.recurrence;
  const { date, time } = splitDateTime(recurrence?.startAt || recurrence?.nextOccurrenceAt || item.displayAt);
  itemDraft.rootId = getRootItemId(item);
  itemDraft.title = item.title;
  itemDraft.typeId = item.typeId ?? undefined;
  itemDraft.priority = item.priority;
  itemDraft.description = item.description;
  itemDraft.assigneeIds = toDraftAssigneeValues(item.assignees);
  itemDraft.reminderPresets = toDraftReminderPresets(item.reminderPresets);
  lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
  itemDraft.isRecurring = true;
  itemDraft.recurrenceStartDate = date || getTodayDateString();
  itemDraft.recurrenceTime = time;
  itemDraft.ruleMode = recurrence?.ruleMode || "simple";
  itemDraft.timezone = recurrence?.timezone || "local";
  itemDraft.cronExpression = (recurrence?.rule as { expression?: string } | undefined)?.expression || recurrence?.cronExpression || "0 0 9 * * 1-5";
  itemDraft.endMode = recurrence?.endMode || "never";
  itemDraft.endValueDate = itemDraft.endMode === "until_date" ? String(recurrence?.endValue || "") : "";
  itemDraft.endValueCount = itemDraft.endMode === "after_count" ? Number(recurrence?.endValue || 1) : 1;
  itemDraft.repeatPreset = deriveRepeatPreset(recurrence);
  if (itemDraft.ruleMode === "simple") syncSimpleDraftFromRule((recurrence?.rule || {}) as TodoRule);
}

async function loadTypes() {
  types.value = ((await invokeToolByChannel("tool:todo:type-list", {})) as { items: TodoType[] }).items || [];
}
async function loadAssignees() {
  assignees.value = ((await invokeToolByChannel("tool:todo:assignee-list", {})) as { items: TodoAssignee[] }).items || [];
}
async function loadItems() {
  items.value = getResponseItems(await invokeToolByChannel("tool:todo:item-list", {})).map(normalizeTodoItem);
}
async function loadReminders() {
  reminders.value = ((await invokeToolByChannel("tool:todo:reminder-list-unread", { limit: 200 })) as { items: TodoReminderEvent[] }).items || [];
}

function openCreateItemDialog(recurring = false) {
  resetItemDraft();
  itemDialogMode.value = "create";
  if (recurring) onItemKindChange(true);
  itemDialogVisible.value = true;
}

async function resolveTypeId(value: SelectTypeValue) {
  if (typeof value === "number") return value;
  const name = typeof value === "string" ? value.trim() : "";
  if (!name) return null;
  const existed = types.value.find((item) => normalizeName(item.name) === normalizeName(name));
  if (existed) return existed.id;
  const result = (await invokeToolByChannel("tool:todo:type-upsert", { name, sortOrder: getNextTypeSortOrder() })) as { id?: number };
  await loadTypes();
  if (typeof result.id !== "number") throw new Error("分类创建失败");
  return result.id;
}

async function resolveAssigneeIds(values: SelectAssigneeValue[]) {
  const ids = new Set<number>();
  let created = false;
  for (const value of values) {
    if (typeof value === "number") {
      ids.add(value);
      continue;
    }
    const name = value.trim();
    if (!name) continue;
    const existed = assignees.value.find((item) => normalizeName(item.name) === normalizeName(name));
    if (existed) {
      ids.add(existed.id);
      continue;
    }
    const result = (await invokeToolByChannel("tool:todo:assignee-upsert", { name })) as { id?: number };
    if (typeof result.id !== "number") throw new Error("执行人创建失败");
    ids.add(result.id);
    created = true;
  }
  if (created) await loadAssignees();
  return [...ids];
}
function getRootItemById(rootId?: number | null) {
  if (!rootId) return null;
  return items.value.find((item) => isRootItem(item) && getRootItemId(item) === rootId) || null;
}

function editItem(item: TodoItem) {
  if (isRootItem(item)) {
    resetItemDraft();
    itemDialogMode.value = "edit_root";
    editingRootSnapshot.value = item;
    itemDraft.scope = "future_instances";
    applyRootItemToDraft(item);
    itemDialogVisible.value = true;
    return;
  }
  resetItemDraft();
  itemDialogMode.value = "edit_item";
  editingItemSnapshot.value = item;
  editingRootSnapshot.value = getRootItemById(getRootItemId(item));
  applyItemToDraft(item);
  itemDraft.scope = "this_instance";
  itemDialogVisible.value = true;
}

function onEditScopeChange(scope: TodoEditScope) {
  itemDraft.scope = scope;
  if (itemDialogMode.value !== "edit_item" || !editingItemSnapshot.value) return;
  if (scope === "this_instance") {
    applyItemToDraft(editingItemSnapshot.value);
    return;
  }
  const targetRoot = editingRootSnapshot.value || getRootItemById(getRootItemId(editingItemSnapshot.value));
  if (!targetRoot) {
    ElMessage.warning("当前事项没有可编辑的周期事项根记录");
    itemDraft.scope = "this_instance";
    applyItemToDraft(editingItemSnapshot.value);
    return;
  }
  applyRootItemToDraft(targetRoot);
  itemDraft.id = editingItemSnapshot.value.id;
  itemDraft.rootId = getRootItemId(targetRoot);
}

async function saveItem() {
  const title = itemDraft.title.trim();
  const singleEventAt = showEventTimeField.value ? buildSingleEventAt() : null;
  const recurrenceStartAt = showRecurrenceFields.value ? buildRecurrenceStartAt() : null;
  const selectedReminderPresets = effectiveReminderPresets(itemDraft.reminderPresets);
  if (!title) {
    ElMessage.warning("请输入事项标题");
    return;
  }
  if (showEventTimeField.value && itemDraft.singleDate && !isFiveMinuteTime(itemDraft.singleTime)) {
    ElMessage.warning("事件时间仅支持5分钟刻度");
    return;
  }
  if (selectedReminderPresets.length > 0 && showEventTimeField.value && !singleEventAt) {
    ElMessage.warning("请先填写日期和时间，再设置提醒方式");
    return;
  }
  if (showRecurrenceFields.value && !itemDraft.recurrenceStartDate) {
    ElMessage.warning("请先选择开始日期");
    return;
  }
  if (showRecurrenceFields.value && !isFiveMinuteTime(itemDraft.recurrenceTime)) {
    ElMessage.warning("周期时间仅支持5分钟刻度");
    return;
  }
  if (showRecurrenceFields.value && !recurrenceStartAt) {
    ElMessage.warning("开始日期或时间格式不正确");
    return;
  }
  if (showCronRepeatFields.value && !itemDraft.cronExpression.trim()) {
    ElMessage.warning("请输入 Cron 表达式");
    return;
  }
  if (showCustomRepeatFields.value && itemDraft.simple.frequency === "weekly" && Number(itemDraft.simple.interval || 1) > 1) {
    ElMessage.warning("按周自定义暂不支持大于 1 的间隔，请改用高级 Cron");
    return;
  }
  if (showRecurrenceFields.value && itemDraft.endMode === "until_date" && itemDraft.endValueDate && !isFiveMinuteDateTime(itemDraft.endValueDate)) {
    ElMessage.warning("结束时间仅支持5分钟刻度");
    return;
  }
  try {
    const typeId = await resolveTypeId(itemDraft.typeId);
    const assigneeIds = await resolveAssigneeIds(itemDraft.assigneeIds);
    const commonPayload = {
      title,
      typeId,
      priority: itemDraft.priority,
      description: itemDraft.description,
      assigneeIds,
      reminderPresets: selectedReminderPresets,
    };

    const payload: TodoItemUpsertPayload & Record<string, unknown> = {
      ...commonPayload,
      kind: itemDraft.isRecurring ? "recurring" : "one_off",
    };
    if (showEventTimeField.value) {
      payload.eventAt = singleEventAt;
      payload.eventTime = payload.eventAt;
      payload.dueAt = payload.eventAt;
      payload.remindAt = computeLegacyRemindAt(singleEventAt, selectedReminderPresets);
    }
    if (showRecurrenceFields.value) {
      payload.recurrence = {
        startAt: recurrenceStartAt,
        ruleMode: itemDraft.ruleMode,
        rule: buildRulePayload(),
        timezone: itemDraft.timezone || "local",
        endMode: itemDraft.endMode,
        endValue: buildEndValue(),
      };
    }
    if (itemDialogMode.value === "create") {
      await invokeToolByChannel("tool:todo:item-create", payload);
    } else {
      payload.id = itemDialogMode.value === "edit_root" ? itemDraft.rootId : itemDraft.id;
      payload.scope = itemDialogMode.value === "edit_root" ? "future_instances" : itemDraft.scope;
      if (itemDraft.rootId) payload.rootId = itemDraft.rootId;
      if (itemDialogMode.value === "edit_root") payload.recordRole = "root";
      await invokeToolByChannel("tool:todo:item-update", payload);
    }

    itemDialogVisible.value = false;
    resetItemDraft();
    await Promise.all([loadItems(), loadReminders()]);
    ElMessage.success("保存成功");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function changeItemStatus(id: number, status: TodoStatus) {
  try {
    await invokeToolByChannel("tool:todo:item-change-status", { id, status });
    await Promise.all([loadItems(), loadReminders()]);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function snoozeItem(id: number, taskReminderId?: number | null) {
  try {
    await invokeToolByChannel("tool:todo:item-snooze", { id, taskReminderId, minutes: 10 });
    await Promise.all([loadItems(), loadReminders()]);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function stopFutureItems(item: TodoItem) {
  try {
    await ElMessageBox.confirm("确认停用该事项的后续周期吗？当前这一次不会被删除。", "停用后续", { type: "warning" });
    await invokeToolByChannel("tool:todo:item-toggle-active", { id: item.id, rootId: getRootItemId(item), active: false });
    await loadItems();
  } catch (error) {
    if ((error as Error).message !== "cancel") ElMessage.error((error as Error).message);
  }
}

async function deleteItem(item: TodoItem) {
  try {
    const message = isRootItem(item)
      ? "确认删除该周期事项吗？已生成的历史实例会保留为独立事项，后续将不再生成。"
      : itemKindOf(item) === "recurring"
        ? "确认删除当前这一次事项吗？如需停止后续，请使用“停后续”。"
        : "确认删除该事项吗？";
    await ElMessageBox.confirm(message, "删除确认", { type: "warning" });
    await invokeToolByChannel("tool:todo:item-delete", isRootItem(item)
      ? { id: item.id, rootId: item.rootId || item.id, recordRole: "root" }
      : { id: item.id });
    await Promise.all([loadItems(), loadReminders()]);
  } catch (error) {
    if ((error as Error).message !== "cancel") ElMessage.error((error as Error).message);
  }
}

async function toggleRootActive(item: TodoItem) {
  try {
    await invokeToolByChannel("tool:todo:item-toggle-active", { id: item.id, rootId: getRootItemId(item), recordRole: "root", active: !recurrenceActive(item) });
    await loadItems();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function markRead(id: number) {
  try {
    await invokeToolByChannel("tool:todo:reminder-mark-read", { id });
    await loadReminders();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function markAllRead() {
  try {
    await invokeToolByChannel("tool:todo:reminder-mark-read", { all: true });
    await loadReminders();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function completeFromReminder(item: TodoReminderEvent) {
  await changeItemStatus(item.taskId, "completed");
  await markRead(item.id);
}

async function snoozeFromReminder(item: TodoReminderEvent) {
  await snoozeItem(item.taskId, item.taskReminderId);
  await markRead(item.id);
}

function resetTypeDraft() {
  typeDraft.id = 0;
  typeDraft.name = "";
  typeDraft.color = "";
  typeDraft.sortOrder = getNextTypeSortOrder();
}
function addType() {
  resetTypeDraft();
  typeDialogVisible.value = true;
}
function renameType(item: TodoType) {
  typeDraft.id = item.id;
  typeDraft.name = item.name;
  typeDraft.color = item.color;
  typeDraft.sortOrder = item.sortOrder;
  typeDialogVisible.value = true;
}
async function saveType() {
  const name = typeDraft.name.trim();
  if (!name) {
    ElMessage.warning("请输入分类名称");
    return;
  }
  try {
    await invokeToolByChannel("tool:todo:type-upsert", typeDraft.id ? { id: typeDraft.id, name, color: typeDraft.color, sortOrder: typeDraft.sortOrder } : { name, color: typeDraft.color, sortOrder: typeDraft.sortOrder || getNextTypeSortOrder() });
    typeDialogVisible.value = false;
    resetTypeDraft();
    await Promise.all([loadTypes(), loadItems()]);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
async function removeType(item: TodoType) {
  try {
    await ElMessageBox.confirm(`确认删除分类「${item.name}」吗？`, "删除确认", { type: "warning" });
    await invokeToolByChannel("tool:todo:type-delete", { id: item.id });
    await Promise.all([loadTypes(), loadItems()]);
  } catch (error) {
    if ((error as Error).message !== "cancel") ElMessage.error((error as Error).message);
  }
}

function resetAssigneeDraft() {
  assigneeDraft.id = 0;
  assigneeDraft.name = "";
}
function addAssignee() {
  resetAssigneeDraft();
  assigneeDialogVisible.value = true;
}
function renameAssignee(item: TodoAssignee) {
  assigneeDraft.id = item.id;
  assigneeDraft.name = item.name;
  assigneeDialogVisible.value = true;
}
async function saveAssignee() {
  const name = assigneeDraft.name.trim();
  if (!name) {
    ElMessage.warning("请输入执行人名称");
    return;
  }
  try {
    await invokeToolByChannel("tool:todo:assignee-upsert", assigneeDraft.id ? { id: assigneeDraft.id, name } : { name });
    assigneeDialogVisible.value = false;
    resetAssigneeDraft();
    await Promise.all([loadAssignees(), loadItems()]);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
async function removeAssignee(item: TodoAssignee) {
  try {
    await ElMessageBox.confirm(`确认删除执行人「${item.name}」吗？`, "删除确认", { type: "warning" });
    await invokeToolByChannel("tool:todo:assignee-delete", { id: item.id });
    await Promise.all([loadAssignees(), loadItems()]);
  } catch (error) {
    if ((error as Error).message !== "cancel") ElMessage.error((error as Error).message);
  }
}

onMounted(async () => {
  await Promise.all([loadTypes(), loadAssignees(), loadItems(), loadReminders()]);
  try {
    reminderUnlisten = await listen("todo-reminder-fired", async () => {
      await Promise.all([loadItems(), loadReminders()]);
    });
  } catch {
    reminderUnlisten = null;
  }
});

onBeforeUnmount(() => {
  reminderUnlisten?.();
  reminderUnlisten = null;
});
</script>

<style scoped>
.todo-panel { display: flex; flex-direction: column; gap: 12px; }
.toolbar { display: flex; gap: 10px; align-items: center; margin-bottom: 12px; flex-wrap: wrap; }
.toolbar-inline-end { margin-top: 12px; justify-content: flex-end; }
.title-cell { display: flex; align-items: center; gap: 8px; }
.basic-grid { display: grid; grid-template-columns: repeat(2, minmax(280px, 1fr)); gap: 12px; }
.card-header { display: flex; justify-content: space-between; align-items: center; }
.dialog-form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0 12px; }
.dialog-form-span-2 { grid-column: 1 / -1; }
.dialog-form :deep(.el-form-item__content) { min-width: 0; }
.repeat-radio-group { display: flex; flex-wrap: wrap; gap: 8px; }
.repeat-radio-group :deep(.el-radio-button__inner) { min-width: 88px; }
.form-tip { color: var(--el-text-color-secondary); font-size: 13px; line-height: 1.6; }
.color-dot { display: inline-block; width: 10px; height: 10px; margin-right: 6px; border-radius: 50%; vertical-align: middle; }
.edit-action-btn { color: #1d4ed8 !important; font-weight: 600; }
.edit-action-btn:hover { color: #1e40af !important; }
@media (max-width: 960px) {
  .basic-grid, .dialog-form-grid { grid-template-columns: 1fr; }
  .dialog-form-span-2 { grid-column: auto; }
}
</style>
