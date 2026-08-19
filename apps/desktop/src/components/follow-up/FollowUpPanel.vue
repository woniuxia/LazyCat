<template>
  <div class="follow-up-panel" v-loading="loading">
    <aside class="follow-up-sidebar">
      <slot name="view-switch" />
      <button
        v-for="section in sections"
        :key="section.key"
        class="section-button"
        :class="{ active: activeGroup === section.key }"
        @click="activeGroup = section.key"
      >
        <span>{{ section.label }}</span
        ><span class="section-count">{{ sectionCounts[section.key] }}</span>
      </button>
      <div class="sidebar-filter-title">筛选</div>
      <el-select v-model="filters.personId" clearable placeholder="全部责任人" size="small">
        <el-option
          v-for="person in assignees"
          :key="person.id"
          :label="person.name"
          :value="person.id"
        />
      </el-select>
      <el-select v-model="filters.priority" clearable placeholder="全部优先级" size="small">
        <el-option
          v-for="priority in priorities"
          :key="priority"
          :label="priority"
          :value="priority"
        />
      </el-select>
      <el-select
        v-model="filters.attentionStatus"
        clearable
        placeholder="全部关注状态"
        size="small"
      >
        <el-option label="关注中" value="active" /><el-option label="已结束" value="ended" />
      </el-select>
    </aside>

    <section class="follow-up-list-pane">
      <header class="follow-up-toolbar">
        <el-input v-model="filters.keyword" clearable placeholder="搜索标题、责任人、描述或进展" />
        <el-button :icon="Refresh" title="刷新" @click="loadItems" />
        <el-button type="primary" :icon="Plus" @click="startCreate">新增关注事项</el-button>
      </header>
      <div class="follow-up-scroll">
        <div class="group-heading">
          <span>{{ activeGroupLabel }}</span
          ><span>{{ visibleItems.length }}</span>
        </div>
        <el-empty v-if="!visibleItems.length" :description="emptyDescription" :image-size="72" />
        <button
          v-for="item in visibleItems"
          :key="item.id"
          class="follow-up-card"
          :class="{ selected: selectedId === item.id }"
          @click="selectItem(item.id)"
        >
          <span class="priority-stripe" :class="item.priority.toLowerCase()" aria-hidden="true" />
          <span class="card-main">
            <span class="card-title-row">
              <strong :title="item.title">{{ item.title }}</strong>
              <el-tag
                class="priority-tag"
                :class="item.priority.toLowerCase()"
                size="small"
                effect="plain"
                >{{ item.priority }}</el-tag
              >
            </span>
            <span class="card-meta"
              ><span><User />{{ item.personName }}</span
              ><span
                ><Calendar />{{
                  item.attentionStatus === "active"
                    ? formatDateTime(item.reviewAt)
                    : resultLabel(item)
                }}</span
              ></span
            >
            <span
              v-if="item.latestProgress || externalDeadlineReached(item) || item.links.length"
              class="card-supporting"
            >
              <span
                v-if="item.latestProgress"
                class="latest-progress"
                :title="item.latestProgress.content"
                >{{ item.latestProgress.content }}</span
              >
              <span class="card-indicators">
                <el-tag
                  v-if="externalDeadlineReached(item)"
                  type="danger"
                  size="small"
                  effect="plain"
                  >外部期限已到</el-tag
                >
                <span v-if="item.links.length" class="link-summary"
                  ><Link />{{ item.links.length }} 个相关链接</span
                >
              </span>
            </span>
          </span>
        </button>
      </div>
    </section>

    <section class="follow-up-detail-pane">
      <el-empty v-if="!selected" description="选择一项查看详情" :image-size="72" />
      <template v-else>
        <header class="detail-header">
          <el-button
            class="mobile-back"
            :icon="Back"
            circle
            title="返回关注事项列表"
            @click="selectedId = null"
          />
          <div class="detail-title">
            <h2>{{ selected.title }}</h2>
            <p>
              {{ selected.personName }} ·
              {{ selected.attentionStatus === "active" ? "关注中" : resultLabel(selected) }}
            </p>
          </div>
          <el-dropdown trigger="click">
            <el-button :icon="MoreFilled" circle title="更多操作" />
            <template #dropdown
              ><el-dropdown-menu>
                <el-dropdown-item @click="startEdit">编辑</el-dropdown-item>
                <el-dropdown-item @click="createTodoDraft">创建任务</el-dropdown-item>
                <el-dropdown-item v-if="selected.attentionStatus === 'active'" @click="snooze"
                  >稍后提醒</el-dropdown-item
                >
                <el-dropdown-item divided class="danger-item" @click="removeItem"
                  >删除</el-dropdown-item
                >
              </el-dropdown-menu></template
            >
          </el-dropdown>
        </header>
        <div class="detail-scroll">
          <dl class="detail-grid">
            <div>
              <dt>复查时间</dt>
              <dd>{{ formatDateTime(selected.reviewAt) }}</dd>
            </div>
            <div>
              <dt>预计完成</dt>
              <dd>{{ formatDateTime(selected.expectedCompletionAt) }}</dd>
            </div>
            <div>
              <dt>优先级</dt>
              <dd>{{ selected.priority }}</dd>
            </div>
            <div>
              <dt>外部结果</dt>
              <dd>{{ resultLabel(selected) }}</dd>
            </div>
          </dl>
          <section v-if="selected.expectedOutcome" class="detail-section">
            <h3>预期结果</h3>
            <p>{{ selected.expectedOutcome }}</p>
          </section>
          <section v-if="selected.description" class="detail-section">
            <h3>描述</h3>
            <p class="pre-wrap">{{ selected.description }}</p>
          </section>
          <section v-if="selected.links.length" class="detail-section">
            <h3>相关链接</h3>
            <button
              v-for="link in selected.links"
              :key="link.id ?? link.url"
              class="text-link"
              @click="openUrl(link.url)"
            >
              <Link />{{ link.title || link.url }}
            </button>
          </section>
          <section class="detail-section timeline-section">
            <div class="section-title-row">
              <h3>进展时间线</h3>
              <el-button
                v-if="selected.attentionStatus === 'active'"
                size="small"
                @click="addProgress"
                >记录进展</el-button
              >
            </div>
            <el-empty
              v-if="!selected.progress.length"
              description="暂无进展记录"
              :image-size="56"
            />
            <div v-for="entry in selected.progress" :key="entry.id" class="timeline-entry">
              <span class="timeline-dot" />
              <div>
                <div class="timeline-meta">
                  <strong>{{ progressKindLabel(entry.kind) }}</strong
                  ><span>{{ formatDateTime(entry.occurredAt) }}</span>
                </div>
                <p>{{ entry.content }}</p>
              </div>
              <el-dropdown v-if="entry.kind === 'progress'" trigger="click"
                ><button class="icon-plain" title="编辑进展"><MoreFilled /></button
                ><template #dropdown
                  ><el-dropdown-menu
                    ><el-dropdown-item @click="editProgress(entry)">编辑</el-dropdown-item
                    ><el-dropdown-item class="danger-item" @click="deleteProgress(entry)"
                      >删除</el-dropdown-item
                    ></el-dropdown-menu
                  ></template
                ></el-dropdown
              >
            </div>
          </section>
        </div>
        <footer class="lifecycle-actions">
          <template v-if="selected.attentionStatus === 'active'">
            <el-button @click="openLifecycle('continue')">继续关注</el-button
            ><el-button type="success" @click="openLifecycle('completed')">确认完成</el-button
            ><el-button @click="openLifecycle('canceled')">确认取消</el-button
            ><el-button type="warning" plain @click="openLifecycle('stop')">结束关注</el-button>
          </template>
          <el-button v-else type="primary" @click="openLifecycle('reopen')">重新关注</el-button>
        </footer>
      </template>
    </section>

    <el-dialog
      v-model="editVisible"
      :title="draft.id ? '编辑关注事项' : '新增关注事项'"
      width="620px"
      destroy-on-close
    >
      <el-form label-position="top" @submit.prevent>
        <el-form-item label="标题" required
          ><el-input v-model="draft.title" maxlength="120"
        /></el-form-item>
        <div class="form-grid">
          <el-form-item label="责任人" required
            ><div class="person-field">
              <el-select v-model="draft.personId" filterable placeholder="选择人员名录"
                ><el-option
                  v-for="person in assignees"
                  :key="person.id"
                  :label="person.name"
                  :value="person.id" /></el-select
              ><el-button
                :icon="Plus"
                title="新增人员"
                @click="quickAddPerson"
              /></div></el-form-item
          ><el-form-item label="优先级"
            ><el-select v-model="draft.priority"
              ><el-option
                v-for="priority in priorities"
                :key="priority"
                :label="priority"
                :value="priority" /></el-select
          ></el-form-item>
        </div>
        <el-form-item label="复查时间" :required="!editingEnded"
          ><div class="date-field">
            <div class="review-date-time">
              <el-date-picker
                v-model="draftReviewDate"
                type="date"
                value-format="YYYY-MM-DD"
                placeholder="选择复查日期"
                :disabled="editingEnded"
              />
              <el-time-select
                v-model="draftReviewTime"
                class="follow-up-review-time"
                start="00:00"
                step="00:15"
                end="23:45"
                placeholder="选择时间"
                :clearable="false"
                :disabled="editingEnded || !draftReviewDate"
              />
            </div>
            <el-button-group
              ><el-button
                v-for="shortcut in shortcuts"
                :key="shortcut.days"
                size="small"
                @click="draft.reviewAt = quickReviewAt(shortcut.days)"
                >{{ shortcut.label }}</el-button
              ></el-button-group
            >
          </div></el-form-item
        >
        <el-form-item label="预计完成时间"
          ><el-date-picker
            v-model="draft.expectedCompletionAt"
            clearable
            type="datetime"
            value-format="YYYY-MM-DDTHH:mm:ssZ"
            placeholder="可选"
        /></el-form-item>
        <el-form-item label="预期结果"><el-input v-model="draft.expectedOutcome" /></el-form-item>
        <el-form-item label="描述"
          ><el-input v-model="draft.description" type="textarea" :rows="4"
        /></el-form-item>
        <el-form-item label="相关链接"
          ><div class="links-editor">
            <div v-for="(link, index) in draft.links" :key="index" class="link-row">
              <el-input v-model="link.title" placeholder="标题（可选）" /><el-input
                v-model="link.url"
                placeholder="https://"
              /><el-button :icon="Delete" title="删除链接" @click="draft.links.splice(index, 1)" />
            </div>
            <el-button link type="primary" @click="draft.links.push({ title: '', url: '' })"
              >添加链接</el-button
            >
          </div></el-form-item
        >
      </el-form>
      <template #footer
        ><el-button @click="editVisible = false">取消</el-button
        ><el-button type="primary" :loading="saving" @click="saveItem">保存</el-button></template
      >
    </el-dialog>

    <el-dialog v-model="lifecycleVisible" :title="lifecycleTitle" width="500px">
      <el-form label-position="top">
        <el-form-item
          v-if="lifecycleMode !== 'reopen'"
          :label="lifecycleMode === 'stop' ? '结束原因' : '进展或结果'"
          required
          ><el-input v-model="lifecycleContent" type="textarea" :rows="4"
        /></el-form-item>
        <el-form-item v-if="lifecycleNeedsReview" label="新的复查时间" required
          ><div class="date-field">
            <div class="review-date-time">
              <el-date-picker
                v-model="lifecycleReviewDate"
                type="date"
                value-format="YYYY-MM-DD"
                placeholder="选择复查日期"
              />
              <el-time-select
                v-model="lifecycleReviewTime"
                class="follow-up-review-time"
                start="00:00"
                step="00:15"
                end="23:45"
                placeholder="选择时间"
                :clearable="false"
                :disabled="!lifecycleReviewDate"
              />
            </div>
            <el-button-group
              ><el-button
                v-for="shortcut in shortcuts"
                :key="shortcut.days"
                size="small"
                @click="lifecycleReviewAt = quickReviewAt(shortcut.days)"
                >{{ shortcut.label }}</el-button
              ></el-button-group
            >
          </div></el-form-item
        >
      </el-form>
      <template #footer
        ><el-button @click="lifecycleVisible = false">取消</el-button
        ><el-button type="primary" :loading="saving" @click="submitLifecycle"
          >确认</el-button
        ></template
      >
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  Back,
  Calendar,
  Delete,
  Link,
  MoreFilled,
  Plus,
  Refresh,
  User,
} from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../../bridge/tauri";
import { APP_EVENTS } from "../../bridge/events";
import type {
  FollowUpDraft,
  FollowUpFilters,
  FollowUpGroup,
  FollowUpItem,
  FollowUpProgress,
  TodoAssignee,
} from "../../types/follow-up";
import {
  emptyFollowUpDraft,
  externalDeadlineReached,
  followUpGroup,
  groupFollowUpItems,
  quickReviewAt,
} from "../../utils/followUp";
import { combineDateTimeParts, splitDateTimeParts } from "../../utils/todoSchedule";

const emit = defineEmits<{
  createTodo: [item: FollowUpItem];
  dueCountChange: [count: number];
}>();
type FollowUpSectionKey = "all" | FollowUpGroup;
let reminderUnlisten: UnlistenFn | null = null;
let loadSequence = 0;
const loading = ref(true),
  saving = ref(false),
  items = ref<FollowUpItem[]>([]),
  assignees = ref<TodoAssignee[]>([]),
  selectedId = ref<number | null>(null),
  activeGroup = ref<FollowUpSectionKey>("all"),
  editVisible = ref(false),
  lifecycleVisible = ref(false);
const filters = reactive<FollowUpFilters>({
  keyword: "",
  personId: null,
  priority: null,
  attentionStatus: null,
});
const draft = reactive<FollowUpDraft>(emptyFollowUpDraft());
const priorities = ["P0", "P1", "P2", "P3"] as const;
const shortcuts = [
  { label: "明天", days: 1 },
  { label: "3 天后", days: 3 },
  { label: "1 周后", days: 7 },
];
const sections: ReadonlyArray<{ key: FollowUpSectionKey; label: string }> = [
  { key: "all", label: "全部" },
  { key: "due", label: "待复查" },
  { key: "soon", label: "近期复查" },
  { key: "later", label: "以后复查" },
  { key: "ended", label: "已结束" },
];
const groups = computed(() => groupFollowUpItems(items.value));
const allItems = computed(() => [
  ...groups.value.due,
  ...groups.value.soon,
  ...groups.value.later,
  ...groups.value.ended,
]);
const sectionCounts = computed<Record<FollowUpSectionKey, number>>(() => ({
  all: allItems.value.length,
  due: groups.value.due.length,
  soon: groups.value.soon.length,
  later: groups.value.later.length,
  ended: groups.value.ended.length,
}));
const visibleItems = computed(() =>
  activeGroup.value === "all" ? allItems.value : groups.value[activeGroup.value],
);
const selected = computed(() => items.value.find((item) => item.id === selectedId.value) ?? null);
const activeGroupLabel = computed(
  () => sections.find((section) => section.key === activeGroup.value)?.label ?? "",
);
const emptyDescription = computed(() =>
  filters.keyword || filters.personId || filters.priority || filters.attentionStatus
    ? "当前筛选条件下暂无关注事项"
    : activeGroup.value === "all"
      ? "暂无关注事项"
      : `${activeGroupLabel.value}暂无关注事项`,
);
const editingEnded = computed(() =>
  Boolean(draft.id && selected.value?.attentionStatus === "ended"),
);
type LifecycleMode = "continue" | "completed" | "canceled" | "stop" | "reopen";
const lifecycleMode = ref<LifecycleMode>("continue"),
  lifecycleContent = ref(""),
  lifecycleReviewAt = ref("");
function reviewPart(value: string, part: "date" | "time") {
  return splitDateTimeParts(value)[part];
}
function updateReviewPart(value: string, part: "date" | "time", nextPart: string) {
  if (!nextPart) return "";
  const current = splitDateTimeParts(value);
  const date = part === "date" ? nextPart : current.date;
  const time = part === "time" ? nextPart : current.time || "09:00";
  return combineDateTimeParts(date, time) ?? "";
}
const draftReviewDate = computed({
  get: () => reviewPart(draft.reviewAt, "date"),
  set: (value: string) => {
    draft.reviewAt = updateReviewPart(draft.reviewAt, "date", value);
  },
});
const draftReviewTime = computed({
  get: () => reviewPart(draft.reviewAt, "time"),
  set: (value: string) => {
    draft.reviewAt = updateReviewPart(draft.reviewAt, "time", value);
  },
});
const lifecycleReviewDate = computed({
  get: () => reviewPart(lifecycleReviewAt.value, "date"),
  set: (value: string) => {
    lifecycleReviewAt.value = updateReviewPart(lifecycleReviewAt.value, "date", value);
  },
});
const lifecycleReviewTime = computed({
  get: () => reviewPart(lifecycleReviewAt.value, "time"),
  set: (value: string) => {
    lifecycleReviewAt.value = updateReviewPart(lifecycleReviewAt.value, "time", value);
  },
});
const lifecycleNeedsReview = computed(
  () => lifecycleMode.value === "continue" || lifecycleMode.value === "reopen",
);
const lifecycleTitle = computed(
  () =>
    ({
      continue: "继续关注",
      completed: "确认完成",
      canceled: "确认取消",
      stop: "结束关注",
      reopen: "重新关注",
    })[lifecycleMode.value],
);

async function loadAssignees() {
  const result = await invokeToolByChannel<{ items: TodoAssignee[] }>(
    "tool:todo:assignee-list",
    {},
  );
  assignees.value = result.items;
}
function hasActiveFilters() {
  return Boolean(
    filters.keyword || filters.personId || filters.priority || filters.attentionStatus,
  );
}
function emitDueCount(result: FollowUpItem[]) {
  emit("dueCountChange", groupFollowUpItems(result).due.length);
}
async function loadDueCount() {
  const result = await invokeToolByChannel<FollowUpItem[]>("tool:follow-up:item-list", {});
  emitDueCount(result);
}
async function loadItems(refreshDueCount = false) {
  const sequence = ++loadSequence;
  try {
    const result = await invokeToolByChannel<FollowUpItem[]>("tool:follow-up:item-list", {
      keyword: filters.keyword,
      personId: filters.personId,
      priority: filters.priority,
      attentionStatus: filters.attentionStatus,
    });
    if (sequence !== loadSequence) return;
    items.value = result;
    if (!hasActiveFilters()) emitDueCount(result);
    else if (refreshDueCount) await loadDueCount();
    if (selectedId.value && !result.some((item) => item.id === selectedId.value))
      selectedId.value = null;
  } catch (error) {
    if (sequence !== loadSequence) return;
    ElMessage.error((error as Error).message || "加载关注事项失败");
  }
}
async function reloadSelected() {
  const id = selectedId.value;
  await loadItems(true);
  if (id) selectedId.value = id;
}
function selectItem(id: number) {
  selectedId.value = id;
}
function assignDraft(value: FollowUpDraft) {
  Object.assign(draft, value);
}
function startCreate() {
  assignDraft(emptyFollowUpDraft());
  editVisible.value = true;
}
function startEdit() {
  if (!selected.value) return;
  assignDraft({
    id: selected.value.id,
    title: selected.value.title,
    description: selected.value.description,
    expectedOutcome: selected.value.expectedOutcome,
    priority: selected.value.priority,
    personId: selected.value.personId,
    reviewAt: selected.value.reviewAt ?? "",
    expectedCompletionAt: selected.value.expectedCompletionAt ?? "",
    links: selected.value.links.map((link) => ({ ...link })),
  });
  editVisible.value = true;
}
async function saveItem() {
  if (!draft.title.trim()) {
    ElMessage.warning("请输入标题");
    return;
  }
  if (!draft.personId) {
    ElMessage.warning("请选择责任人");
    return;
  }
  if (!editingEnded.value && !draft.reviewAt) {
    ElMessage.warning("请选择复查时间");
    return;
  }
  saving.value = true;
  try {
    const channel = draft.id ? "tool:follow-up:item-update" : "tool:follow-up:item-create";
    const saved = await invokeToolByChannel<FollowUpItem>(channel, { ...draft });
    editVisible.value = false;
    selectedId.value = saved.id;
    await loadItems(true);
    const current = items.value.find((item) => item.id === saved.id);
    if (current && activeGroup.value !== "all") activeGroup.value = followUpGroup(current);
    ElMessage.success(draft.id ? "关注事项已更新" : "关注事项已创建");
  } catch (error) {
    ElMessage.error((error as Error).message || "保存失败");
  } finally {
    saving.value = false;
  }
}
async function quickAddPerson() {
  try {
    const { value } = await ElMessageBox.prompt("输入人员名称", "新增人员", {
      inputValidator: (value) => Boolean(value.trim()) || "名称不能为空",
    });
    const result = await invokeToolByChannel<{ id: number }>("tool:todo:assignee-upsert", {
      name: value.trim(),
    });
    await loadAssignees();
    draft.personId = result.id;
  } catch (error) {
    if (error !== "cancel" && error !== "close")
      ElMessage.error((error as Error).message || "新增人员失败");
  }
}
function openLifecycle(mode: LifecycleMode) {
  lifecycleMode.value = mode;
  lifecycleContent.value = "";
  lifecycleReviewAt.value = quickReviewAt(1);
  lifecycleVisible.value = true;
}
async function submitLifecycle() {
  if (!selected.value) return;
  if (lifecycleMode.value !== "reopen" && !lifecycleContent.value.trim()) {
    ElMessage.warning("请输入内容");
    return;
  }
  if (lifecycleNeedsReview.value && !lifecycleReviewAt.value) {
    ElMessage.warning("请选择新的复查时间");
    return;
  }
  const channels = {
    continue: "tool:follow-up:continue",
    completed: "tool:follow-up:confirm-completed",
    canceled: "tool:follow-up:confirm-canceled",
    stop: "tool:follow-up:stop",
    reopen: "tool:follow-up:reopen",
  } as const;
  saving.value = true;
  try {
    await invokeToolByChannel(channels[lifecycleMode.value], {
      id: selected.value.id,
      content: lifecycleContent.value,
      reviewAt: lifecycleReviewAt.value,
    });
    lifecycleVisible.value = false;
    await reloadSelected();
    if (selected.value && activeGroup.value !== "all")
      activeGroup.value = followUpGroup(selected.value);
    ElMessage.success("状态已更新");
  } catch (error) {
    ElMessage.error((error as Error).message || "状态更新失败");
  } finally {
    saving.value = false;
  }
}
async function promptProgress(
  message: string,
  title: string,
  initialValue: string,
  persist: (content: string) => Promise<unknown>,
  failureMessage: string,
) {
  let inputValue = initialValue;
  while (true) {
    let value: string;
    try {
      ({ value } = await ElMessageBox.prompt(message, title, {
        inputValue,
        inputType: "textarea",
        inputValidator: (candidate) => Boolean(candidate.trim()) || "进展内容不能为空",
      }));
    } catch (error) {
      if (error !== "cancel" && error !== "close")
        ElMessage.error((error as Error).message || failureMessage);
      return;
    }
    try {
      await persist(value);
      await reloadSelected();
      return;
    } catch (error) {
      inputValue = value;
      ElMessage.error((error as Error).message || failureMessage);
    }
  }
}
async function addProgress() {
  const itemId = selected.value?.id;
  if (!itemId) return;
  await promptProgress(
    "记录责任人反馈或当前进展",
    "记录进展",
    "",
    (content) => invokeToolByChannel("tool:follow-up:progress-add", { id: itemId, content }),
    "记录进展失败",
  );
}
async function editProgress(entry: FollowUpProgress) {
  await promptProgress(
    "修改进展内容",
    "编辑进展",
    entry.content,
    (content) =>
      invokeToolByChannel("tool:follow-up:progress-update", {
        progressId: entry.id,
        content,
      }),
    "编辑进展失败",
  );
}
async function deleteProgress(entry: FollowUpProgress) {
  try {
    await ElMessageBox.confirm("确定删除这条进展记录吗？", "删除进展", { type: "warning" });
    await invokeToolByChannel("tool:follow-up:progress-delete", { progressId: entry.id });
    await reloadSelected();
  } catch (error) {
    if (error !== "cancel" && error !== "close")
      ElMessage.error((error as Error).message || "删除进展失败");
  }
}
async function snooze() {
  if (!selected.value) return;
  try {
    await invokeToolByChannel("tool:follow-up:item-snooze", { id: selected.value.id, minutes: 60 });
    await reloadSelected();
    ElMessage.success("将在 1 小时后再次提醒");
  } catch (error) {
    ElMessage.error((error as Error).message || "稍后提醒失败");
  }
}
async function removeItem() {
  if (!selected.value) return;
  try {
    await ElMessageBox.confirm(
      `确定删除“${selected.value.title}”及全部进展记录吗？`,
      "删除关注事项",
      { type: "warning" },
    );
    await invokeToolByChannel("tool:follow-up:item-delete", { id: selected.value.id });
    selectedId.value = null;
    await loadItems(true);
    ElMessage.success("关注事项已删除");
  } catch (error) {
    if (error !== "cancel" && error !== "close")
      ElMessage.error((error as Error).message || "删除失败");
  }
}
function createTodoDraft() {
  if (selected.value) emit("createTodo", selected.value);
}
function formatDateTime(value: string | null) {
  if (!value) return "未设置";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : new Intl.DateTimeFormat("zh-CN", {
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      }).format(date);
}
function resultLabel(item: FollowUpItem) {
  if (item.externalResult === "completed") return "已完成";
  if (item.externalResult === "canceled") return "已取消";
  return item.attentionStatus === "ended" ? "已结束关注" : "结果未知";
}
function progressKindLabel(kind: FollowUpProgress["kind"]) {
  return {
    progress: "进展记录",
    continued: "继续关注",
    completed: "确认完成",
    canceled: "确认取消",
    stopped_following: "结束关注",
    reopened: "重新关注",
  }[kind];
}
async function openUrl(url: string) {
  try {
    await invokeToolByChannel("tool:todo:open-link", { url });
  } catch (error) {
    ElMessage.error((error as Error).message || "打开链接失败");
  }
}
async function focus(itemId: number | null, dueOnly = false) {
  activeGroup.value = "due";
  if (itemId) {
    if (!items.value.some((item) => item.id === itemId)) await loadItems();
    selectedId.value = items.value.some((item) => item.id === itemId) ? itemId : null;
    if (!dueOnly && selected.value) activeGroup.value = followUpGroup(selected.value);
  }
}
watch(filters, () => void loadItems(), { deep: true });
onMounted(async () => {
  try {
    await Promise.all([loadAssignees(), loadItems(true)]);
  } catch (error) {
    ElMessage.error((error as Error).message || "加载关注事项失败");
  } finally {
    loading.value = false;
  }
  try {
    reminderUnlisten = await listen(APP_EVENTS.FOLLOW_UP_REVIEW_DUE, () => void loadItems(true));
  } catch (error) {
    reminderUnlisten = null;
    ElMessage.error((error as Error).message || "关注事项提醒监听失败");
  }
});
onBeforeUnmount(() => {
  reminderUnlisten?.();
  reminderUnlisten = null;
});
defineExpose({ focus, loadItems });
</script>

<style scoped>
.follow-up-panel {
  display: grid;
  grid-template-columns: 260px minmax(300px, 1fr) minmax(320px, 0.9fr);
  height: 100%;
  min-height: 0;
  background: #f6f7f9;
  color: #263238;
}
.follow-up-sidebar,
.follow-up-list-pane,
.follow-up-detail-pane {
  min-height: 0;
  background: #fff;
}
.follow-up-sidebar {
  padding: 18px 12px;
  border-right: 1px solid #e6e9ee;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.section-button {
  border: 0;
  background: transparent;
  display: flex;
  justify-content: space-between;
  padding: 9px 10px;
  border-radius: 6px;
  color: #52606d;
  cursor: pointer;
}
.section-button:hover,
.section-button.active {
  background: #eef5ff;
  color: #2563a9;
}
.section-count {
  font-variant-numeric: tabular-nums;
}
.sidebar-filter-title {
  font-size: 12px;
  color: #8a94a1;
  margin: 14px 8px 2px;
}
.follow-up-list-pane {
  display: flex;
  flex-direction: column;
  border-right: 1px solid #e6e9ee;
}
.follow-up-toolbar {
  display: grid;
  grid-template-columns: minmax(160px, 1fr) auto auto;
  gap: 8px;
  padding: 14px;
  border-bottom: 1px solid #e6e9ee;
}
.follow-up-scroll,
.detail-scroll {
  min-height: 0;
  overflow: auto;
  padding: 14px;
}
.group-heading {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  font-weight: 700;
  color: #596574;
  margin-bottom: 10px;
}
.follow-up-card {
  position: relative;
  width: 100%;
  display: block;
  text-align: left;
  border: 1px solid #e5e9ef;
  background: #fff;
  border-radius: 6px;
  padding: 12px 12px 12px 16px;
  margin-bottom: 8px;
  cursor: pointer;
  overflow: hidden;
  font: inherit;
  color: inherit;
  transition:
    border-color 160ms ease,
    background-color 160ms ease;
}
.follow-up-card:hover,
.follow-up-card.selected {
  border-color: #8db9e8;
  background: #f7fbff;
}
.follow-up-card:focus-visible {
  outline: 2px solid #4185c5;
  outline-offset: 2px;
}
.priority-stripe {
  position: absolute;
  inset: 0 auto 0 0;
  width: 4px;
  background: #8d99a6;
}
.priority-stripe.p0 {
  background: #d64c4c;
}
.priority-stripe.p1 {
  background: #e49132;
}
.priority-stripe.p2 {
  background: #4185c5;
}
.priority-stripe.p3 {
  background: #7a8a99;
}
.card-main {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.card-title-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.card-title-row strong {
  min-width: 0;
  font-size: 14px;
  line-height: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.priority-tag {
  flex: 0 0 auto;
  min-width: 34px;
  justify-content: center;
  font-variant-numeric: tabular-nums;
}
.priority-tag.p0 {
  color: #bd3434;
  border-color: #efb6b6;
  background: #fff7f7;
}
.priority-tag.p1 {
  color: #a96110;
  border-color: #edc791;
  background: #fffaf2;
}
.card-meta {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px 12px;
  min-width: 0;
  font-size: 12px;
  color: #66717d;
}
.card-meta span,
.link-summary,
.text-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.card-meta span {
  min-width: 0;
}
.card-meta span:first-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card-meta svg,
.link-summary svg {
  width: 14px;
  height: 14px;
  flex: 0 0 14px;
}
.card-supporting {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-width: 0;
  padding-top: 7px;
  border-top: 1px solid #edf0f3;
}
.latest-progress {
  min-width: 0;
  flex: 1;
  font-size: 12px;
  color: #4d5966;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card-indicators {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  flex: 0 0 auto;
}
.link-summary {
  font-size: 11px;
  color: #7b8794;
  white-space: nowrap;
}
.follow-up-detail-pane {
  display: flex;
  flex-direction: column;
}
.detail-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding: 16px 18px;
  border-bottom: 1px solid #e6e9ee;
}
.mobile-back {
  display: none;
}
.detail-title {
  min-width: 0;
  flex: 1;
}
.detail-header h2 {
  font-size: 17px;
  margin: 0 0 5px;
  overflow-wrap: anywhere;
}
.detail-header p {
  font-size: 12px;
  color: #788390;
  margin: 0;
}
.detail-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  margin: 0 0 18px;
}
.detail-grid div {
  background: #f6f8fa;
  padding: 10px;
  border-radius: 5px;
}
.detail-grid dt {
  font-size: 11px;
  color: #7b8794;
}
.detail-grid dd {
  font-size: 13px;
  margin: 4px 0 0;
}
.detail-section {
  border-top: 1px solid #edf0f3;
  padding: 15px 0;
}
.detail-section h3 {
  font-size: 13px;
  margin: 0 0 9px;
}
.detail-section p {
  font-size: 13px;
  line-height: 1.6;
  margin: 0;
}
.pre-wrap {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.text-link {
  border: 0;
  background: transparent;
  color: #2563a9;
  padding: 3px 0;
  cursor: pointer;
  max-width: 100%;
  overflow-wrap: anywhere;
}
.section-title-row,
.timeline-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.timeline-entry {
  display: grid;
  grid-template-columns: 10px minmax(0, 1fr) auto;
  gap: 8px;
  padding: 10px 0;
}
.timeline-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #4185c5;
  margin-top: 5px;
}
.timeline-meta {
  gap: 8px;
  font-size: 11px;
  color: #87919c;
}
.timeline-entry p {
  margin: 4px 0 0;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.icon-plain {
  border: 0;
  background: transparent;
  color: #7b8794;
  cursor: pointer;
}
.lifecycle-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  padding: 12px 16px;
  border-top: 1px solid #e6e9ee;
}
.form-grid {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: 12px;
}
.person-field,
.date-field {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}
.review-date-time {
  display: grid;
  grid-template-columns: minmax(170px, 1fr) 112px;
  gap: 8px;
  min-width: 0;
  flex: 1;
}
.review-date-time .el-date-editor,
.review-date-time .el-select {
  width: 100%;
}
.person-field .el-select,
.date-field .el-date-editor {
  flex: 1;
}
.links-editor {
  width: 100%;
}
.link-row {
  display: grid;
  grid-template-columns: 0.7fr 1.3fr auto;
  gap: 7px;
  margin-bottom: 7px;
}
:deep(.danger-item) {
  color: #c23b3b;
}
@media (max-width: 1280px) {
  .follow-up-panel {
    grid-template-columns: 240px minmax(280px, 1fr) minmax(320px, 0.9fr);
  }
}
@media (max-width: 1050px) {
  .follow-up-panel {
    grid-template-columns: 240px minmax(280px, 1fr);
  }
  .follow-up-detail-pane {
    position: absolute;
    inset: 0 0 0 240px;
    z-index: 4;
  }
  .follow-up-detail-pane:has(.el-empty) {
    display: none;
  }
  .mobile-back {
    display: inline-flex;
    flex: 0 0 auto;
  }
}
@media (max-width: 1024px) {
  .follow-up-panel {
    grid-template-columns: 220px minmax(280px, 1fr);
  }
  .follow-up-detail-pane {
    inset-inline-start: 220px;
  }
}
@media (max-width: 760px) {
  .follow-up-panel {
    grid-template-columns: 1fr;
  }
  .follow-up-sidebar {
    display: none;
  }
  .follow-up-detail-pane {
    inset: 0;
  }
  .follow-up-toolbar {
    grid-template-columns: 1fr auto;
  }
  .follow-up-toolbar .el-button--primary {
    grid-column: 1/-1;
  }
  .form-grid {
    grid-template-columns: 1fr;
  }
  .date-field {
    align-items: flex-start;
    flex-direction: column;
  }
  .review-date-time {
    width: 100%;
  }
  .card-meta {
    grid-template-columns: 1fr;
  }
  .card-supporting {
    align-items: flex-start;
    flex-direction: column;
  }
  .card-indicators {
    justify-content: flex-start;
  }
}
</style>
