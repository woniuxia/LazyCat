<template>
  <div class="todo-panel">
    <div class="toolbar">
      <div class="toolbar-right">
        <el-input
          v-model.trim="itemKeyword"
          clearable
          placeholder="搜索标题或描述"
          style="width: 220px"
        />
        <el-button @click="loadItems">刷新</el-button>
        <el-button type="primary" @click="openCreateItemDialog()">新增事项</el-button>
        <el-button
          text
          class="toolbar-settings-btn"
          title="基础数据设置"
          aria-label="基础数据设置"
          @click="basicsDialogVisible = true"
        >
          <el-icon><Setting /></el-icon>
        </el-button>
      </div>
    </div>

    <div class="todo-layout">
      <aside class="todo-stats">
        <div class="stats-section">
          <div class="stats-section-title">概览</div>
          <div class="stats-grid">
            <div class="stat-card">
              <div class="stat-number">{{ activeItems.length }}</div>
              <div class="stat-label">待办</div>
            </div>
            <div class="stat-card">
              <div class="stat-number">{{ doneItems.length }}</div>
              <div class="stat-label">已完成</div>
            </div>
            <div class="stat-card">
              <div class="stat-number">{{ todayDueCount }}</div>
              <div class="stat-label">今日到期</div>
            </div>
            <div class="stat-card" :class="{ 'is-alert': overdueCount > 0 }">
              <div class="stat-number">{{ overdueCount }}</div>
              <div class="stat-label">逾期</div>
            </div>
          </div>
        </div>
        <div v-if="typeDistribution.length > 0" class="stats-section">
          <div class="stats-section-header">
            <div class="stats-section-title">分类分布</div>
            <el-button size="small" link type="primary" :disabled="filterType === null" @click="clearTypeFilter">
              清空
            </el-button>
          </div>
          <div class="stats-bar-list">
            <div
              v-for="entry in typeDistribution"
              :key="entry.name"
              class="stats-bar-item is-clickable"
              :class="{ 'is-active': filterType === entry.name }"
              @click="toggleTypeFilter(entry.name)"
            >
              <div class="stats-bar-label">
                <span class="color-dot" :style="{ backgroundColor: entry.color }" />
                <span>{{ entry.name }}</span>
                <span class="stats-bar-count">{{ entry.count }}</span>
              </div>
              <div class="stats-bar-track">
                <div
                  class="stats-bar-fill"
                  :style="{ width: statsBarWidth(entry.count, typeDistribution), backgroundColor: entry.color }"
                />
              </div>
            </div>
          </div>
        </div>
        <div v-if="priorityDistribution.length > 0" class="stats-section">
          <div class="stats-section-header">
            <div class="stats-section-title">优先级分布</div>
            <el-button
              size="small"
              link
              type="primary"
              :disabled="filterPriority === null"
              @click="clearPriorityFilter"
            >
              清空
            </el-button>
          </div>
          <div class="stats-bar-list">
            <div
              v-for="entry in priorityDistribution"
              :key="entry.priority"
              class="stats-bar-item is-clickable"
              :class="{ 'is-active': filterPriority === entry.priority }"
              @click="togglePriorityFilter(entry.priority)"
            >
              <div class="stats-bar-label">
                <span class="priority-dot" :class="'priority-' + entry.priority.toLowerCase()" />
                <span>{{ entry.priority }}</span>
                <span class="stats-bar-count">{{ entry.count }}</span>
              </div>
              <div class="stats-bar-track">
                <div
                  class="stats-bar-fill"
                  :class="'priority-bar-' + entry.priority.toLowerCase()"
                  :style="{ width: statsBarWidth(entry.count, priorityDistribution) }"
                />
              </div>
            </div>
          </div>
        </div>
      </aside>
      <div class="todo-main">
        <div v-if="hasActiveFilter" class="filter-indicator">
          <span class="filter-indicator-text">
            已筛选
            <template v-if="filterType !== null">分类「{{ filterType }}」</template>
            <template v-if="filterType !== null && filterPriority !== null">、</template>
            <template v-if="filterPriority !== null">优先级 {{ filterPriority }}</template>
          </span>
          <el-button size="small" link type="primary" @click="clearAllFilters">清除筛选</el-button>
        </div>

    <div class="item-section">
      <div class="item-section-header">
        <div class="item-section-title-wrap">
          <h3 class="item-section-title">待办事项</h3>
          <el-tag size="small" effect="light" class="count-tag">{{ displayActiveItems.length }}</el-tag>
        </div>
      </div>

      <el-empty
        v-if="displayActiveItems.length === 0"
        :description="hasActiveFilter ? '当前筛选条件下暂无待办事项' : '暂无待办事项'"
      />
      <el-table
        v-else
        :data="displayActiveItems"
        :row-class-name="todoRowClassName"
        size="small"
        class="todo-table"
      >
        <el-table-column width="40" align="center">
          <template #default="{ row }">
            <el-checkbox
              :model-value="isDoneItem(row)"
              :disabled="!row.status"
              @change="onCheckItem(row)"
            />
          </template>
        </el-table-column>
        <el-table-column label="标题" min-width="200">
          <template #default="{ row }">
            <div class="title-cell">
              <span class="title-left">
                <span class="item-title">{{ row.title }}</span>
                <span v-if="hasRepeatRule(row)" class="item-badge badge-repeat" title="重复">
                  <el-icon :size="12"><Refresh /></el-icon>
                  <span class="badge-text">重复</span>
                </span>
              </span>
              <span class="title-right">
                <span v-if="row.pinned" class="item-badge badge-pinned" title="置顶">
                  <el-icon :size="12"><Top /></el-icon>
                  <span class="badge-text">置顶</span>
                </span>
                <span v-if="isItemOverdue(row)" class="item-badge badge-overdue" title="逾期">
                  <el-icon :size="12"><AlarmClock /></el-icon>
                  <span class="badge-text">逾期</span>
                </span>
              </span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="时间" width="160">
          <template #default="{ row }">
            {{ itemTimeLabel(row) }}
          </template>
        </el-table-column>
        <el-table-column label="分类" width="120">
          <template #default="{ row }">
            <template v-if="row.typeName">
              <span v-if="row.typeColor" class="color-dot" :style="{ backgroundColor: row.typeColor }" />
              {{ row.typeName }}
            </template>
          </template>
        </el-table-column>
        <el-table-column label="优先级" width="100">
          <template #default="{ row }">
            <span class="priority-dot" :class="'priority-' + row.priority.toLowerCase()" />
            {{ row.priority }}
          </template>
        </el-table-column>
        <el-table-column label="执行人" width="120">
          <template #default="{ row }">
            {{ row.assignees.map((a: TodoAssignee) => a.name).join("、") }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120" align="center">
          <template #default="{ row }">
            <div class="table-actions">
              <el-button size="small" link type="primary" @click="editItem(row)">编辑</el-button>
              <el-dropdown trigger="click" @command="(cmd: string) => handleRowAction(cmd, row)">
                <el-button size="small" link class="item-more-btn">
                  <el-icon><MoreFilled /></el-icon>
                </el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item v-if="canPinItem(row)" command="pin">
                      {{ row.pinned ? "取消置顶" : "置顶" }}
                    </el-dropdown-item>
                    <el-dropdown-item v-if="canCancelItem(row)" command="cancel">
                      取消事项
                    </el-dropdown-item>
                    <el-dropdown-item command="delete" divided>
                      <span style="color: var(--el-color-danger)">删除</span>
                    </el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <div class="item-section">
      <div class="item-section-header done-section-header" @click="toggleDoneCollapsed">
        <div class="item-section-title-wrap">
          <h3 class="item-section-title">已办事项</h3>
          <el-tag size="small" effect="light" class="count-tag">{{ displayDoneItems.length }}</el-tag>
        </div>
        <span class="done-toggle-icon" :class="{ 'is-collapsed': doneCollapsed }">▾</span>
      </div>

      <el-empty
        v-if="displayDoneItems.length === 0"
        :description="hasActiveFilter ? '当前筛选条件下暂无已办事项' : '暂无已办事项'"
      />
      <el-table
        v-else
        v-show="!doneCollapsed"
        :data="displayDoneItems"
        :row-class-name="doneRowClassName"
        size="small"
        class="todo-table done-table"
      >
        <el-table-column width="40" align="center">
          <template #default="{ row }">
            <el-checkbox
              :model-value="isDoneItem(row)"
              :disabled="!row.status"
              @change="onCheckItem(row)"
            />
          </template>
        </el-table-column>
        <el-table-column label="标题" min-width="200">
          <template #default="{ row }">
            <div class="title-cell">
              <span class="title-left">
                <span class="item-title is-done">{{ row.title }}</span>
                <span v-if="hasRepeatRule(row)" class="item-badge badge-repeat" title="重复">
                  <el-icon :size="12"><Refresh /></el-icon>
                  <span class="badge-text">重复</span>
                </span>
              </span>
              <span class="title-right">
                <span v-if="isItemOverdue(row)" class="item-badge badge-overdue" title="逾期">
                  <el-icon :size="12"><AlarmClock /></el-icon>
                  <span class="badge-text">逾期</span>
                </span>
              </span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="时间" width="160">
          <template #default="{ row }">
            {{ itemTimeLabel(row) }}
          </template>
        </el-table-column>
        <el-table-column label="分类" width="120">
          <template #default="{ row }">
            <template v-if="row.typeName">
              <span v-if="row.typeColor" class="color-dot" :style="{ backgroundColor: row.typeColor }" />
              {{ row.typeName }}
            </template>
          </template>
        </el-table-column>
        <el-table-column label="优先级" width="100">
          <template #default="{ row }">
            <span class="priority-dot" :class="'priority-' + row.priority.toLowerCase()" />
            {{ row.priority }}
          </template>
        </el-table-column>
        <el-table-column label="执行人" width="120">
          <template #default="{ row }">
            {{ row.assignees.map((a: TodoAssignee) => a.name).join("、") }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120" align="center">
          <template #default="{ row }">
            <div class="table-actions">
              <el-button size="small" link type="primary" @click="editItem(row)">编辑</el-button>
              <el-button size="small" link type="danger" @click="deleteItem(row)">删除</el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </div>

      </div>
    </div>

    <el-dialog
      v-model="basicsDialogVisible"
      title="基础数据设置"
      width="760px"
      :close-on-click-modal="false"
    >
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
                <el-button
                  size="small"
                  text
                  type="danger"
                  @click="removeType(row)"
                  >删除</el-button
                >
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
                <el-button size="small" text type="danger" @click="removeAssignee(row)"
                  >删除</el-button
                >
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </div>
    </el-dialog>

    <el-dialog
      v-model="itemDialogVisible"
      :title="itemDialogTitle"
      width="720px"
      custom-class="todo-item-dialog"
      :close-on-click-modal="false"
      @closed="resetItemDraft"
    >
      <el-form label-position="top" class="todo-item-form">
        <div class="todo-form-section">
          <el-form-item v-if="showScopeSelector" label="编辑范围">
            <el-radio-group v-model="itemDraft.scope" @change="onEditScopeChange">
              <el-radio value="this_instance">仅当前一次</el-radio>
              <el-radio value="future_instances">此后未发生项</el-radio>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="标题">
            <el-input v-model.trim="itemDraft.title" placeholder="请输入事项标题" />
          </el-form-item>
          <div class="todo-form-row">
            <el-form-item label="分类" class="todo-form-item-flex">
              <el-select
                v-model="itemDraft.typeId"
                clearable
                filterable
                allow-create
                default-first-option
                placeholder="可输入新分类"
                style="width: 100%"
              >
                <el-option
                  v-for="item in sortedTypes"
                  :key="item.id"
                  :label="item.name"
                  :value="item.id"
                />
              </el-select>
            </el-form-item>
            <el-form-item label="优先级" class="todo-form-item-flex">
              <el-select v-model="itemDraft.priority" style="width: 100%">
                <template #prefix>
                  <span class="priority-dot" :class="'priority-' + itemDraft.priority.toLowerCase()" />
                </template>
                <el-option
                  v-for="opt in priorityOptions"
                  :key="opt.value"
                  :label="opt.label"
                  :value="opt.value"
                >
                  <span class="priority-dot" :class="'priority-' + opt.value.toLowerCase()" />
                  {{ opt.label }}
                </el-option>
              </el-select>
            </el-form-item>
          </div>
          <el-form-item label="执行人">
            <el-select
              v-model="itemDraft.assigneeIds"
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

        <div class="todo-form-section">
          <div class="todo-form-row">
            <el-form-item label="日期" class="todo-form-item-date">
              <el-date-picker
                v-model="eventDateModel"
                type="date"
                value-format="YYYY-MM-DD"
                clearable
                style="width: 100%"
              />
            </el-form-item>
            <el-form-item label="时间" class="todo-form-item-time">
              <div class="time-picker-inline">
                <div class="time-picker-fused">
                  <el-select v-model="eventHour" class="time-fused-select" placeholder="时">
                    <el-option
                      v-for="option in hourOptions"
                      :key="option.value"
                      :label="option.label"
                      :value="option.value"
                    />
                  </el-select>
                  <span class="time-fused-separator">:</span>
                  <el-select v-model="eventMinute" class="time-fused-select" placeholder="分">
                    <el-option
                      v-for="option in minuteOptions"
                      :key="option.value"
                      :label="option.label"
                      :value="option.value"
                    />
                  </el-select>
                </div>
                <el-button text class="time-fused-clear" @click="clearEventSchedule">清空</el-button>
              </div>
            </el-form-item>
          </div>
          <el-form-item label="提醒">
            <el-select
              v-model="itemDraft.reminderPresets"
              multiple
              clearable
              style="width: 100%"
              @change="onReminderPresetsChange"
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
              v-model="itemDraft.repeatPreset"
              class="repeat-radio-group"
              @change="onRepeatPresetChange"
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
                      v-model="itemDraft.simple.frequency"
                      style="width: 100%"
                      @change="onCustomFrequencyChange"
                    >
                      <el-option label="每天" value="daily" />
                      <el-option label="每周" value="weekly" />
                      <el-option label="每月" value="monthly" />
                    </el-select>
                  </el-form-item>
                  <el-form-item label="间隔" class="todo-form-item-flex">
                    <el-input-number
                      v-model="itemDraft.simple.interval"
                      :min="1"
                      :max="365"
                      style="width: 100%"
                    />
                  </el-form-item>
                </div>
              </template>
              <el-form-item v-if="showWeeklyWeekdays" label="周几">
                <el-checkbox-group v-model="itemDraft.simple.weekdays">
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
                  v-model="itemDraft.simple.dayOfMonth"
                  :min="1"
                  :max="31"
                  style="width: 100%"
                />
              </el-form-item>
              <template v-if="showCronRepeatFields">
                <el-form-item label="Cron 表达式">
                  <el-input
                    v-model.trim="itemDraft.cronExpression"
                    placeholder="例如：0 0 9 * * 1-5"
                  />
                </el-form-item>
                <el-form-item label="时区">
                  <el-select
                    v-model="itemDraft.timezone"
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
                  <el-select v-model="itemDraft.endMode" style="width: 100%">
                    <el-option label="持续生成" value="never" />
                    <el-option label="结束时间" value="until_date" />
                    <el-option label="生成次数" value="after_count" />
                  </el-select>
                </el-form-item>
                <el-form-item
                  v-if="itemDraft.endMode === 'until_date'"
                  label="结束时间"
                  class="todo-form-item-flex"
                >
                  <el-date-picker
                    v-model="itemDraft.endValueDate"
                    type="datetime"
                    value-format="YYYY-MM-DDTHH:mm:ssZ"
                    :disabled-minutes="disabledFiveMinuteMinutes"
                    :disabled-seconds="disabledAllSeconds"
                    style="width: 100%"
                  />
                </el-form-item>
                <el-form-item
                  v-else-if="itemDraft.endMode === 'after_count'"
                  label="生成次数"
                  class="todo-form-item-flex"
                >
                  <el-input-number
                    v-model="itemDraft.endValueCount"
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

        <div class="todo-form-section">
          <el-form-item label="描述">
            <el-input
              v-model="itemDraft.description"
              type="textarea"
              :rows="4"
              placeholder="可补充事项说明"
            />
          </el-form-item>
        </div>
      </el-form>
      <template #footer>
        <el-button @click="itemDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="saveItem">{{ itemDialogSubmitText }}</el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="typeDialogVisible"
      :title="typeDialogTitle"
      width="480px"
      @closed="resetTypeDraft"
    >
      <el-form label-width="72px">
        <el-form-item label="名称"
          ><el-input v-model.trim="typeDraft.name" placeholder="请输入分类名称"
        /></el-form-item>
        <el-form-item label="颜色"
          ><el-input v-model.trim="typeDraft.color" placeholder="例如：#409eff"
        /></el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="typeDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="saveType">保存</el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="assigneeDialogVisible"
      :title="assigneeDialogTitle"
      width="420px"
      @closed="resetAssigneeDraft"
    >
      <el-form label-width="72px">
        <el-form-item label="名称"
          ><el-input v-model.trim="assigneeDraft.name" placeholder="请输入执行人名称"
        /></el-form-item>
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
import { AlarmClock, MoreFilled, Refresh, Setting, Top } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  TodoAssignee,
  TodoEditScope,
  TodoEndMode,
  TodoItem,
  TodoItemUpsertPayload,
  TodoKind,
  TodoPriority,
  TodoRecordRole,
  TodoRecurrence,
  TodoReminderPreset,
  TodoRepeatPreset,
  TodoRule,
  TodoRuleMode,
  TodoSimpleRule,
  TodoStatus,
  TodoType,
} from "../types";
import { groupTodoItemsByBucket } from "../utils/todoBuckets";
import {
  TODO_REPEAT_PRESET_OPTIONS,
  TODO_WEEKDAY_OPTIONS,
  buildSimpleRuleFromPreset,
  combineLocalDateTime,
  deriveRepeatPreset,
  getCreateDraftDefaultDateTime,
  getTodayDateString,
  isFiveMinuteDateTime,
  isFiveMinuteTime,
  normalizeEndMode,
  splitDateTime,
} from "../utils/todoSchedule";

type SelectTypeValue = number | string | undefined;
type SelectAssigneeValue = number | string;
type ItemDialogMode = "create" | "edit_item";

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

const items = ref<TodoItem[]>([]);
const types = ref<TodoType[]>([]);
const assignees = ref<TodoAssignee[]>([]);
const itemKeyword = ref("");
const filterType = ref<string | null>(null);
const filterPriority = ref<TodoPriority | null>(null);
const doneCollapsed = ref(true);
const basicsDialogVisible = ref(false);
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
  { label: "不提醒", value: "none" },
  { label: "准时提醒", value: "0m" },
  { label: "提前五分钟", value: "5m" },
  { label: "提前十分钟", value: "10m" },
  { label: "提前半个小时", value: "30m" },
  { label: "提前一个小时", value: "1h" },
  { label: "提前一天", value: "1d" },
  { label: "提前两天", value: "2d" },
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

function pad2(value: number) {
  return String(value).padStart(2, "0");
}

function splitDraftEventTime(value: string) {
  if (!value.trim()) return { hour: "", minute: "" };
  const [hourText = "", minuteText = ""] = value.split(":");
  const hour = Number(hourText);
  const minute = Number(minuteText);
  return {
    hour: Number.isInteger(hour) && hour >= 0 && hour <= 23 ? pad2(hour) : "",
    minute:
      Number.isInteger(minute) && minute >= 0 && minute <= 55 && minute % 5 === 0
        ? pad2(minute)
        : "",
  };
}

function composeDraftEventTime(hour: string, minute: string) {
  if (!hour && !minute) return "";
  return `${hour || "00"}:${minute || "00"}`;
}

const hourOptions = Array.from({ length: 24 }, (_item, index) => {
  const value = pad2(index);
  return { label: value, value };
});

const minuteOptions = Array.from({ length: 12 }, (_item, index) => {
  const value = pad2(index * 5);
  return { label: value, value };
});

const repeatPresetOptions = TODO_REPEAT_PRESET_OPTIONS;
const weekdayOptions = TODO_WEEKDAY_OPTIONS;
const priorityOptions = [
  { value: 'P0', label: 'P0 - 紧急' },
  { value: 'P1', label: 'P1 - 高' },
  { value: 'P2', label: 'P2 - 中' },
  { value: 'P3', label: 'P3 - 低' },
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
  eventDate: initialCreateSchedule.date,
  eventTime: initialCreateSchedule.time,
  reminderPresets: [...defaultReminderPresets] as TodoReminderPreset[],
  scope: "this_instance" as TodoEditScope,
  repeatPreset: "none" as TodoRepeatPreset,
  ruleMode: "simple" as TodoRuleMode,
  timezone: "local",
  cronExpression: "0 0 9 * * 1-5",
  endMode: "never" as TodoEndMode,
  endValueDate: "",
  endValueCount: 1,
  simple: {
    frequency: "daily" as TodoSimpleRule["frequency"],
    interval: 1,
    time: initialCreateSchedule.time,
    weekdays: [1, 2, 3, 4, 5] as number[],
    dayOfMonth: 1,
  },
});

const typeDraft = reactive<TodoTypeDraft>({ id: 0, name: "", color: "", sortOrder: 0 });
const assigneeDraft = reactive<TodoAssigneeDraft>({ id: 0, name: "" });

const isRepeating = computed(() => itemDraft.repeatPreset !== "none");

const rootMap = computed(() => {
  const map = new Map<number, TodoItem>();
  for (const item of items.value) {
    if (isRootItem(item)) map.set(item.id, item);
  }
  return map;
});

const filteredItems = computed(() => {
  const keyword = itemKeyword.value.trim().toLowerCase();
  return items.value.filter((item) => {
    if (isRootItem(item)) return false;
    if (!keyword) return true;
    return (
      item.title.toLowerCase().includes(keyword) || item.description.toLowerCase().includes(keyword)
    );
  });
});

const sortedTypes = computed(() => {
  const typeCounts = new Map<number, number>();
  for (const item of items.value) {
    if (typeof item.typeId !== "number") continue;
    typeCounts.set(item.typeId, (typeCounts.get(item.typeId) || 0) + 1);
  }
  return types.value
    .map((item, index) => ({ item, index, count: typeCounts.get(item.id) || 0 }))
    .sort((left, right) => right.count - left.count || left.index - right.index)
    .map(({ item }) => item);
});

const bucketedItems = computed(() => groupTodoItemsByBucket(filteredItems.value));
const activeItems = computed(() => bucketedItems.value.activeItems);
const doneItems = computed(() => bucketedItems.value.doneItems);

const hasActiveFilter = computed(() => filterType.value !== null || filterPriority.value !== null);

function applyDisplayFilter(list: TodoItem[]): TodoItem[] {
  let result = list;
  if (filterType.value !== null) {
    const currentType = filterType.value;
    result = result.filter((item) => (item.typeName || "未分类") === currentType);
  }
  if (filterPriority.value !== null) {
    const currentPriority = filterPriority.value;
    result = result.filter((item) => item.priority === currentPriority);
  }
  return result;
}

const displayActiveItems = computed(() => applyDisplayFilter(activeItems.value));
const displayDoneItems = computed(() => applyDisplayFilter(doneItems.value));

const todayDueCount = computed(() => {
  const today = getTodayDateString();
  return activeItems.value.filter((item) => {
    const time = itemScheduleAt(item);
    return time && time.startsWith(today);
  }).length;
});

const overdueCount = computed(() => {
  return activeItems.value.filter((item) => isItemOverdue(item)).length;
});

const typeDistribution = computed(() => {
  const map = new Map<string, { name: string; color: string; count: number }>();
  for (const item of activeItems.value) {
    const name = item.typeName || "未分类";
    const existing = map.get(name);
    if (existing) {
      existing.count++;
    } else {
      map.set(name, { name, color: item.typeColor || "#909399", count: 1 });
    }
  }
  return [...map.values()].sort((a, b) => b.count - a.count);
});

const priorityDistribution = computed(() => {
  const counts: Record<string, number> = { P0: 0, P1: 0, P2: 0, P3: 0 };
  for (const item of activeItems.value) {
    if (counts[item.priority] !== undefined) counts[item.priority]++;
  }
  return (["P0", "P1", "P2", "P3"] as const)
    .map((p) => ({ priority: p, count: counts[p] }))
    .filter((entry) => entry.count > 0);
});

function statsBarWidth(count: number, list: { count: number }[]) {
  const max = Math.max(...list.map((i) => i.count), 1);
  return Math.round((count / max) * 100) + "%";
}

function toggleTypeFilter(name: string) {
  filterType.value = filterType.value === name ? null : name;
}

function togglePriorityFilter(priority: TodoPriority) {
  filterPriority.value = filterPriority.value === priority ? null : priority;
}

function clearTypeFilter() {
  filterType.value = null;
}

function clearPriorityFilter() {
  filterPriority.value = null;
}

function clearAllFilters() {
  clearTypeFilter();
  clearPriorityFilter();
}

const editingItemIsRecurring = computed(
  () => !!editingItemSnapshot.value && itemKindOf(editingItemSnapshot.value) === "recurring",
);
const showScopeSelector = computed(
  () => itemDialogMode.value === "edit_item" && editingItemIsRecurring.value,
);
const showRecurrenceFields = computed(() => {
  if (!isRepeating.value) return false;
  if (itemDialogMode.value === "create") return true;
  return itemDraft.scope === "future_instances";
});
const showWeeklyWeekdays = computed(
  () =>
    isRepeating.value &&
    (itemDraft.repeatPreset === "weekly" ||
      (itemDraft.repeatPreset === "custom" && itemDraft.simple.frequency === "weekly")),
);
const showMonthlyDayOfMonth = computed(
  () =>
    isRepeating.value &&
    (itemDraft.repeatPreset === "monthly" ||
      (itemDraft.repeatPreset === "custom" && itemDraft.simple.frequency === "monthly")),
);
const showCustomRepeatFields = computed(
  () => isRepeating.value && itemDraft.repeatPreset === "custom",
);
const showCronRepeatFields = computed(() => isRepeating.value && itemDraft.repeatPreset === "cron");
const eventDateModel = computed({
  get: () => itemDraft.eventDate || undefined,
  set: (value: string | null | undefined) => {
    if (!value) {
      clearEventSchedule();
      return;
    }
    itemDraft.eventDate = value;
  },
});
const eventHour = computed({
  get: () => splitDraftEventTime(itemDraft.eventTime).hour,
  set: (value: string) => {
    const { minute } = splitDraftEventTime(itemDraft.eventTime);
    itemDraft.eventTime = composeDraftEventTime(value, minute);
  },
});
const eventMinute = computed({
  get: () => splitDraftEventTime(itemDraft.eventTime).minute,
  set: (value: string) => {
    const { hour } = splitDraftEventTime(itemDraft.eventTime);
    itemDraft.eventTime = composeDraftEventTime(hour, value);
  },
});
const repeatFormTip = computed(() => {
  if (showCronRepeatFields.value)
    return "Cron \u8868\u8fbe\u5f0f\u51b3\u5b9a\u5b9e\u9645\u89e6\u53d1\u65f6\u95f4\uff1b\u65e5\u671f\u53ea\u4f5c\u4e3a\u9996\u6b21\u751f\u6548\u4e0b\u754c\u3002";
  return "\u91cd\u590d\u4e8b\u9879\u4f1a\u4ece\u65e5\u671f\u8d77\u6309\u89c4\u5219\u751f\u6210\u5b9e\u4f8b\uff1b\u9009\u62e9\u201c\u6b64\u540e\u672a\u53d1\u751f\u9879\u201d\u65f6\uff0c\u4fdd\u5b58\u7684\u662f\u91cd\u590d\u89c4\u5219\u3002";
});
const itemDialogTitle = computed(() => {
  if (itemDialogMode.value === "create") return "新增事项";
  return "编辑事项";
});
const itemDialogSubmitText = computed(() =>
  itemDialogMode.value === "create" ? "创建事项" : "保存",
);
const typeDialogTitle = computed(() => (typeDraft.id ? "编辑分类" : "新增分类"));
const assigneeDialogTitle = computed(() => (assigneeDraft.id ? "编辑执行人" : "新增执行人"));

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

function isActionableStatus(status: TodoStatus | null) {
  return status === "pending" || status === "in_progress";
}

function priorityTagType(priority: TodoPriority) {
  return ({ P0: "danger", P1: "warning", P2: "primary", P3: "info" }[priority] || "info") as
    | "danger"
    | "warning"
    | "primary"
    | "info";
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

function hasRepeatRule(item: TodoItem): boolean {
  if (item.kind !== "recurring") return false;
  if (item.recurrence) return true;
  const root = rootMap.value.get(item.rootId);
  return !!root?.recurrence;
}

function getItemRecurrence(item: TodoItem): TodoRecurrence | null {
  if (item.recurrence) return item.recurrence;
  const root = rootMap.value.get(item.rootId);
  return root?.recurrence ?? null;
}

function isDoneItem(item: TodoItem) {
  return item.status === "completed" || item.status === "canceled";
}

function canPinItem(item: TodoItem) {
  return isActionableStatus(item.status);
}

function canCancelItem(item: TodoItem) {
  return isActionableStatus(item.status);
}

function truncateDescription(desc: string, maxLen = 40): string {
  if (desc.length <= maxLen) return desc;
  return desc.slice(0, maxLen) + "...";
}

function itemScheduleAt(item: TodoItem) {
  return isRootItem(item) ? item.displayAt : item.eventAt;
}

function isItemOverdue(item: TodoItem): boolean {
  const time = itemScheduleAt(item);
  if (!time || !isActionableStatus(item.status)) return false;
  return new Date(time).getTime() < Date.now();
}

function todoRowClassName({ row }: { row: TodoItem }) {
  return 'todo-row-' + row.priority.toLowerCase();
}

function doneRowClassName({ row }: { row: TodoItem }) {
  return 'todo-row-' + row.priority.toLowerCase() + ' is-done-row';
}

function itemTimeLabel(item: TodoItem) {
  return formatDate(itemScheduleAt(item));
}

function toggleDoneCollapsed() {
  doneCollapsed.value = !doneCollapsed.value;
}

function handleRowAction(command: string, row: TodoItem) {
  switch (command) {
    case "pin":
      toggleItemPin(row.id);
      break;
    case "cancel":
      changeItemStatus(row.id, "canceled");
      break;
    case "delete":
      deleteItem(row);
      break;
  }
}

function onCheckItem(item: TodoItem) {
  if (!item.status) return;
  void changeItemStatus(item.id, isDoneItem(item) ? "pending" : "completed");
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
  const order: TodoReminderPreset[] = ["none", "0m", "5m", "10m", "30m", "1h", "1d", "2d"];
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
  return ["pending", "in_progress", "completed", "canceled"].includes(value)
    ? (value as TodoStatus)
    : "pending";
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
  const matched = Object.entries(reminderPresetToMinutesMap).find(
    ([, value]) => value === minutes,
  )?.[0];
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

function deriveReminderPresets(
  record: Record<string, unknown>,
  eventAt?: string | null,
): TodoReminderPreset[] {
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
  const offsetMinutes = readNullableNumber(record, [
    "reminderOffsetMinutes",
    "reminderMinutes",
    "offsetMinutes",
  ]);
  if (offsetMinutes != null) {
    const preset = reminderPresetFromMinutes(offsetMinutes);
    return preset === "none" ? [] : [preset];
  }
  const remindAt = readNullableString(record, ["remindAt", "reminderAt"]);
  if (eventAt && remindAt) {
    const eventDate = new Date(eventAt);
    const remindDate = new Date(remindAt);
    if (!Number.isNaN(eventDate.getTime()) && !Number.isNaN(remindDate.getTime())) {
      const preset = reminderPresetFromMinutes(
        Math.round((eventDate.getTime() - remindDate.getTime()) / 60000),
      );
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

function normalizeRule(
  rawRule: unknown,
  ruleMode: TodoRuleMode,
  fallbackCronExpression = "",
): TodoRule {
  if (ruleMode === "cron") {
    const expressionRecord = asRecord(rawRule);
    const expression =
      typeof rawRule === "string"
        ? rawRule
        : readString(expressionRecord, ["expression", "cronExpression"], fallbackCronExpression);
    return { expression };
  }

  const source =
    typeof rawRule === "string"
      ? (() => {
          try {
            return asRecord(JSON.parse(rawRule));
          } catch {
            return {};
          }
        })()
      : asRecord(rawRule);

  const frequency = (["daily", "weekly", "monthly"] as const).includes(
    readString(source, ["frequency"], "daily") as "daily" | "weekly" | "monthly",
  )
    ? (readString(source, ["frequency"], "daily") as "daily" | "weekly" | "monthly")
    : "daily";
  const interval = Math.max(1, readNumber(source, ["interval"], 1));
  const time = readString(source, ["time"], "09:00");
  const weekdays = readArray(source, ["weekdays"])
    .map((item) => Number(item))
    .filter((day) => Number.isInteger(day) && day >= 1 && day <= 7);
  const dayOfMonth = Math.min(31, Math.max(1, readNumber(source, ["dayOfMonth"], 1)));
  if (frequency === "weekly")
    return {
      frequency,
      interval,
      time,
      weekdays: weekdays.length > 0 ? weekdays : [1, 2, 3, 4, 5],
    };
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
  const hasRecurrence =
    kind === "recurring" &&
    ("ruleMode" in recurrenceSource ||
      "rule" in recurrenceSource ||
      "cronExpression" in recurrenceSource ||
      "nextOccurrenceAt" in recurrenceSource ||
      recordRole === "root");
  const recurrenceRuleMode = normalizeRuleMode(
    readString(recurrenceRecord, ["ruleMode"], "simple"),
  );
  const recurrenceCronExpression = readString(recurrenceRecord, [
    "cronExpression",
    "cron",
    "expression",
  ]);
  const recurrence = hasRecurrence
    ? ({
        startAt: readNullableString(recurrenceRecord, ["startAt", "start_at", "firstOccurrenceAt"]),
        ruleMode: recurrenceRuleMode,
        rule: normalizeRule(
          readUnknown(recurrenceRecord, ["rule", "ruleJson", "schedule"]),
          recurrenceRuleMode,
          recurrenceCronExpression,
        ),
        cronExpression: recurrenceCronExpression,
        timezone: readString(recurrenceRecord, ["timezone", "tz"], "local"),
        endMode: normalizeEndMode(readString(recurrenceRecord, ["endMode"], "never")),
        endValue: readUnknown(recurrenceRecord, ["endValue", "until", "count"]) as
          | string
          | number
          | null,
        nextOccurrenceAt: readNullableString(recurrenceRecord, [
          "nextOccurrenceAt",
          "nextEventAt",
          "nextRunAt",
        ]),
        generatedCount: readNumber(recurrenceRecord, ["generatedCount", "generated"], 0),
        active: readBoolean(recurrenceRecord, ["active", "enabled"], true),
      } satisfies TodoRecurrence)
    : null;
  const rootId =
    readNullableNumber(record, ["rootId", "seriesId", "templateId", "sourceTemplateId"]) ??
    readNumber(record, ["id", "taskId"]);
  const rawStatus = readUnknown(record, ["status"]);
  return {
    id: readNumber(record, ["id", "taskId"]),
    rootId,
    kind,
    recordRole,
    pinned: readBoolean(record, ["pinned"]),
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
    displayAt: readNullableString(record, [
      "displayAt",
      "eventAt",
      "eventTime",
      "dueAt",
      "remindAt",
      "nextOccurrenceAt",
    ]),
    assignees: normalizeAssignees(readUnknown(record, ["assignees", "owners", "members"])),
    isOverdue: readBoolean(record, ["isOverdue"]),
    recurrence,
    canEditFuture: readBoolean(
      record,
      ["canEditFuture"],
      kind === "recurring" && recordRole === "occurrence",
    ),
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
    .map((assignee) =>
      typeof assignee.id === "number" && assignee.id > 0 ? assignee.id : assignee.name,
    )
    .filter(
      (value): value is SelectAssigneeValue =>
        (typeof value === "number" && value > 0) ||
        (typeof value === "string" && value.trim().length > 0),
    );
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
    startDate: itemDraft.eventDate,
    time: itemDraft.eventTime,
    currentRule: {
      frequency: itemDraft.simple.frequency,
      interval: Math.max(1, Number(itemDraft.simple.interval || 1)),
      time: itemDraft.eventTime || itemDraft.simple.time || "09:00",
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

function buildEventAt() {
  return combineLocalDateTime(itemDraft.eventDate, itemDraft.eventTime);
}

function syncSimpleDraftFromRule(rule: TodoRule) {
  if (!("frequency" in rule)) return;
  itemDraft.simple.frequency = rule.frequency;
  itemDraft.simple.interval = Math.max(1, Number(rule.interval || 1));
  itemDraft.simple.time = rule.time || "09:00";
  itemDraft.eventTime = rule.time || itemDraft.eventTime || "09:00";
  itemDraft.simple.weekdays =
    Array.isArray(rule.weekdays) && rule.weekdays.length > 0 ? [...rule.weekdays] : [1, 2, 3, 4, 5];
  itemDraft.simple.dayOfMonth = Math.min(31, Math.max(1, Number(rule.dayOfMonth || 1)));
}

function applyRepeatPresetRule(preset: TodoRepeatPreset) {
  itemDraft.repeatPreset = preset;
  if (preset === "cron") {
    itemDraft.ruleMode = "cron";
    if (!itemDraft.eventDate) itemDraft.eventDate = getCreateDraftDefaultDateTime().date;
    return;
  }
  itemDraft.ruleMode = "simple";
  const nextRule = buildSimpleRuleFromPreset({
    preset,
    startDate: itemDraft.eventDate,
    time: itemDraft.eventTime,
    currentRule: {
      frequency: itemDraft.simple.frequency,
      interval: itemDraft.simple.interval,
      time: itemDraft.eventTime,
      weekdays: itemDraft.simple.weekdays,
      dayOfMonth: itemDraft.simple.dayOfMonth,
    },
  });
  syncSimpleDraftFromRule(nextRule);
}

async function onRepeatPresetChange(nextPreset: TodoRepeatPreset) {
  if (nextPreset === "none") {
    if (itemDialogMode.value === "edit_item" && editingItemIsRecurring.value) {
      try {
        await ElMessageBox.confirm(
          "将此重复事项改为不重复，后续将不再自动生成实例。确认吗？",
          "取消重复",
          { type: "warning" },
        );
      } catch {
        itemDraft.repeatPreset = deriveRepeatPreset(editingRootSnapshot.value?.recurrence || null);
        return;
      }
    }
    itemDraft.repeatPreset = "none";
    itemDraft.ruleMode = "simple";
    return;
  }
  applyRepeatPresetRule(nextPreset);
}

function onCustomFrequencyChange() {
  if (itemDraft.repeatPreset !== "custom") return;
  applyRepeatPresetRule("custom");
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

function resetReminderPresetsToNone() {
  itemDraft.reminderPresets = ["none"];
  lastReminderPresetSelection.value = ["none"];
}

function clearEventSchedule() {
  itemDraft.eventDate = "";
  itemDraft.eventTime = "";
  resetReminderPresetsToNone();
}

function resetItemDraft() {
  const defaultSchedule = getCreateDraftDefaultDateTime();
  itemDraft.id = 0;
  itemDraft.rootId = 0;
  itemDraft.title = "";
  itemDraft.typeId = undefined;
  itemDraft.priority = "P2";
  itemDraft.description = "";
  itemDraft.assigneeIds = [];
  itemDraft.eventDate = defaultSchedule.date;
  itemDraft.eventTime = defaultSchedule.time;
  itemDraft.reminderPresets = [...defaultReminderPresets];
  itemDraft.scope = "this_instance";
  itemDraft.repeatPreset = "none";
  itemDraft.ruleMode = "simple";
  itemDraft.timezone = "local";
  itemDraft.cronExpression = "0 0 9 * * 1-5";
  itemDraft.endMode = "never";
  itemDraft.endValueDate = "";
  itemDraft.endValueCount = 1;
  itemDraft.simple.frequency = "daily";
  itemDraft.simple.interval = 1;
  itemDraft.simple.time = defaultSchedule.time;
  itemDraft.simple.weekdays = [1, 2, 3, 4, 5];
  itemDraft.simple.dayOfMonth = 1;
  lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
  editingItemSnapshot.value = null;
  editingRootSnapshot.value = null;
  itemDialogMode.value = "create";
}

function applyItemToDraft(item: TodoItem) {
  const { date, time } = splitDateTime(item.eventAt, "");
  itemDraft.id = item.id;
  itemDraft.rootId = getRootItemId(item);
  itemDraft.title = item.title;
  itemDraft.typeId = item.typeId ?? undefined;
  itemDraft.priority = item.priority;
  itemDraft.description = item.description;
  itemDraft.assigneeIds = toDraftAssigneeValues(item.assignees);
  itemDraft.eventDate = date;
  itemDraft.eventTime = time;
  itemDraft.reminderPresets = toDraftReminderPresets(item.reminderPresets);
  lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
  itemDraft.repeatPreset =
    itemKindOf(item) === "recurring" ? deriveRepeatPreset(getItemRecurrence(item)) : "none";
  itemDraft.scope = "this_instance";
}

function applyRootItemToDraft(item: TodoItem) {
  const recurrence = item.recurrence;
  const { date, time } = splitDateTime(
    recurrence?.startAt || recurrence?.nextOccurrenceAt || item.displayAt,
    "",
  );
  itemDraft.rootId = getRootItemId(item);
  itemDraft.title = item.title;
  itemDraft.typeId = item.typeId ?? undefined;
  itemDraft.priority = item.priority;
  itemDraft.description = item.description;
  itemDraft.assigneeIds = toDraftAssigneeValues(item.assignees);
  itemDraft.reminderPresets = toDraftReminderPresets(item.reminderPresets);
  lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
  itemDraft.eventDate = date || getTodayDateString();
  itemDraft.eventTime = time;
  itemDraft.ruleMode = recurrence?.ruleMode || "simple";
  itemDraft.timezone = recurrence?.timezone || "local";
  itemDraft.cronExpression =
    (recurrence?.rule as { expression?: string } | undefined)?.expression ||
    recurrence?.cronExpression ||
    "0 0 9 * * 1-5";
  itemDraft.endMode = recurrence?.endMode || "never";
  itemDraft.endValueDate =
    itemDraft.endMode === "until_date" ? String(recurrence?.endValue || "") : "";
  itemDraft.endValueCount =
    itemDraft.endMode === "after_count" ? Number(recurrence?.endValue || 1) : 1;
  itemDraft.repeatPreset = deriveRepeatPreset(recurrence);
  if (itemDraft.ruleMode === "simple")
    syncSimpleDraftFromRule((recurrence?.rule || {}) as TodoRule);
}

async function loadTypes() {
  types.value =
    ((await invokeToolByChannel("tool:todo:type-list", {})) as { items: TodoType[] }).items || [];
}
async function loadAssignees() {
  assignees.value =
    ((await invokeToolByChannel("tool:todo:assignee-list", {})) as { items: TodoAssignee[] })
      .items || [];
}
async function loadItems() {
  items.value = getResponseItems(await invokeToolByChannel("tool:todo:item-list", {})).map(
    normalizeTodoItem,
  );
}

function openCreateItemDialog() {
  resetItemDraft();
  itemDialogMode.value = "create";
  itemDialogVisible.value = true;
}

async function resolveTypeId(value: SelectTypeValue) {
  if (typeof value === "number") return value;
  const name = typeof value === "string" ? value.trim() : "";
  if (!name) return null;
  const existed = types.value.find((item) => normalizeName(item.name) === normalizeName(name));
  if (existed) return existed.id;
  const result = (await invokeToolByChannel("tool:todo:type-upsert", {
    name,
    sortOrder: getNextTypeSortOrder(),
  })) as { id?: number };
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
    const existed = assignees.value.find(
      (item) => normalizeName(item.name) === normalizeName(name),
    );
    if (existed) {
      ids.add(existed.id);
      continue;
    }
    const result = (await invokeToolByChannel("tool:todo:assignee-upsert", { name })) as {
      id?: number;
    };
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
  const targetRoot =
    editingRootSnapshot.value || getRootItemById(getRootItemId(editingItemSnapshot.value));
  if (!targetRoot) {
    ElMessage.warning("当前事项没有可编辑的重复事项根记录");
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
  const eventAt = buildEventAt();
  const selectedReminderPresets = effectiveReminderPresets(itemDraft.reminderPresets);
  const hasEventDate = !!itemDraft.eventDate.trim();
  const hasEventTime = !!itemDraft.eventTime.trim();
  if (!title) {
    ElMessage.warning("请输入事项标题");
    return;
  }
  if (hasEventDate !== hasEventTime) {
    ElMessage.warning("日期和时间需要同时填写或同时清空");
    return;
  }
  if (hasEventTime && !isFiveMinuteTime(itemDraft.eventTime)) {
    ElMessage.warning("事件时间仅支持5分钟刻度");
    return;
  }
  if (selectedReminderPresets.length > 0 && !eventAt) {
    ElMessage.warning("请先填写日期和时间，再设置提醒方式");
    return;
  }
  if (isRepeating.value && showRecurrenceFields.value) {
    if (!hasEventDate || !hasEventTime) {
      ElMessage.warning("重复事项需要同时填写日期和时间");
      return;
    }
    if (!isFiveMinuteTime(itemDraft.eventTime)) {
      ElMessage.warning("时间仅支持5分钟刻度");
      return;
    }
    if (!eventAt) {
      ElMessage.warning("日期或时间格式不正确");
      return;
    }
    if (showCronRepeatFields.value && !itemDraft.cronExpression.trim()) {
      ElMessage.warning("请输入 Cron 表达式");
      return;
    }
    if (
      showCustomRepeatFields.value &&
      itemDraft.simple.frequency === "weekly" &&
      Number(itemDraft.simple.interval || 1) > 1
    ) {
      ElMessage.warning("按周自定义暂不支持大于 1 的间隔，请改用高级 Cron");
      return;
    }
    if (
      itemDraft.endMode === "until_date" &&
      itemDraft.endValueDate &&
      !isFiveMinuteDateTime(itemDraft.endValueDate)
    ) {
      ElMessage.warning("结束时间仅支持5分钟刻度");
      return;
    }
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

    const kind: TodoKind = isRepeating.value ? "recurring" : "one_off";
    const payload: TodoItemUpsertPayload & Record<string, unknown> = {
      ...commonPayload,
      kind,
    };

    if (!isRepeating.value) {
      payload.eventAt = eventAt;
      payload.eventTime = payload.eventAt;
      payload.dueAt = payload.eventAt;
      payload.remindAt = computeLegacyRemindAt(eventAt, selectedReminderPresets);
    }

    if (isRepeating.value && showRecurrenceFields.value) {
      payload.recurrence = {
        startAt: eventAt,
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
      payload.id = itemDraft.id;
      payload.scope = itemDraft.scope;
      if (itemDraft.rootId) payload.rootId = itemDraft.rootId;
      if (itemDraft.scope === "future_instances") payload.recordRole = "root";
      await invokeToolByChannel("tool:todo:item-update", payload);
    }

    itemDialogVisible.value = false;
    resetItemDraft();
    await loadItems();
    ElMessage.success("保存成功");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function changeItemStatus(id: number, status: TodoStatus) {
  try {
    await invokeToolByChannel("tool:todo:item-change-status", { id, status });
    await loadItems();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function toggleItemPin(id: number) {
  try {
    await invokeToolByChannel("tool:todo:item-toggle-pin", { id });
    await loadItems();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function snoozeItem(id: number, taskReminderId?: number | null) {
  try {
    await invokeToolByChannel("tool:todo:item-snooze", { id, taskReminderId, minutes: 10 });
    await loadItems();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function deleteItem(item: TodoItem) {
  try {
    const message = isRootItem(item)
      ? "确认删除该重复事项吗？删除后将停止后续生成。"
      : item.kind === "recurring"
        ? "确认删除当前事项吗？不会停止后续重复。"
        : "确认删除该事项吗？";
    await ElMessageBox.confirm(message, "删除确认", { type: "warning" });
    await invokeToolByChannel("tool:todo:item-delete", {
      id: item.id,
      kind: item.kind,
      recordRole: item.recordRole,
      rootId: getRootItemId(item),
    });
    await loadItems();
  } catch (error) {
    if ((error as Error).message !== "cancel") ElMessage.error((error as Error).message);
  }
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
    await invokeToolByChannel(
      "tool:todo:type-upsert",
      typeDraft.id
        ? { id: typeDraft.id, name, color: typeDraft.color, sortOrder: typeDraft.sortOrder }
        : {
            name,
            color: typeDraft.color,
            sortOrder: typeDraft.sortOrder || getNextTypeSortOrder(),
          },
    );
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
    await invokeToolByChannel(
      "tool:todo:assignee-upsert",
      assigneeDraft.id ? { id: assigneeDraft.id, name } : { name },
    );
    assigneeDialogVisible.value = false;
    resetAssigneeDraft();
    await Promise.all([loadAssignees(), loadItems()]);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
async function removeAssignee(item: TodoAssignee) {
  try {
    await ElMessageBox.confirm(`确认删除执行人「${item.name}」吗？`, "删除确认", {
      type: "warning",
    });
    await invokeToolByChannel("tool:todo:assignee-delete", { id: item.id });
    await Promise.all([loadAssignees(), loadItems()]);
  } catch (error) {
    if ((error as Error).message !== "cancel") ElMessage.error((error as Error).message);
  }
}

onMounted(async () => {
  await Promise.all([loadTypes(), loadAssignees(), loadItems()]);
  try {
    reminderUnlisten = await listen("todo-reminder-fired", async () => {
      await loadItems();
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
.todo-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
  justify-content: flex-end;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.toolbar-right {
  display: flex;
  gap: 10px;
  align-items: center;
}
.toolbar-settings-btn {
  width: 32px;
  height: 32px;
  padding: 0;
  border-radius: 999px;
  color: var(--el-text-color-secondary);
}
.toolbar-settings-btn:hover {
  color: var(--el-color-primary);
  background: var(--el-fill-color-light);
}
.item-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 12px;
}
.item-section:last-child {
  margin-bottom: 0;
}
.item-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.item-section-title-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}
.item-section-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}
.count-tag {
  border-radius: 999px;
  padding-inline: 8px;
}
.item-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}
.item-title.is-done {
  text-decoration: line-through;
  color: var(--el-text-color-secondary);
}
.title-cell {
  display: flex;
  align-items: center;
  width: 100%;
}
.title-left {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  flex: 1;
  gap: 6px;
}
.title-right {
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  margin-left: 8px;
  gap: 6px;
}
.item-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 20px;
  padding: 0 7px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 500;
  line-height: 1;
  vertical-align: middle;
  white-space: nowrap;
}
.badge-pinned {
  color: var(--lc-success);
  background: rgba(52, 211, 153, 0.10);
}
.badge-overdue {
  color: var(--lc-danger);
  background: rgba(248, 113, 113, 0.10);
}
.badge-repeat {
  color: var(--lc-warning);
  background: rgba(251, 191, 36, 0.10);
}
.todo-table {
  width: 100%;
}
.todo-table :deep(.el-table__row) {
  position: relative;
}
.todo-table :deep(.el-table__row td:first-child .cell) {
  padding-left: 12px;
}
.todo-table :deep(.todo-row-p0 td:first-child) {
  border-left: 3px solid var(--lc-danger);
}
.todo-table :deep(.todo-row-p1 td:first-child) {
  border-left: 3px solid var(--lc-warning);
}
.todo-table :deep(.todo-row-p2 td:first-child) {
  border-left: 3px solid var(--lc-accent);
}
.todo-table :deep(.todo-row-p3 td:first-child) {
  border-left: 3px solid var(--lc-text-muted);
}
.done-table {
  opacity: 0.7;
}
.table-actions {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
}
.table-actions :deep(.el-button--primary) {
  color: var(--el-color-primary);
}
.table-actions :deep(.el-button--danger) {
  color: var(--el-color-danger);
}
.item-more-btn {
  padding: 4px;
  color: var(--el-text-color-secondary);
}
.done-section-header {
  cursor: pointer;
  user-select: none;
}
.done-toggle-icon {
  color: var(--el-text-color-secondary);
  display: inline-flex;
  align-items: center;
  font-size: 14px;
  line-height: 1;
  transition: transform 0.2s ease;
}
.done-toggle-icon.is-collapsed {
  transform: rotate(-90deg);
}
.basic-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(280px, 1fr));
  gap: 12px;
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.todo-form-section {
  margin-bottom: 20px;
}
.todo-form-section:last-child {
  margin-bottom: 0;
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
  margin-bottom: 18px;
}
.todo-form-item-flex {
  flex: 1;
}
.todo-form-item-date {
  width: 220px;
  flex-shrink: 0;
}
.todo-form-item-time {
  width: 240px;
  flex-shrink: 0;
}
.time-picker-inline {
  display: flex;
  align-items: center;
  gap: 8px;
}
.time-picker-fused {
  display: flex;
  align-items: center;
  border: 1px solid var(--lc-border);
  border-radius: var(--el-border-radius-base);
  overflow: hidden;
  transition: border-color 0.2s;
  min-width: 160px;
}
.time-picker-fused:hover {
  border-color: var(--el-color-primary-light-3);
}
.time-picker-fused:focus-within {
  border-color: var(--el-color-primary);
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
.priority-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
  vertical-align: middle;
  flex-shrink: 0;
}
.priority-p0 {
  background-color: var(--lc-danger);
}
.priority-p1 {
  background-color: var(--lc-warning);
}
.priority-p2 {
  background-color: var(--lc-accent);
}
.priority-p3 {
  background-color: var(--lc-text-muted);
}
.repeat-detail-card {
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--el-border-radius-base);
  padding: 16px;
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
  min-width: 88px;
}
.color-dot {
  display: inline-block;
  width: 10px;
  height: 10px;
  margin-right: 6px;
  border-radius: 50%;
  vertical-align: middle;
}
.todo-layout {
  display: flex;
  gap: 24px;
  flex: 1;
  min-height: 0;
}
.todo-main {
  flex: 1;
  min-width: 0;
}
.todo-stats {
  width: 280px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 20px;
}
.stats-section {
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 16px;
}
.stats-section-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  margin-bottom: 12px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.stats-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 12px;
}
.stats-section-header .stats-section-title {
  margin-bottom: 0;
}
.stats-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}
.stat-card {
  text-align: center;
  padding: 10px 8px;
  background: var(--el-fill-color-lighter);
  border-radius: 6px;
}
.stat-number {
  font-size: 22px;
  font-weight: 700;
  color: var(--el-text-color-primary);
  line-height: 1.2;
}
.stat-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 2px;
}
.stat-card.is-alert .stat-number {
  color: var(--el-color-danger);
}
.stats-bar-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.stats-bar-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.stats-bar-item.is-clickable {
  cursor: pointer;
  border-radius: 6px;
  padding: 4px 8px;
  margin: -4px -8px;
  transition: background-color 0.15s ease;
}
.stats-bar-item.is-clickable:hover {
  background-color: var(--el-fill-color-light);
}
.stats-bar-item.is-active {
  background-color: var(--el-color-primary-light-9);
  border-left: 3px solid var(--el-color-primary);
  padding-left: 5px;
}
.stats-bar-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  color: var(--el-text-color-primary);
}
.filter-indicator {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  margin-bottom: 12px;
  font-size: 13px;
  background: var(--el-color-primary-light-9);
  border: 1px solid var(--el-color-primary-light-7);
  border-radius: 6px;
}
.filter-indicator-text {
  color: var(--el-text-color-regular);
}
.stats-bar-count {
  margin-left: auto;
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
}
.stats-bar-track {
  height: 6px;
  background: var(--el-fill-color-lighter);
  border-radius: 3px;
  overflow: hidden;
}
.stats-bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.3s ease;
}
.priority-bar-p0 {
  background-color: var(--lc-danger);
}
.priority-bar-p1 {
  background-color: var(--lc-warning);
}
.priority-bar-p2 {
  background-color: var(--lc-accent);
}
.priority-bar-p3 {
  background-color: var(--lc-text-muted);
}
@media (max-width: 900px) {
  .todo-layout {
    flex-direction: column;
  }
  .todo-stats {
    width: 100%;
    flex-direction: row;
    flex-wrap: wrap;
  }
  .stats-section {
    flex: 1;
    min-width: 200px;
  }
}
@media (max-width: 640px) {
  .basic-grid {
    grid-template-columns: 1fr;
  }
  .todo-form-row {
    flex-direction: column;
    gap: 0;
  }
}
</style>

<style>
.todo-item-dialog .el-dialog {
  border-radius: var(--lc-radius-lg);
  background: var(--lc-surface-0);
}
.todo-item-dialog .el-dialog__header {
  margin-right: 0;
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--lc-border);
}
.todo-item-dialog .el-dialog__title {
  font-family: var(--lc-font-display);
  font-size: 18px;
  font-weight: 600;
  color: var(--lc-text);
}
.todo-item-dialog .el-dialog__body {
  padding: 20px 24px;
}
.todo-item-dialog .el-dialog__footer {
  padding: 16px 24px 20px;
  border-top: 1px solid var(--lc-border);
}
</style>
