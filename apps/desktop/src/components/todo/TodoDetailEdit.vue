<template>
  <div class="detail-edit">
    <div class="detail-pane-header">
      <div class="detail-title-group">
        <div class="detail-eyebrow">事项编辑</div>
        <h3 class="detail-title">{{ dialogTitle }}</h3>
        <div class="detail-subtitle">
          {{ mode === "create" ? "在右栏直接填写并保存" : "修改后会保留当前选中项" }}
        </div>
      </div>
    </div>
    <div ref="scrollRef" class="detail-scroll detail-scroll--form">
      <el-form label-position="top" class="todo-item-form">
        <div class="todo-form-section">
          <el-form-item label="标题">
            <el-input
              ref="titleInputRef"
              v-model.trim="draft.title"
              placeholder="请输入事项标题"
              @keydown.enter.exact.prevent="$emit('titleEnter', $event)"
            />
          </el-form-item>
        </div>

        <!-- 关联项目/工作项（始终可见） -->
        <div class="todo-form-section">
          <el-form-item label="关联">
            <div v-if="draft.kind === 'recurring'" class="pm-link-recurring-hint">
              重复事项暂不支持关联项目工作项
            </div>
            <InlinePmSelector
              v-else
              :project-id="draft.projectId"
              :project-name="draft.projectId ? projectOptions.find(p => p.id === draft.projectId)?.name ?? null : null"
              :project-color="draft.projectId ? projectOptions.find(p => p.id === draft.projectId)?.color ?? null : null"
              :pm-item-id="draft.pmItemId"
              :pm-item-title="draft.pmItemTitle"
              :pm-item-status="draft.pmItemStatus"
              :candidates="pmCandidates"
              :candidates-loading="false"
              :project-list="projectOptions"
              @link="(id: number) => $emit('pmSelectChange', id)"
              @unlink="$emit('pmSelectChange', null)"
              @create-pm="(title: string, projectId: number) => $emit('pmCreate', title, projectId)"
              @search="(keyword: string) => $emit('pmSearch', keyword)"
              @change-project="(projectId: number) => $emit('pmProjectChange', projectId)"
              @clear-all="$emit('pmProjectChange', null)"
            />
          </el-form-item>
        </div>

        <div class="todo-form-more-toggle" @click="$emit('toggleMoreFields')">
          <span class="more-toggle-text">
            {{ showMoreFields ? "收起设置" : "更多设置" }}
          </span>
          <span v-if="!showMoreFields && moreFieldsSummary" class="more-toggle-summary">
            {{ moreFieldsSummary }}
          </span>
          <span class="more-toggle-icon" :class="{ 'is-expanded': showMoreFields }">
            <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
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

        <div v-show="showMoreFields" class="todo-form-collapsible">
          <div class="todo-form-section">
            <el-form-item label="分类与优先级" class="todo-form-item-category-priority">
              <div class="category-priority-row">
                <el-select
                  v-model="draft.typeId"
                  clearable
                  filterable
                  allow-create
                  default-first-option
                  placeholder="可输入新分类"
                  style="flex: 1"
                >
                  <el-option
                    v-for="item in sortedTypes"
                    :key="item.id"
                    :label="item.name"
                    :value="item.id"
                  />
                </el-select>
                <el-select v-model="draft.priority" style="width: 150px">
                  <template #prefix>
                    <span
                      class="priority-dot"
                      :class="'priority-' + draft.priority.toLowerCase()"
                    />
                  </template>
                  <el-option
                    v-for="opt in priorityOptions"
                    :key="opt.value"
                    :label="opt.label"
                    :value="opt.value"
                  >
                    <span
                      class="priority-dot"
                      :class="'priority-' + opt.value.toLowerCase()"
                    />
                    {{ opt.label }}
                  </el-option>
                </el-select>
              </div>
            </el-form-item>
            <el-form-item label="执行人">
              <el-select
                v-model="draft.assigneeIds"
                multiple
                clearable
                filterable
                allow-create
                default-first-option
                :reserve-keyword="false"
                placeholder="可输入新执行人"
                style="width: 100%"
              >
                <el-option
                  v-for="item in assignees"
                  :key="item.id"
                  :label="item.name"
                  :value="item.id"
                />
              </el-select>
            </el-form-item>
          </div>

          <div ref="scheduleRef" class="todo-form-section">
            <el-form-item label="日期与时间" class="todo-form-item-datetime">
              <div class="datetime-row">
                <el-date-picker
                  :model-value="draft.eventDate || undefined"
                  type="date"
                  value-format="YYYY-MM-DD"
                  clearable
                  style="width: 160px"
                  @update:model-value="$emit('eventDateChange', $event)"
                />
                <div class="time-picker-fused">
                  <el-select :model-value="timeHour" class="time-fused-select" placeholder="时" @update:model-value="$emit('eventHourChange', $event)">
                    <el-option
                      v-for="option in hourOptions"
                      :key="option.value"
                      :label="option.label"
                      :value="option.value"
                    />
                  </el-select>
                  <span class="time-fused-separator">:</span>
                  <el-select
                    :model-value="timeMinute"
                    class="time-fused-select"
                    placeholder="分"
                    @update:model-value="$emit('eventMinuteChange', $event)"
                  >
                    <el-option
                      v-for="option in minuteOptions"
                      :key="option.value"
                      :label="option.label"
                      :value="option.value"
                    />
                  </el-select>
                </div>
              </div>
              <div class="datetime-actions">
                <div class="date-quick-presets">
                  <el-button
                    text
                    size="small"
                    class="date-preset-btn"
                    @click="$emit('fillQuickDate', 0)"
                  >今天</el-button>
                  <el-button
                    text
                    size="small"
                    class="date-preset-btn"
                    @click="$emit('fillQuickDate', 1)"
                  >明天</el-button>
                  <el-button
                    text
                    size="small"
                    class="date-preset-btn"
                    @click="$emit('fillQuickDate', 2)"
                  >后天</el-button>
                </div>
                <el-button
                  v-if="!draft.eventDate || !draft.eventTime"
                  text
                  size="small"
                  class="time-fused-clear"
                  @click="$emit('fillDefaultDateTime')"
                >填充</el-button>
                <el-button
                  v-else
                  text
                  size="small"
                  class="time-fused-clear"
                  @click="$emit('clearEventSchedule')"
                >清空</el-button>
              </div>
            </el-form-item>
            <el-form-item label="提醒">
              <el-select
                v-model="draft.reminderPresets"
                multiple
                clearable
                style="width: 100%"
                @change="$emit('reminderPresetsChange', $event)"
              >
                <el-option
                  v-for="option in reminderPresetOptions"
                  :key="option.value"
                  :label="option.label"
                  :value="option.value"
                />
              </el-select>
            </el-form-item>
          </div>

          <div class="todo-form-section">
            <el-form-item label="重复方式">
              <el-radio-group
                v-model="draft.repeatPreset"
                class="repeat-radio-group"
                @change="$emit('repeatPresetChange', $event)"
              >
                <el-radio-button
                  v-for="option in repeatPresetOptions"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ option.label }}
                </el-radio-button>
              </el-radio-group>
            </el-form-item>
            <template v-if="isRepeating">
              <div class="repeat-detail-card">
                <template v-if="showCustomRepeatFields">
                  <div class="todo-form-row">
                    <el-form-item label="频率" class="todo-form-item-flex">
                      <el-select
                        v-model="draft.simple.frequency"
                        style="width: 100%"
                        @change="$emit('customFrequencyChange')"
                      >
                        <el-option label="每天" value="daily" />
                        <el-option label="每周" value="weekly" />
                        <el-option label="每月" value="monthly" />
                      </el-select>
                    </el-form-item>
                    <el-form-item label="间隔" class="todo-form-item-flex">
                      <el-input-number
                        v-model="draft.simple.interval"
                        :min="1"
                        :max="365"
                        style="width: 100%"
                      />
                    </el-form-item>
                  </div>
                </template>
                <el-form-item v-if="showWeeklyWeekdays" label="周几">
                  <el-checkbox-group v-model="draft.simple.weekdays">
                    <el-checkbox
                      v-for="option in weekdayOptions"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </el-checkbox>
                  </el-checkbox-group>
                </el-form-item>
                <el-form-item v-if="showMonthlyDayOfMonth" label="每月几号">
                  <el-input-number
                    v-model="draft.simple.dayOfMonth"
                    :min="1"
                    :max="31"
                    style="width: 100%"
                  />
                </el-form-item>
                <template v-if="showCronRepeatFields">
                  <el-form-item label="Cron 表达式">
                    <el-input
                      v-model.trim="draft.cronExpression"
                      placeholder="例如：0 0 9 * * Mon-Fri"
                    />
                  </el-form-item>
                  <el-form-item label="时区">
                    <el-select
                      v-model="draft.timezone"
                      filterable
                      allow-create
                      default-first-option
                      style="width: 100%"
                    >
                      <el-option label="本地时区" value="local" />
                      <el-option label="UTC" value="UTC" />
                      <el-option label="Asia/Shanghai" value="Asia/Shanghai" />
                    </el-select>
                  </el-form-item>
                </template>
                <div class="todo-form-row">
                  <el-form-item label="结束条件" class="todo-form-item-flex">
                    <el-select v-model="draft.endMode" style="width: 100%">
                      <el-option label="持续生成" value="never" />
                      <el-option label="结束时间" value="until_date" />
                      <el-option label="生成次数" value="after_count" />
                    </el-select>
                  </el-form-item>
                  <el-form-item
                    v-if="draft.endMode === 'until_date'"
                    label="结束时间"
                    class="todo-form-item-flex"
                  >
                    <el-date-picker
                      v-model="draft.endValueDate"
                      type="datetime"
                      value-format="YYYY-MM-DDTHH:mm:ssZ"
                      :disabled-minutes="disabledFiveMinuteMinutes"
                      :disabled-seconds="disabledAllSeconds"
                      style="width: 100%"
                    />
                  </el-form-item>
                  <el-form-item
                    v-else-if="draft.endMode === 'after_count'"
                    label="生成次数"
                    class="todo-form-item-flex"
                  >
                    <el-input-number
                      v-model="draft.endValueCount"
                      :min="1"
                      :max="9999"
                      style="width: 100%"
                    />
                  </el-form-item>
                </div>
              </div>
              <div class="repeat-tip">{{ repeatFormTip }}</div>
            </template>
          </div>
        </div>

        <div class="todo-form-section">
          <el-form-item label="描述">
            <RichDescriptionEditor
              ref="editorRef"
              v-model="draft.description"
              owner-type="todo"
              :owner-id="selectedItem?.id ?? null"
              placeholder="Todo 描述（支持粘贴图片）"
            />
          </el-form-item>
        </div>
        <div class="todo-form-section">
          <el-form-item label="关联链接">
            <div class="link-edit-list">
              <div v-for="(link, i) in draft.links" :key="i" class="link-edit-row">
                <el-input v-model="link.url" placeholder="URL 或文件路径" size="small" />
                <el-input
                  v-model="link.title"
                  placeholder="标题（可选）"
                  size="small"
                  style="width: 150px; flex-shrink: 0"
                />
                <el-button
                  text
                  size="small"
                  type="danger"
                  @click="draft.links.splice(i, 1)"
                >删除</el-button>
              </div>
            </div>
            <el-button
              text
              type="primary"
              size="small"
              @click="draft.links.push({ url: '', title: '' })"
            >
              + 添加链接
            </el-button>
          </el-form-item>
        </div>
      </el-form>
    </div>
    <div class="detail-pane-footer">
      <div v-if="mode === 'edit' && selectedItem" class="detail-footer-actions">
        <el-button
          v-if="canPinItem(selectedItem)"
          size="small"
          link
          @click="$emit('togglePin', selectedItem.id)"
        >
          {{ selectedItem.pinned ? "取消置顶" : "置顶" }}
        </el-button>
        <el-button
          size="small"
          link
          type="success"
          @click="$emit('changeStatus', selectedItem.id, isDoneItem(selectedItem) ? 'pending' : 'completed')"
        >
          {{ isDoneItem(selectedItem) ? "恢复" : "完成" }}
        </el-button>
        <el-button size="small" link type="danger" @click="$emit('delete', selectedItem)">
          删除
        </el-button>
      </div>
      <div class="detail-footer-submit">
        <el-button @click="$emit('cancel')">取消</el-button>
        <el-button type="primary" @click="$emit('save')">{{ submitText }}</el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Plus } from "@element-plus/icons-vue";
import type { TodoAssignee, TodoItem, TodoPriority, TodoReminderPreset, TodoRepeatPreset, TodoSimpleRule } from "../../types";
import type { PmCandidateItem } from "../../types/pm";
import { effectiveReminderPresets } from "../../composables/useTodoItem";
import InlinePmSelector from "../InlinePmSelector.vue";
import RichDescriptionEditor from "../RichDescriptionEditor.vue";
import {
  useRichDescriptionLifecycle,
  type RichEditorExposed,
} from "../../composables/useRichDescriptionLifecycle";

interface DraftShape {
  title: string;
  description: string;
  projectId: number | null;
  pmItemId: number | null;
  pmItemTitle: string | null;
  pmItemProjectId: number | null;
  pmItemStatus: string | null;
  typeId: number | string | null;
  priority: TodoPriority;
  assigneeIds: (number | string)[];
  eventDate: string | null;
  eventTime: string | null;
  reminderPresets: TodoReminderPreset[];
  repeatPreset: TodoRepeatPreset;
  kind: string;
  simple: { frequency: string; interval: number; weekdays: number[]; dayOfMonth: number };
  cronExpression: string;
  timezone: string;
  endMode: string;
  endValueDate: string | null;
  endValueCount: number;
  links: { url: string; title: string }[];
}

const props = defineProps<{
  mode: "create" | "edit";
  draft: DraftShape;
  selectedItem: TodoItem | null;
  showMoreFields: boolean;
  pmLinkItemId: number | null;
  sortedTypes: { id: number; name: string }[];
  assignees: TodoAssignee[];
  projectOptions: { id: number; name: string; color: string }[];
  pmCandidates: PmCandidateItem[];
  priorityOptions: { value: TodoPriority; label: string }[];
  reminderPresetOptions: { label: string; value: TodoReminderPreset }[];
  repeatPresetOptions: { value: TodoRepeatPreset; label: string }[];
  weekdayOptions: { value: number; label: string }[];
  hourOptions: { value: string; label: string }[];
  minuteOptions: { value: string; label: string }[];
  timeHour: string;
  timeMinute: string;
}>();

defineEmits<{
  titleEnter: [event: KeyboardEvent];
  toggleMoreFields: [];
  pmSelectChange: [value: number | null];
  pmProjectChange: [value: number | null];
  pmCreate: [title: string, projectId: number];
  pmSearch: [keyword: string];
  navigateToPm: [pmItemId: number, pmProjectId: number | null];
  eventDateChange: [value: string | null | undefined];
  eventHourChange: [value: string];
  eventMinuteChange: [value: string];
  fillQuickDate: [daysOffset: number];
  fillDefaultDateTime: [];
  clearEventSchedule: [];
  reminderPresetsChange: [values: TodoReminderPreset[]];
  repeatPresetChange: [preset: TodoRepeatPreset];
  customFrequencyChange: [];
  togglePin: [id: number];
  changeStatus: [id: number, status: string];
  delete: [item: TodoItem];
  cancel: [];
  save: [];
}>();

const titleInputRef = ref<{ focus: () => void } | null>(null);
const editorRef = ref<RichEditorExposed | null>(null);
const scrollRef = ref<HTMLElement | null>(null);
const scheduleRef = ref<HTMLElement | null>(null);
const submittedThisRound = ref(false);

function focusTitleInput() {
  titleInputRef.value?.focus();
}

function focusScheduleInput() {
  const scheduleSection = scheduleRef.value;
  if (!scheduleSection) return;
  const formScroll = scrollRef.value;
  if (formScroll) {
    const scrollHostRect = formScroll.getBoundingClientRect();
    const sectionRect = scheduleSection.getBoundingClientRect();
    const nextScrollTop = formScroll.scrollTop + sectionRect.top - scrollHostRect.top - 16;
    formScroll.scrollTo({ top: Math.max(0, nextScrollTop), behavior: "smooth" });
  } else {
    scheduleSection.scrollIntoView({ block: "start", behavior: "smooth" });
  }
  const firstInput = scheduleSection.querySelector(
    "input:not([disabled])",
  ) as HTMLInputElement | null;
  firstInput?.focus();
}

const richLifecycle = useRichDescriptionLifecycle({
  ownerType: "todo",
  editorRef,
  getRealId: () => props.selectedItem?.id ?? null,
});

// 切换编辑对象时：重置 submit 标记，并让 Editor 生成新 tempId / 同步 description
watch(
  () => [props.mode, props.selectedItem?.id],
  () => {
    submittedThisRound.value = false;
    queueMicrotask(() => {
      (editorRef.value as any)?.reset?.(props.draft.description ?? "");
    });
  },
  { immediate: true }
);

defineExpose({
  focusTitleInput,
  focusScheduleInput,
  /** 新建 Todo 成功后：把 tmp-<uuid> 下附件 rebind 到 realId */
  async runAfterSubmit(realId: number) {
    submittedThisRound.value = true;
    await richLifecycle.afterSubmit(realId);
  },
  /** 编辑 Todo 保存前：按当前 doc attIds 清理被删图的残留附件 */
  async runBeforeSubmit() {
    submittedThisRound.value = true;
    await richLifecycle.beforeCloseEdit();
  },
  /** 取消/离开：仅对未提交的新建场景做 tmp 清理 */
  async runOnCancel() {
    if (submittedThisRound.value) return;
    try {
      await richLifecycle.onCancel();
    } catch (e) {
      console.warn("TodoDetailEdit cleanup cancel failed:", e);
    }
  },
});

// --- Computed ---

const isRepeating = computed(() => props.draft.repeatPreset !== "none");

const showCustomRepeatFields = computed(
  () => isRepeating.value && props.draft.repeatPreset === "custom",
);

const showWeeklyWeekdays = computed(
  () =>
    isRepeating.value &&
    (props.draft.repeatPreset === "weekly" ||
      (props.draft.repeatPreset === "custom" && props.draft.simple.frequency === "weekly")),
);

const showMonthlyDayOfMonth = computed(
  () =>
    isRepeating.value &&
    (props.draft.repeatPreset === "monthly" ||
      (props.draft.repeatPreset === "custom" && props.draft.simple.frequency === "monthly")),
);

const showCronRepeatFields = computed(
  () => isRepeating.value && props.draft.repeatPreset === "cron",
);

const repeatFormTip = computed(() => {
  if (showCronRepeatFields.value)
    return "Cron 表达式决定实际触发时间；日期只作为首次生效下界。";
    return "重复事项会从日期起按规则生成实例；选择\u201C此后未发生项\u201D时，保存的是重复规则。";
});

const dialogTitle = computed(() => (props.mode === "create" ? "新增事项" : "编辑事项"));

const submitText = computed(() => (props.mode === "create" ? "创建事项" : "保存"));

const moreFieldsSummary = computed(() => {
  const parts: string[] = [];
  if (props.draft.typeId) parts.push("分类");
  if (props.draft.priority !== "P2") parts.push("优先级");
  if (props.draft.assigneeIds.length > 0) parts.push("执行人");
  if (props.draft.eventDate || props.draft.eventTime) parts.push("日期");
  if (effectiveReminderPresets(props.draft.reminderPresets).length > 0) parts.push("提醒");
  if (props.draft.repeatPreset !== "none") parts.push("重复");
  return parts.length > 0 ? `已设${parts.join("、")}` : "";
});

// --- Helpers ---

function isDoneItem(item: TodoItem) {
  return item.status === "completed";
}

function canPinItem(item: TodoItem) {
  return item.status !== "completed";
}

function disabledFiveMinuteMinutes(..._args: unknown[]) {
  return Array.from({ length: 60 }, (_, index) => index).filter((minute) => minute % 5 !== 0);
}

function disabledAllSeconds(..._args: unknown[]) {
  return Array.from({ length: 60 }, (_, index) => index);
}
</script>

<style scoped>
.detail-edit {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
}
.detail-pane-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--lc-border);
  background: linear-gradient(180deg, var(--lc-surface-0), var(--lc-surface-1));
  flex-shrink: 0;
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
.detail-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 16px 20px;
}
.detail-scroll--form {
  padding-bottom: 12px;
}
.detail-pane-footer {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 20px;
  border-top: 1px solid var(--lc-border);
  background: var(--lc-surface-0);
}
.detail-footer-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.detail-footer-submit {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  margin-left: auto;
}
.todo-form-row {
  flex-direction: column;
  gap: 0;
}
.todo-form-item-datetime,
.todo-form-item-category-priority,
.todo-form-item-flex {
  width: 100%;
}
.datetime-row {
  flex-wrap: wrap;
}
.todo-form-section {
  margin-bottom: 16px;
}
.todo-form-section:last-child {
  margin-bottom: 0;
}
.todo-form-more-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 0;
  margin-bottom: 8px;
  cursor: pointer;
  user-select: none;
  color: var(--el-color-primary);
  font-size: 13px;
  border-top: 1px solid var(--lc-border-subtle);
  border-bottom: 1px solid var(--lc-border-subtle);
  border-radius: 0;
  transition:
    background 0.15s ease,
    border-radius 0.15s ease;
}
.todo-form-more-toggle:hover {
  background: var(--lc-surface-1);
  border-radius: 6px;
}
.more-toggle-text {
  font-weight: 500;
}
.more-toggle-summary {
  color: var(--lc-text-muted);
  font-size: 12px;
}
.more-toggle-icon {
  display: inline-flex;
  transition: transform 0.2s;
}
.more-toggle-icon.is-expanded {
  transform: rotate(180deg);
}
.todo-form-collapsible {
  /* container for v-show toggled fields */
}
.todo-item-form :deep(.el-form-item__label) {
  font-size: 13px;
  font-weight: 500;
  color: var(--lc-text);
}
.todo-form-row {
  display: flex;
  gap: 16px;
}
.todo-form-row .el-form-item {
  margin-bottom: 14px;
}
.todo-form-item-flex {
  flex: 1;
}
.todo-form-item-datetime {
  width: 100%;
}
.todo-form-item-category-priority {
  width: 100%;
}
.category-priority-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}
.priority-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.priority-p0 { background-color: var(--lc-danger); }
.priority-p1 { background-color: var(--lc-warning); }
.priority-p2 { background-color: var(--lc-accent); }
.priority-p3 { background-color: var(--lc-text-muted); }
.datetime-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}
.datetime-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  margin-top: 4px;
}
.time-picker-fused {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  border: 1px solid var(--lc-border);
  border-radius: var(--el-border-radius-base);
  overflow: hidden;
  transition: border-color 0.2s;
}
.time-picker-fused:hover {
  border-color: var(--lc-border-hover);
}
.time-picker-fused:focus-within {
  border-color: var(--lc-accent);
}
.time-picker-fused .time-fused-select :deep(.el-input__wrapper) {
  box-shadow: none !important;
  border-radius: 0;
}
.time-fused-separator {
  color: var(--lc-text-muted);
  font-weight: 600;
  padding: 0 2px;
  user-select: none;
}
.time-fused-clear {
  flex-shrink: 0;
}
.repeat-detail-card {
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--el-border-radius-base);
  padding: 12px;
  margin-bottom: 12px;
}
.repeat-tip {
  color: var(--lc-text-muted);
  font-size: 13px;
  line-height: 1.6;
  margin-top: 4px;
}
.repeat-radio-group {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.repeat-radio-group :deep(.el-radio-button__inner) {
  min-width: 76px;
}
  .detail-pane-footer {
    padding-left: 14px;
    padding-right: 14px;
  }
.date-quick-presets {
  display: flex;
  gap: 4px;
}
.date-preset-btn {
  font-size: 12px;
  padding: 2px 8px;
  height: auto;
  color: var(--lc-text-muted);
}
.date-preset-btn:hover {
  color: var(--lc-accent);
}
.link-edit-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
  margin-bottom: 6px;
}
.link-edit-row {
  display: flex;
  gap: 6px;
  align-items: center;
}

/* --- Custom scrollbar --- */
.detail-scroll::-webkit-scrollbar {
  width: 4px;
}
.detail-scroll::-webkit-scrollbar-thumb {
  background: var(--lc-border);
  border-radius: 2px;
}
.detail-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--lc-border-hover);
}
.detail-scroll::-webkit-scrollbar-track {
  background: transparent;
}

/* --- Responsive --- */
@media (max-width: 640px) {
  .detail-pane-header,
  .detail-scroll {
    padding: 14px;
  }
}
</style>
