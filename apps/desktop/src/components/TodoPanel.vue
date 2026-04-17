<template>
  <div class="todo-panel">
    <div class="todo-layout">
      <aside class="todo-stats todo-sidebar">
        <div class="stats-section">
          <div class="stats-section-header">
            <div class="stats-section-title">概览</div>
            <el-button
              size="small"
              link
              type="primary"
              class="overview-settings-btn"
              title="基础数据设置"
              aria-label="基础数据设置"
              @click="basicsDialogVisible = true"
            >
              <el-icon><Setting /></el-icon>
              <span>基础数据</span>
            </el-button>
          </div>
          <div class="stats-grid">
            <div class="stat-card">
              <div class="stat-number">{{ activeItems.length }}</div>
              <div class="stat-label">任务</div>
            </div>
            <div class="stat-card">
              <div class="stat-number">{{ doneItems.length + recentWeekItems.length }}</div>
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
            <el-button
              size="small"
              link
              type="primary"
              :disabled="filterType === null"
              @click="clearTypeFilter"
            >
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
                  :style="{
                    width: statsBarWidth(entry.count, typeDistribution),
                    backgroundColor: entry.color,
                  }"
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
                    <span v-if="relativeTimeLabel(row)" class="meta-chip meta-time">
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
            @title-enter="onCreateTitleEnter"
            @toggle-more-fields="showMoreFields = !showMoreFields"
            @pm-select-change="handlePmSelectChange"
            @navigate-to-pm="navigateToPmItem"
            @event-date-change="(v) => { if (!v) clearEventSchedule(); else itemDraft.eventDate = v; }"
            @event-hour-change="(v) => { const { minute } = splitDraftEventTime(itemDraft.eventTime); itemDraft.eventTime = composeDraftEventTime(v, minute); }"
            @event-minute-change="(v) => { const { hour } = splitDraftEventTime(itemDraft.eventTime); itemDraft.eventTime = composeDraftEventTime(hour, v); }"
            @fill-quick-date="fillQuickDate"
            @fill-default-date-time="fillDefaultDateTime"
            @clear-event-schedule="clearEventSchedule"
            @reminder-presets-change="onReminderPresetsChange"
            @repeat-preset-change="onRepeatPresetChange"
            @custom-frequency-change="onCustomFrequencyChange"
            @insert-md-syntax="insertMdSyntax"
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
            @edit="enterEditMode"
            @toggle-pin="toggleItemPin"
            @change-status="(id, status) => changeItemStatus(id, status as TodoStatus)"
            @delete="deleteItem"
            @copy-title="copyTitle"
            @open-link="openLink"
            @navigate-to-pm="navigateToPmItem"
          />
        </template>
        <div v-else class="detail-empty-pane">
          <div class="detail-empty-visual">
            <div class="empty-illustration">
              <svg class="empty-svg" viewBox="0 0 200 160" fill="none">
                <!-- 背景装饰圆 -->
                <circle cx="160" cy="40" r="20" fill="var(--lc-accent)" opacity="0.08" />
                <circle cx="30" cy="120" r="15" fill="var(--lc-success)" opacity="0.06" />
                <circle cx="170" cy="130" r="10" fill="var(--lc-warning)" opacity="0.08" />
                <!-- 主文档图形 -->
                <rect
                  x="50"
                  y="20"
                  width="100"
                  height="120"
                  rx="12"
                  fill="var(--lc-surface-1)"
                  stroke="var(--lc-border)"
                  stroke-width="2"
                />
                <rect
                  x="65"
                  y="45"
                  width="70"
                  height="6"
                  rx="3"
                  fill="var(--lc-border)"
                  opacity="0.6"
                />
                <rect
                  x="65"
                  y="60"
                  width="50"
                  height="6"
                  rx="3"
                  fill="var(--lc-border)"
                  opacity="0.4"
                />
                <rect
                  x="65"
                  y="75"
                  width="60"
                  height="6"
                  rx="3"
                  fill="var(--lc-border)"
                  opacity="0.4"
                />
                <rect
                  x="65"
                  y="90"
                  width="40"
                  height="6"
                  rx="3"
                  fill="var(--lc-border)"
                  opacity="0.4"
                />
                <!-- 勾选标记 -->
                <circle
                  cx="140"
                  cy="115"
                  r="22"
                  fill="var(--lc-surface-0)"
                  stroke="var(--lc-accent)"
                  stroke-width="2.5"
                />
                <path
                  d="M130 115L137 122L152 107"
                  stroke="var(--lc-accent)"
                  stroke-width="3"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
                <!-- 小装饰点 -->
                <circle cx="60" cy="30" r="4" fill="var(--lc-danger)" opacity="0.6" />
                <circle cx="75" cy="30" r="4" fill="var(--lc-warning)" opacity="0.6" />
                <circle cx="90" cy="30" r="4" fill="var(--lc-success)" opacity="0.6" />
              </svg>
              <div class="empty-glow"></div>
            </div>
          </div>
          <div class="detail-empty-content">
            <div class="detail-empty-title">选择事项查看详情</div>
            <div class="detail-empty-text">
              在列表中点击任意任务，或快速创建新任务开始管理您的工作。
            </div>
          </div>
          <div class="detail-empty-actions">
            <el-button type="primary" size="large" @click="startCreate">
              <el-icon class="empty-btn-icon"><Plus /></el-icon>
              新建任务
            </el-button>
            <el-button size="large" @click="loadItems">
              <el-icon class="empty-btn-icon"><Refresh /></el-icon>
              刷新列表
            </el-button>
          </div>
          <div class="detail-empty-divider">
            <span>今日概览</span>
          </div>
          <div class="detail-empty-stats">
            <div class="detail-empty-stat" :class="{ 'is-active': todayDueCount > 0 }">
              <div class="stat-icon today">
                <el-icon><Calendar /></el-icon>
              </div>
              <div class="stat-info">
                <span class="stat-label">今日到期</span>
                <strong class="stat-value">{{ todayDueCount }}</strong>
              </div>
            </div>
            <div class="detail-empty-stat" :class="{ 'is-alert': overdueCount > 0 }">
              <div class="stat-icon overdue">
                <el-icon><AlarmClock /></el-icon>
              </div>
              <div class="stat-info">
                <span class="stat-label">逾期事项</span>
                <strong class="stat-value">{{ overdueCount }}</strong>
              </div>
            </div>
          </div>
        </div>
      </aside>
    </div>

    <el-dialog
      v-model="basicsDialogVisible"
      title="基础数据设置"
      width="920px"
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
                <el-button size="small" text type="danger" @click="removeType(row)">删除</el-button>
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

    <teleport to="body">
      <div
        v-if="todoContextMenu.visible && todoContextMenuItem"
        ref="todoContextMenuRef"
        class="todo-context-menu"
        :style="{ left: `${todoContextMenu.x}px`, top: `${todoContextMenu.y}px` }"
        role="menu"
        aria-label="任务操作菜单"
        @click.stop
        @contextmenu.prevent.stop
      >
        <button
          type="button"
          class="todo-context-menu-item"
          role="menuitem"
          @click="handleTodoContextMenuCommand('pin')"
        >
          {{ todoContextMenuItem.pinned ? "取消置顶" : "置顶" }}
        </button>
        <button
          type="button"
          class="todo-context-menu-item"
          role="menuitem"
          @click="handleTodoContextMenuCommand('complete')"
        >
          完成
        </button>
        <button
          type="button"
          class="todo-context-menu-item"
          role="menuitem"
          @click="handleTodoContextMenuCommand('edit-time')"
        >
          编辑任务时间
        </button>
        <div class="todo-context-menu-divider" />
        <button
          type="button"
          class="todo-context-menu-item is-danger"
          role="menuitem"
          @click="handleTodoContextMenuCommand('delete')"
        >
          删除
        </button>
      </div>
    </teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, h, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  AlarmClock,
  Calendar,
  Document,
  Grid,
  Plus,
  Refresh,
  Setting,
  Top,
  User,
} from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import {
  useClipboardSuggestion,
  type PendingToolInput,
} from "../composables/useClipboardSuggestion";
import type {
  TodoAssignee,
  TodoEndMode,
  TodoItem,
  TodoItemUpsertPayload,
  TodoKind,
  TodoLink,
  TodoPriority,
  TodoRecurrence,
  TodoReminderPreset,
  TodoRepeatPreset,
  TodoRule,
  TodoRuleMode,
  TodoSimpleRule,
  TodoStatus,
  TodoType,
} from "../types";
import { PM_STATUS_COLUMNS } from "../types/pm";
import type { PmCandidateItem } from "../types/pm";
import { useTabs } from "../composables/useTabs";
import { groupTodoItemsByBucket } from "../utils/todoBuckets";
import { clampContextMenuPosition } from "../utils/contextMenu";
import { formatTodoRelativeDateTimeLabel } from "../utils/todoRelativeDate";
import {
  prevMonth as calPrevMonth,
  nextMonth as calNextMonth,
  formatDateKey,
} from "../utils/calendarGrid";
import TodoCalendarGrid from "./TodoCalendarGrid.vue";
import TodoDetailView from "./TodoDetailView.vue";
import TodoDetailEdit from "./TodoDetailEdit.vue";
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
type DetailMode = "empty" | "view" | "edit" | "create";

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

function pmStatusColor(status: string | null | undefined): string {
  return PM_STATUS_COLUMNS.find(c => c.key === (status || "todo"))?.color ?? "#909399";
}
function pmStatusLabel(status: string | null | undefined): string {
  return PM_STATUS_COLUMNS.find(c => c.key === (status || "todo"))?.label ?? "待办";
}

const items = ref<TodoItem[]>([]);
const types = ref<TodoType[]>([]);
const assignees = ref<TodoAssignee[]>([]);
const projectOptions = ref<{ id: number; name: string; color: string }[]>([]);
const filterProjectId = ref<number | string | null>(null);
const showMoreFields = ref(false);
const itemKeyword = ref("");
const todoContextMenuRef = ref<HTMLElement | null>(null);
const todoDetailEditRef = ref<{
  titleInputRef: { value: { focus: () => void } | null };
  descTextareaRef: { value: { $el: HTMLElement } | null };
  scrollRef: { value: HTMLElement | null };
  scheduleRef: { value: HTMLElement | null };
} | null>(null);
const filterType = ref<string | null>(null);
const filterPriority = ref<TodoPriority | null>(null);
const doneCollapsed = ref(true);
const recentWeekCollapsed = ref(true);
const basicsDialogVisible = ref(false);
const viewMode = ref<"list" | "calendar">("list");
const todoPmLinkItemId = ref<number | null>(null);
const todoPmCandidates = ref<PmCandidateItem[]>([]);
let skipProjectWatch = false;
const todoLinkedPmItem = ref<{ id: number; title: string; status: string; projectId: number } | null>(null);
const pmCreateDialogVisible = ref(false);
const pmCreateTitle = ref("");
const pmCreateProjectId = ref<number | null>(null);
const calendarMonth = ref(new Date());
const itemDialogMode = ref<ItemDialogMode>("create");
const detailMode = ref<DetailMode>("empty");
const selectedItemId = ref<number | null>(null);
const draftBaseline = ref("");
const typeDialogVisible = ref(false);
const assigneeDialogVisible = ref(false);
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
let titleFocusTimer: ReturnType<typeof setTimeout> | null = null;
const { watchPendingToolInput } = useClipboardSuggestion();
const { openTab } = useTabs();

// Drawer 相关状态

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

import {
  asRecord,
  effectiveReminderPresets,
  getResponseItems,
  getRootItemId,
  normalizeReminderPresets,
  normalizeTodoItem,
  readNullableNumber,
  reminderPresetFromMinutes,
  reminderPresetToMinutes,
  toDraftReminderPresets,
} from "../composables/useTodoItem";

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
});

const typeDraft = reactive<TodoTypeDraft>({ id: 0, name: "", color: "", sortOrder: 0 });
const assigneeDraft = reactive<TodoAssigneeDraft>({ id: 0, name: "" });

const isRepeating = computed(() => itemDraft.repeatPreset !== "none");

const filteredItems = computed(() => {
  const keyword = itemKeyword.value.trim().toLowerCase();
  return items.value.filter((item) => {
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
const recentWeekItems = computed(() => bucketedItems.value.recentWeekItems);
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
const displayRecentWeekItems = computed(() => applyDisplayFilter(recentWeekItems.value));
const displayDoneItems = computed(() => applyDisplayFilter(doneItems.value));
const todoContextMenuItem = computed(() =>
  todoContextMenu.itemId == null
    ? null
    : items.value.find((item) => item.id === todoContextMenu.itemId) || null,
);
const selectedItem = computed(() =>
  selectedItemId.value == null
    ? null
    : items.value.find((item) => item.id === selectedItemId.value) || null,
);
const allItemsForCalendar = computed(() => items.value);
const isDetailEditing = computed(
  () => detailMode.value === "edit" || detailMode.value === "create",
);
const isDraftDirty = computed(
  () => isDetailEditing.value && draftBaseline.value !== snapshotItemDraft(),
);

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

function normalizeDraftTypeValue(value: SelectTypeValue) {
  if (typeof value === "number") return value;
  const name = typeof value === "string" ? value.trim() : "";
  return name || null;
}

function normalizeDraftAssigneeValues(values: SelectAssigneeValue[]) {
  return values
    .map((value) => (typeof value === "number" ? `id:${value}` : `name:${value.trim()}`))
    .filter((value) => !value.endsWith(":"))
    .sort();
}

function insertMdSyntax(prefix: string, suffix: string) {
  const el = todoDetailEditRef.value?.descTextareaRef.value?.$el?.querySelector("textarea") as HTMLTextAreaElement | null;
  if (!el) return;
  const start = el.selectionStart;
  const end = el.selectionEnd;
  const text = itemDraft.description;
  const selected = text.slice(start, end);
  const replacement = prefix + (selected || "文本") + suffix;
  itemDraft.description = text.slice(0, start) + replacement + text.slice(end);
  nextTick(() => {
    const cursorPos = start + prefix.length + (selected || "文本").length;
    el.focus();
    el.setSelectionRange(
      selected ? start + prefix.length : start + prefix.length,
      selected ? start + prefix.length + selected.length : cursorPos,
    );
  });
}

function snapshotItemDraft() {
  return JSON.stringify({
    mode: itemDialogMode.value,
    title: itemDraft.title.trim(),
    typeId: normalizeDraftTypeValue(itemDraft.typeId),
    priority: itemDraft.priority,
    description: itemDraft.description,
    assigneeIds: normalizeDraftAssigneeValues(itemDraft.assigneeIds),
    eventDate: itemDraft.eventDate,
    eventTime: itemDraft.eventTime,
    reminderPresets: normalizeReminderPresets(itemDraft.reminderPresets),
    repeatPreset: itemDraft.repeatPreset,
    ruleMode: itemDraft.ruleMode,
    timezone: itemDraft.timezone,
    cronExpression: itemDraft.cronExpression.trim(),
    endMode: itemDraft.endMode,
    endValueDate: itemDraft.endValueDate,
    endValueCount: Number(itemDraft.endValueCount || 1),
    simple: {
      frequency: itemDraft.simple.frequency,
      interval: Number(itemDraft.simple.interval || 1),
      time: itemDraft.simple.time,
      weekdays: [...itemDraft.simple.weekdays].sort((left, right) => left - right),
      dayOfMonth: Number(itemDraft.simple.dayOfMonth || 1),
    },
  });
}

function markDraftBaseline() {
  draftBaseline.value = snapshotItemDraft();
}

async function ensureDetailCanLeave() {
  if (!isDetailEditing.value || !isDraftDirty.value) return true;
  const result = await submitItemChanges(false);
  if (!result.ok) return false;
  finalizeDetailAfterSave(result.id);
  return true;
}

function finalizeDetailAfterSave(savedId?: number | null) {
  const fallbackId = itemDialogMode.value === "edit_item" ? itemDraft.id : null;
  const nextId = savedId ?? selectedItemId.value ?? fallbackId;
  resetItemDraft();
  draftBaseline.value = "";
  if (typeof nextId === "number" && nextId > 0) {
    selectedItemId.value = nextId;
    detailMode.value = "view";
    return;
  }
  selectedItemId.value = null;
  detailMode.value = "empty";
}

function selectItem(item: TodoItem) {
  if (selectedItemId.value === item.id && detailMode.value === "view") return;
  if (isDetailEditing.value && isDraftDirty.value) {
    selectItemAsync(item);
    return;
  }
  selectedItemId.value = item.id;
  detailMode.value = "view";
}

async function selectItemAsync(item: TodoItem) {
  if (!(await ensureDetailCanLeave())) return;
  selectedItemId.value = item.id;
  detailMode.value = "view";
}

type TodoContextMenuCommand = "pin" | "complete" | "edit-time" | "delete";

const TODO_CONTEXT_MENU_PADDING = 12;

function closeTodoContextMenu() {
  todoContextMenu.visible = false;
  todoContextMenu.itemId = null;
}

async function prepareItemForInlineAction(item: TodoItem) {
  if (!(await ensureDetailCanLeave())) return false;
  selectedItemId.value = item.id;
  detailMode.value = "view";
  return true;
}

function positionTodoContextMenu(anchorX: number, anchorY: number) {
  const menu = todoContextMenuRef.value;
  if (!menu) return;
  const position = clampContextMenuPosition({
    anchorX,
    anchorY,
    menuWidth: menu.offsetWidth,
    menuHeight: menu.offsetHeight,
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
    padding: TODO_CONTEXT_MENU_PADDING,
  });
  todoContextMenu.x = position.x;
  todoContextMenu.y = position.y;
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
  await nextTick();
  positionTodoContextMenu(event.clientX, event.clientY);
}

async function enterEditTimeMode(item?: TodoItem | null) {
  const target = item || selectedItem.value;
  if (!target) return;
  await enterEditMode(target);
  if (selectedItemId.value !== target.id) return;
  showMoreFields.value = true;
  await nextTick();
  const scheduleSection = todoDetailEditRef.value?.scheduleRef.value;
  if (!scheduleSection) return;
  const formScroll = todoDetailEditRef.value?.scrollRef.value;
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

function onTodoContextMenuGlobalClick(event: MouseEvent) {
  if (!todoContextMenu.visible) return;
  const target = event.target;
  if (target instanceof Node && todoContextMenuRef.value?.contains(target)) return;
  closeTodoContextMenu();
}

function onTodoContextMenuGlobalContextMenu(event: MouseEvent) {
  if (!todoContextMenu.visible) return;
  const target = event.target;
  if (target instanceof Node && todoContextMenuRef.value?.contains(target)) return;
  closeTodoContextMenu();
}

function onTodoContextMenuGlobalKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  closeTodoContextMenu();
}

async function focusCreateTitleInput() {
  if (titleFocusTimer) {
    clearTimeout(titleFocusTimer);
    titleFocusTimer = null;
  }
  await nextTick();
  titleFocusTimer = setTimeout(() => {
    titleFocusTimer = null;
    if (detailMode.value !== "create" || itemDialogMode.value !== "create") return;
    todoDetailEditRef.value?.titleInputRef.value?.focus();
  }, 0);
}

function onCreateTitleEnter(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (detailMode.value !== "create" || itemDialogMode.value !== "create") return;
  void saveItem();
}

async function startCreate() {
  if (!(await ensureDetailCanLeave())) return;
  resetItemDraft();
  itemDialogMode.value = "create";
  detailMode.value = "create";
  selectedItemId.value = null;
  showMoreFields.value = false;
  markDraftBaseline();
  await focusCreateTitleInput();
}

async function createOnDate(dateKey: string) {
  if (!(await ensureDetailCanLeave())) return;
  resetItemDraft();
  itemDialogMode.value = "create";
  detailMode.value = "create";
  selectedItemId.value = null;
  itemDraft.eventDate = dateKey;
  itemDraft.eventTime = "09:00";
  showMoreFields.value = true;
  markDraftBaseline();
  await focusCreateTitleInput();
}

function cancelDetailEdit() {
  resetItemDraft();
  draftBaseline.value = "";
  if (selectedItemId.value !== null && selectedItem.value) {
    detailMode.value = "view";
    return;
  }
  detailMode.value = "empty";
}

const editingItemIsRecurring = computed(
  () => !!editingItemSnapshot.value && itemKindOf(editingItemSnapshot.value) === "recurring",
);
const showRecurrenceFields = computed(() => {
  return isRepeating.value;
});
const showCustomRepeatFields = computed(
  () => isRepeating.value && itemDraft.repeatPreset === "custom",
);
const showCronRepeatFields = computed(() => isRepeating.value && itemDraft.repeatPreset === "cron");
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
        itemDraft.repeatPreset = deriveRepeatPreset(editingItemSnapshot.value?.recurrence || null);
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

function fillDefaultDateTime() {
  const defaults = getCreateDraftDefaultDateTime();
  itemDraft.eventDate = defaults.date;
  itemDraft.eventTime = defaults.time;
  itemDraft.simple.time = defaults.time;
  if (itemDraft.reminderPresets.length === 1 && itemDraft.reminderPresets[0] === "none") {
    itemDraft.reminderPresets = ["0m"];
    lastReminderPresetSelection.value = ["0m"];
  }
}

function fillQuickDate(daysOffset: number) {
  const target = new Date();
  target.setDate(target.getDate() + daysOffset);
  const year = target.getFullYear();
  const month = pad2(target.getMonth() + 1);
  const day = pad2(target.getDate());
  itemDraft.eventDate = `${year}-${month}-${day}`;
  if (itemDraft.reminderPresets.length === 1 && itemDraft.reminderPresets[0] === "none") {
    itemDraft.reminderPresets = ["0m"];
    lastReminderPresetSelection.value = ["0m"];
  }
}

function resetItemDraft() {
  itemDraft.id = 0;
  itemDraft.rootId = 0;
  itemDraft.title = "";
  itemDraft.typeId = undefined;
  itemDraft.priority = "P2";
  itemDraft.description = "";
  itemDraft.assigneeIds = [];
  itemDraft.links = [];
  itemDraft.eventDate = "";
  itemDraft.eventTime = "";
  itemDraft.reminderPresets = [...defaultReminderPresets];
  itemDraft.repeatPreset = "none";
  itemDraft.ruleMode = "simple";
  itemDraft.timezone = "local";
  itemDraft.cronExpression = "0 0 9 * * Mon-Fri";
  itemDraft.endMode = "never";
  itemDraft.endValueDate = "";
  itemDraft.endValueCount = 1;
  itemDraft.simple.frequency = "daily";
  itemDraft.simple.interval = 1;
  itemDraft.simple.time = "";
  itemDraft.simple.weekdays = [1, 2, 3, 4, 5];
  itemDraft.simple.dayOfMonth = 1;
  itemDraft.projectId = null;
  itemDraft.pmItemId = null;
  itemDraft.pmItemTitle = null;
  itemDraft.pmItemProjectId = null;
  itemDraft.pmItemStatus = null;
  todoPmLinkItemId.value = null;
  todoPmCandidates.value = [];
  todoLinkedPmItem.value = null;
  lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
  editingItemSnapshot.value = null;
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
  itemDraft.links = (item.links || []).map((l) => ({ url: l.url, title: l.title }));
  itemDraft.eventDate = date;
  itemDraft.eventTime = time;
  itemDraft.reminderPresets = toDraftReminderPresets(item.reminderPresets);
  lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
  const recurrence = getItemRecurrence(item);
  itemDraft.repeatPreset =
    itemKindOf(item) === "recurring" ? deriveRepeatPreset(recurrence) : "none";

  // 新增：加载详细规则到 simple 对象
  if (itemKindOf(item) === "recurring" && recurrence?.rule) {
    itemDraft.ruleMode = recurrence.ruleMode || "simple";
    itemDraft.timezone = recurrence.timezone || "local";
    if (itemDraft.ruleMode === "simple") {
      syncSimpleDraftFromRule(recurrence.rule as TodoRule);
    } else if (itemDraft.ruleMode === "cron") {
      itemDraft.cronExpression =
        recurrence.cronExpression ||
        (recurrence.rule as { expression?: string }).expression ||
        itemDraft.cronExpression;
    }
  }
  skipProjectWatch = true;
  itemDraft.projectId = item.projectId ?? null;
  itemDraft.pmItemId = item.pmItemId ?? null;
  itemDraft.pmItemTitle = item.pmItemTitle ?? null;
  itemDraft.pmItemProjectId = item.pmItemProjectId ?? null;
  itemDraft.pmItemStatus = item.pmItemStatus ?? null;
  todoPmLinkItemId.value = item.pmItemId ?? null;
  // Populate linked PM item info for display in dropdown
  if (item.pmItemId) {
    todoLinkedPmItem.value = {
      id: item.pmItemId,
      title: item.pmItemTitle ?? "",
      status: item.pmItemStatus ?? "todo",
      projectId: item.pmItemProjectId ?? item.projectId ?? 0,
    };
  } else {
    todoLinkedPmItem.value = null;
  }
  if (item.projectId && item.kind !== "recurring") {
    loadTodoPmCandidates(item.projectId, item.pmItemId);
  } else {
    todoPmCandidates.value = [];
  }
  // Ensure skipProjectWatch is consumed: if projectId didn't change (e.g. both null),
  // the watcher won't fire, so we reset here after the scheduler flushes.
  nextTick(() => { skipProjectWatch = false; });
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
  closeTodoContextMenu();
  const params: Record<string, unknown> = {};
  if (filterProjectId.value === "none") {
    params.projectFilter = "none";
  } else if (typeof filterProjectId.value === "number") {
    params.projectId = filterProjectId.value;
  }
  items.value = getResponseItems(await invokeToolByChannel("tool:todo:item-list", params)).map(
    normalizeTodoItem,
  );
}

async function loadProjects() {
  try {
    const list = (await invokeToolByChannel("tool:pm:project-list", {})) as { id: number; name: string; color: string; status: string }[];
    projectOptions.value = (list || []).filter((p) => p.status === "active");
  } catch {
    projectOptions.value = [];
  }
}

async function loadTodoPmCandidates(projectId: number, linkedPmItemId?: number | null) {
  try {
    const result = await invokeToolByChannel("tool:todo:pm-candidates", { projectId }) as { items: PmCandidateItem[] };
    let candidates = result?.items || [];
    // Ensure currently linked PM item is in the list (it may be filtered out by other criteria)
    if (linkedPmItemId && !candidates.some((c) => c.id === linkedPmItemId)) {
      const linked = todoLinkedPmItem.value;
      if (linked && linked.id === linkedPmItemId) {
        candidates = [
          { id: linked.id, title: linked.title, status: linked.status, priority: "P2", projectId: linked.projectId, projectName: null, projectColor: null },
          ...candidates,
        ];
      }
    }
    todoPmCandidates.value = candidates;
  } catch {
    todoPmCandidates.value = [];
  }
}

async function onPmCreateConfirm() {
  const title = pmCreateTitle.value.trim();
  if (!title) {
    ElMessage.warning("请输入工作项标题");
    return;
  }
  const projectId = itemDraft.projectId ?? pmCreateProjectId.value;
  if (!projectId) {
    ElMessage.warning("请选择所属项目");
    return;
  }
  try {
    // Set project on draft if not already set
    if (!itemDraft.projectId) {
      skipProjectWatch = true;
      itemDraft.projectId = projectId;
    }
    // If the todo item hasn't been saved yet (create mode), save it first
    let todoId = itemDraft.id;
    if (!todoId) {
      const saveResult = await submitItemChanges(false);
      if (!saveResult.ok || !saveResult.id) {
        return;
      }
      todoId = saveResult.id;
      itemDraft.id = todoId;
    } else {
      // Existing todo — persist the project change before linking
      await submitItemChanges(false);
    }
    const result = await invokeToolByChannel("tool:pm:item-create", {
      projectId,
      title,
      itemType: "task",
      priority: "P2",
      status: "todo",
    }) as { id: number };
    await invokeToolByChannel("tool:todo:item-set-pm-link", {
      todoItemId: todoId,
      pmItemId: result.id,
    });
    itemDraft.pmItemId = result.id;
    itemDraft.pmItemTitle = title;
    itemDraft.pmItemProjectId = projectId;
    itemDraft.pmItemStatus = "todo";
    todoPmLinkItemId.value = result.id;
    todoLinkedPmItem.value = { id: result.id, title, status: "todo", projectId };
    pmCreateDialogVisible.value = false;
    pmCreateTitle.value = "";
    pmCreateProjectId.value = null;
    await loadTodoPmCandidates(projectId);
    await loadItems();
    // If the item was just created (was in create mode), switch to edit mode so user can continue editing
    if (itemDialogMode.value === "create") {
      itemDialogMode.value = "edit_item";
      selectedItemId.value = todoId;
    }
    ElMessage.success("工作项已创建并关联");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function onPmCreateClosed() {
  pmCreateTitle.value = "";
  pmCreateProjectId.value = null;
}

async function onTodoPmLinkChange(pmItemId: number | null) {
  if (!itemDraft.id) return;
  try {
    // Ensure the project assignment is persisted before linking/unlinking PM item
    await submitItemChanges(false);
    if (pmItemId) {
      await invokeToolByChannel("tool:todo:item-set-pm-link", {
        todoItemId: itemDraft.id,
        pmItemId,
      });
    } else {
      await invokeToolByChannel("tool:todo:item-set-pm-link", {
        todoItemId: itemDraft.id,
        pmItemId: null,
      });
    }
    itemDraft.pmItemId = pmItemId;
    todoPmLinkItemId.value = pmItemId;
    const candidate = pmItemId ? todoPmCandidates.value.find((c) => c.id === pmItemId) : null;
    itemDraft.pmItemTitle = candidate?.title ?? null;
    itemDraft.pmItemProjectId = candidate?.projectId ?? null;
    itemDraft.pmItemStatus = candidate?.status ?? null;
    if (candidate) {
      todoLinkedPmItem.value = { id: candidate.id, title: candidate.title, status: candidate.status, projectId: candidate.projectId };
    } else {
      todoLinkedPmItem.value = null;
    }
    await loadItems();
  } catch (error) {
    ElMessage.error((error as Error).message);
    todoPmLinkItemId.value = itemDraft.pmItemId;
  }
}

function handlePmSelectChange(value: number | null) {
  if (value === -1) {
    todoPmLinkItemId.value = null;
    pmCreateDialogVisible.value = true;
    pmCreateTitle.value = "";
    return;
  }
  void onTodoPmLinkChange(value);
}

function navigateToPmItem(pmItemId: number, _pmProjectId: number | null) {
  openTab("pm", "项目管理");
  ElMessage.info({ message: `已切换到项目管理，请查看工作项 #${pmItemId}`, duration: 3000 });
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

async function copyTitle(title: string) {
  await navigator.clipboard.writeText(title);
  ElMessage.success("标题已复制");
}

async function enterEditMode(item?: TodoItem | null) {
  const target = item || selectedItem.value;
  if (!target) return;
  if (detailMode.value === "edit" && selectedItemId.value === target.id) return;
  if (!(await ensureDetailCanLeave())) return;
  selectedItemId.value = target.id;
  resetItemDraft();
  itemDialogMode.value = "edit_item";
  editingItemSnapshot.value = target;
  applyItemToDraft(target);
  detailMode.value = "edit";
  showMoreFields.value =
    target.assignees.length > 0 ||
    !!target.eventAt ||
    effectiveReminderPresets(target.reminderPresets).length > 0 ||
    hasRepeatRule(target);
  markDraftBaseline();
}

async function submitItemChanges(showSuccess = true) {
  const title = itemDraft.title.trim();
  const eventAt = buildEventAt();
  const selectedReminderPresets = effectiveReminderPresets(itemDraft.reminderPresets);
  const hasEventDate = !!itemDraft.eventDate.trim();
  const hasEventTime = !!itemDraft.eventTime.trim();
  if (!title) {
    ElMessage.warning("请输入事项标题");
    return { ok: false, id: null as number | null };
  }
  if (hasEventDate !== hasEventTime) {
    ElMessage.warning("日期和时间需要同时填写或同时清空");
    return { ok: false, id: null as number | null };
  }
  if (hasEventTime && !isFiveMinuteTime(itemDraft.eventTime)) {
    ElMessage.warning("事件时间仅支持5分钟刻度");
    return { ok: false, id: null as number | null };
  }
  if (selectedReminderPresets.length > 0 && !eventAt) {
    ElMessage.warning("请先填写日期和时间，再设置提醒方式");
    return { ok: false, id: null as number | null };
  }
  if (isRepeating.value && showRecurrenceFields.value) {
    if (!hasEventDate || !hasEventTime) {
      ElMessage.warning("重复事项需要同时填写日期和时间");
      return { ok: false, id: null as number | null };
    }
    if (!isFiveMinuteTime(itemDraft.eventTime)) {
      ElMessage.warning("时间仅支持5分钟刻度");
      return { ok: false, id: null as number | null };
    }
    if (!eventAt) {
      ElMessage.warning("日期或时间格式不正确");
      return { ok: false, id: null as number | null };
    }
    if (showCronRepeatFields.value && !itemDraft.cronExpression.trim()) {
      ElMessage.warning("请输入 Cron 表达式");
      return { ok: false, id: null as number | null };
    }
    if (
      showCustomRepeatFields.value &&
      itemDraft.simple.frequency === "weekly" &&
      Number(itemDraft.simple.interval || 1) > 1
    ) {
      ElMessage.warning("按周自定义暂不支持大于 1 的间隔，请改用高级 Cron");
      return { ok: false, id: null as number | null };
    }
    if (
      itemDraft.endMode === "until_date" &&
      itemDraft.endValueDate &&
      !isFiveMinuteDateTime(itemDraft.endValueDate)
    ) {
      ElMessage.warning("结束时间仅支持5分钟刻度");
      return { ok: false, id: null as number | null };
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
      links: itemDraft.links.filter((l) => l.url.trim()),
      reminderPresets: selectedReminderPresets,
    };

    const kind: TodoKind = isRepeating.value ? "recurring" : "one_off";
    const payload: TodoItemUpsertPayload & Record<string, unknown> = {
      ...commonPayload,
      kind,
      projectId: itemDraft.projectId,
    };

    if (!isRepeating.value) {
      payload.eventAt = eventAt;
    }

    if (isRepeating.value) {
      payload.recurrence = {
        startAt: eventAt,
        ruleMode: itemDraft.ruleMode,
        rule: buildRulePayload(),
        timezone: itemDraft.timezone || "local",
        endMode: itemDraft.endMode,
        endValue: buildEndValue(),
      };
    }

    let response: unknown;
    if (itemDialogMode.value === "create") {
      response = await invokeToolByChannel("tool:todo:item-create", payload);
    } else {
      payload.id = itemDraft.id;
      if (itemDraft.rootId) payload.rootId = itemDraft.rootId;
      response = await invokeToolByChannel("tool:todo:item-update", payload);
    }

    const savedId =
      readNullableNumber(asRecord(response), ["id"]) ??
      (itemDialogMode.value === "edit_item" ? itemDraft.id : null);
    await loadItems();
    if (showSuccess) ElMessage.success("保存成功");
    return { ok: true, id: savedId };
  } catch (error) {
    ElMessage.error((error as Error).message);
    return { ok: false, id: null as number | null };
  }
}

async function saveItem() {
  const result = await submitItemChanges(true);
  if (!result.ok) return;
  finalizeDetailAfterSave(result.id);
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

async function openLink(url: string) {
  try {
    await invokeToolByChannel("tool:todo:open-link", { url });
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
    // 普通事项：直接删除
    if (item.kind !== "recurring") {
      await ElMessageBox.confirm("确认删除该事项吗？", "删除确认", {
        type: "warning",
      });
      await invokeToolByChannel("tool:todo:item-delete", {
        id: item.id,
        scope: "this_instance",
      });
      await loadItems();
      return;
    }

    // 重复事项：显示选择对话框
    const scope = await showDeleteScopeDialog(item.title);
    if (scope === null) return; // 用户取消

    await invokeToolByChannel("tool:todo:item-delete", {
      id: item.id,
      scope: scope, // "this_instance" | "future_instances"
    });
    await loadItems();
  } catch (error) {
    if ((error as Error).message) ElMessage.error((error as Error).message);
  }
}

async function showDeleteScopeDialog(itemTitle: string): Promise<string | null> {
  const baseStyle =
    "display: flex; align-items: flex-start; gap: 12px; padding: 14px 16px; border: 1.5px solid var(--lc-border); border-radius: 10px; background: var(--lc-surface-1); cursor: pointer; text-align: left; transition: border-color 0.2s, background 0.2s, box-shadow 0.2s, transform 0.15s; width: 100%; outline: none;";
  const iconBoxBase =
    "flex-shrink: 0; width: 34px; height: 34px; border-radius: 8px; display: flex; align-items: center; justify-content: center; font-size: 16px; transition: background 0.2s;";
  const labelStyle =
    "font-size: 14px; font-weight: 600; line-height: 1.4; transition: color 0.2s;";
  const descStyle = "font-size: 12px; color: var(--lc-text-muted); line-height: 1.4; margin-top: 2px;";

  // SVG trash icon (single instance / mild)
  const svgTrashOne =
    '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>';
  // SVG trash-x icon (all instances / destructive)
  const svgTrashAll =
    '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/><line x1="10" y1="11" x2="14" y2="16"/><line x1="14" y1="11" x2="10" y2="16"/></svg>';

  interface OptionCfg {
    label: string;
    desc: string;
    scope: string;
    iconSvg: string;
    accentColor: string;
    accentBg: string;
  }

  const makeOption = (cfg: OptionCfg, resolveFn: (v: string) => void) =>
    h(
      "button",
      {
        style: baseStyle,
        onMouseenter: (e: MouseEvent) => {
          const el = e.currentTarget as HTMLElement;
          el.style.borderColor = cfg.accentColor;
          el.style.background = "var(--lc-surface-2)";
          el.style.boxShadow = `0 2px 8px ${cfg.accentColor}18`;
          el.style.transform = "translateY(-1px)";
        },
        onMouseleave: (e: MouseEvent) => {
          const el = e.currentTarget as HTMLElement;
          el.style.borderColor = "var(--lc-border)";
          el.style.background = "var(--lc-surface-1)";
          el.style.boxShadow = "none";
          el.style.transform = "none";
        },
        onFocus: (e: FocusEvent) => {
          const el = e.currentTarget as HTMLElement;
          el.style.borderColor = cfg.accentColor;
          el.style.boxShadow = `0 0 0 2px ${cfg.accentColor}30`;
        },
        onBlur: (e: FocusEvent) => {
          const el = e.currentTarget as HTMLElement;
          el.style.borderColor = "var(--lc-border)";
          el.style.boxShadow = "none";
        },
        onClick: () => {
          ElMessageBox.close();
          resolveFn(cfg.scope);
        },
      },
      [
        h("span", {
          style: `${iconBoxBase} background: ${cfg.accentBg}; color: ${cfg.accentColor};`,
          innerHTML: cfg.iconSvg,
        }),
        h("div", { style: "flex: 1; min-width: 0;" }, [
          h("div", { style: `${labelStyle} color: var(--lc-text);` }, cfg.label),
          h("div", { style: descStyle }, cfg.desc),
        ]),
      ],
    );

  return new Promise((resolve) => {
    ElMessageBox({
      title: "删除重复事项",
      message: h("div", { style: "padding: 8px 0 4px;" }, [
        h(
          "p",
          {
            style:
              "margin-bottom: 16px; font-size: 13px; color: var(--lc-text-muted); line-height: 1.5;",
          },
          [
            h("span", null, "「"),
            h(
              "span",
              { style: "font-weight: 600; color: var(--lc-text);" },
              itemTitle,
            ),
            h("span", null, "」是重复事项，请选择删除范围："),
          ],
        ),
        h(
          "div",
          { style: "display: flex; flex-direction: row; gap: 10px;" },
          [
            makeOption(
              {
                label: "仅删除本次",
                desc: "后续重复事项将继续按规则生成",
                scope: "this_instance",
                iconSvg: svgTrashOne,
                accentColor: "var(--lc-accent)",
                accentBg: "var(--lc-accent-bg, rgba(64,150,255,0.08))",
              },
              resolve,
            ),
            makeOption(
              {
                label: "删除本次及后续所有",
                desc: "停止后续自动生成，已完成的实例不受影响",
                scope: "future_instances",
                iconSvg: svgTrashAll,
                accentColor: "var(--lc-danger, #e25050)",
                accentBg: "rgba(226,80,80,0.08)",
              },
              resolve,
            ),
          ],
        ),
      ]),
      showCancelButton: true,
      showConfirmButton: false,
      cancelButtonText: "取消",
      customClass: "todo-delete-scope-dialog",
      closeOnClickModal: true,
      closeOnPressEscape: true,
      beforeClose: (_action: string, _instance: unknown, done: () => void) => {
        resolve(null);
        done();
      },
    });
  });
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

watch(filterProjectId, () => loadItems());

watch(
  () => itemDraft.projectId,
  (newProjectId) => {
    if (skipProjectWatch) {
      skipProjectWatch = false;
      return;
    }
    todoPmLinkItemId.value = null;
    itemDraft.pmItemId = null;
    itemDraft.pmItemTitle = null;
    itemDraft.pmItemProjectId = null;
    itemDraft.pmItemStatus = null;
    todoLinkedPmItem.value = null;
    if (newProjectId && itemDraft.kind !== "recurring") {
      loadTodoPmCandidates(newProjectId);
    } else {
      todoPmCandidates.value = [];
    }
  },
);

watch(selectedItem, (item) => {
  if (detailMode.value === "create") return;
  if (selectedItemId.value !== null && !item) {
    selectedItemId.value = null;
    draftBaseline.value = "";
    resetItemDraft();
    detailMode.value = "empty";
  }
});

watch(viewMode, () => {
  closeTodoContextMenu();
});

watch(todoContextMenuItem, (item) => {
  if (!todoContextMenu.visible || item) return;
  closeTodoContextMenu();
});

watchPendingToolInput("todo", (input) => applyPendingTodoInput(input));

onMounted(async () => {
  document.addEventListener("click", onTodoContextMenuGlobalClick);
  document.addEventListener("contextmenu", onTodoContextMenuGlobalContextMenu);
  window.addEventListener("keydown", onTodoContextMenuGlobalKeydown);
  await Promise.all([loadTypes(), loadAssignees(), loadItems(), loadProjects()]);
  try {
    reminderUnlisten = await listen("todo-reminder-fired", async () => {
      await loadItems();
    });
  } catch {
    reminderUnlisten = null;
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("click", onTodoContextMenuGlobalClick);
  document.removeEventListener("contextmenu", onTodoContextMenuGlobalContextMenu);
  window.removeEventListener("keydown", onTodoContextMenuGlobalKeydown);
  closeTodoContextMenu();
  reminderUnlisten?.();
  reminderUnlisten = null;
  if (titleFocusTimer) {
    clearTimeout(titleFocusTimer);
    titleFocusTimer = null;
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

/* --- Detail Empty State (Enhanced) --- */
.detail-empty-pane {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 20px;
  padding: 32px 24px;
  text-align: center;
  animation: fadeIn 0.4s var(--lc-ease);
}

.detail-empty-visual {
  position: relative;
}

.empty-illustration {
  position: relative;
  width: 180px;
  height: 144px;
}

.empty-svg {
  width: 100%;
  height: 100%;
  animation: float 6s ease-in-out infinite;
}

.empty-glow {
  position: absolute;
  inset: 20%;
  background: radial-gradient(circle, var(--lc-accent) 0%, transparent 70%);
  opacity: 0.1;
  filter: blur(20px);
  animation: pulse 4s ease-in-out infinite;
}

@keyframes float {
  0%,
  100% {
    transform: translateY(0px);
  }
  50% {
    transform: translateY(-8px);
  }
}

@keyframes pulse {
  0%,
  100% {
    opacity: 0.08;
    transform: scale(1);
  }
  50% {
    opacity: 0.15;
    transform: scale(1.1);
  }
}

.detail-empty-content {
  max-width: 280px;
}

.detail-empty-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--lc-text);
  margin-bottom: 8px;
}

.detail-empty-text {
  font-size: 13px;
  line-height: 1.7;
  color: var(--lc-text-muted);
}

.detail-empty-actions {
  display: flex;
  gap: 12px;
  margin-top: 4px;
}

.empty-btn-icon {
  margin-right: 4px;
}

.detail-empty-divider {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  max-width: 320px;
  color: var(--lc-text-muted);
  font-size: 12px;
  margin-top: 8px;
}

.detail-empty-divider::before,
.detail-empty-divider::after {
  content: "";
  flex: 1;
  height: 1px;
  background: var(--lc-border);
}

.detail-empty-stats {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  width: 100%;
  max-width: 320px;
}

.detail-empty-stat {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px;
  border-radius: 12px;
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  transition: all 0.25s var(--lc-ease);
}

.detail-empty-stat:hover {
  transform: translateY(-2px);
  box-shadow: var(--lc-shadow-sm);
}

.detail-empty-stat.is-active {
  background: linear-gradient(135deg, rgba(56, 189, 248, 0.08), var(--lc-surface-1));
  border-color: rgba(56, 189, 248, 0.3);
}

.detail-empty-stat.is-alert {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.08), var(--lc-surface-1));
  border-color: rgba(248, 113, 113, 0.3);
}

.detail-empty-stat .stat-icon {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 10px;
  font-size: 18px;
  flex-shrink: 0;
}

.stat-icon.today {
  color: var(--lc-accent);
  background: rgba(56, 189, 248, 0.12);
}

.stat-icon.overdue {
  color: var(--lc-danger);
  background: rgba(248, 113, 113, 0.12);
}

.detail-empty-stat .stat-info {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
}

.detail-empty-stat .stat-label {
  font-size: 12px;
  color: var(--lc-text-muted);
  white-space: nowrap;
}

.detail-empty-stat .stat-value {
  font-size: 20px;
  font-weight: 700;
  color: var(--lc-text);
  white-space: nowrap;
}

.detail-empty-stat.is-alert .stat-value {
  color: var(--lc-danger);
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
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

.todo-context-menu {
  position: fixed;
  z-index: 3000;
  min-width: 164px;
  padding: 6px;
  border-radius: 14px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 18px 42px rgba(15, 23, 42, 0.16);
  backdrop-filter: blur(16px);
  animation: todoContextMenuEnter 0.16s var(--lc-ease-out);
}

.todo-context-menu-item {
  width: 100%;
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: var(--lc-text);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  transition:
    background var(--lc-duration) var(--lc-ease),
    color var(--lc-duration) var(--lc-ease);
}

.todo-context-menu-item:hover {
  background: rgba(14, 165, 233, 0.08);
  color: var(--lc-accent-strong);
}

.todo-context-menu-item.is-danger {
  color: var(--lc-danger);
}

.todo-context-menu-item.is-danger:hover {
  background: rgba(248, 113, 113, 0.12);
  color: var(--lc-danger);
}

.todo-context-menu-divider {
  height: 1px;
  margin: 6px 4px;
  background: rgba(148, 163, 184, 0.18);
}

@keyframes todoContextMenuEnter {
  from {
    opacity: 0;
    transform: translateY(6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
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

/* --- Sidebar stats --- */
.todo-stats {
  width: auto;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 20px;
  overflow-y: auto;
  padding-right: 4px;
}
.stats-section {
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-sm);
  padding: 14px;
}
.stats-section-title {
  font-family: var(--lc-font-display);
  font-size: 11px;
  font-weight: 700;
  color: var(--lc-text-muted);
  margin-bottom: 10px;
  text-transform: uppercase;
  letter-spacing: 0.8px;
}
.stats-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 10px;
}
.stats-section-header .stats-section-title {
  margin-bottom: 0;
}
.overview-settings-btn {
  padding: 0;
  gap: 4px;
  font-weight: 500;
}
.stats-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
.stat-card {
  text-align: center;
  padding: 10px 8px 8px;
  background: var(--lc-surface-2);
  border-radius: 6px;
  border: 1px solid var(--lc-border-subtle);
}
.stat-number {
  font-family: var(--lc-font-display);
  font-size: 20px;
  font-weight: 700;
  color: var(--lc-text);
  line-height: 1.1;
}
.stat-label {
  font-family: var(--lc-font-body);
  font-size: 11px;
  color: var(--lc-text-muted);
  margin-top: 2px;
  letter-spacing: 0.3px;
}
.stat-card.is-alert .stat-number {
  color: var(--lc-danger);
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
  background-color: var(--lc-accent-dim);
  border-left: 3px solid var(--lc-accent);
  padding-left: 5px;
}
.stats-bar-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  color: var(--lc-text);
}
.stats-bar-count {
  margin-left: auto;
  font-family: var(--lc-font-display);
  font-size: 12px;
  font-weight: 600;
  color: var(--lc-text-secondary);
}
.stats-bar-track {
  height: 5px;
  background: var(--lc-surface-3);
  border-radius: 3px;
  overflow: hidden;
}
.stats-bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.35s var(--lc-ease);
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

/* --- Shared dot styles --- */
.color-dot {
  display: inline-block;
  width: 10px;
  height: 10px;
  margin-right: 6px;
  border-radius: 50%;
  vertical-align: middle;
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

/* --- Dialog forms --- */
.basic-grid {
  display: grid;
  grid-template-columns: minmax(420px, 1.45fr) minmax(280px, 1fr);
  gap: 12px;
  align-items: start;
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
@media (max-width: 900px) {
  .basic-grid {
    grid-template-columns: 1fr;
  }
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
  .detail-empty-stats {
    grid-template-columns: 1fr;
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
  .todo-stats {
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
  .detail-grid,
  .detail-empty-stats {
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
.detail-scroll::-webkit-scrollbar,
.todo-sidebar::-webkit-scrollbar {
  width: 4px;
}
.todo-list-scroll::-webkit-scrollbar-thumb,
.detail-scroll::-webkit-scrollbar-thumb,
.todo-sidebar::-webkit-scrollbar-thumb {
  background: var(--lc-border);
  border-radius: 2px;
}
.todo-list-scroll::-webkit-scrollbar-thumb:hover,
.detail-scroll::-webkit-scrollbar-thumb:hover,
.todo-sidebar::-webkit-scrollbar-thumb:hover {
  background: var(--lc-border-hover);
}
.todo-list-scroll::-webkit-scrollbar-track,
.detail-scroll::-webkit-scrollbar-track,
.todo-sidebar::-webkit-scrollbar-track {
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

.toolbar-left .el-radio-group {
  --el-radio-button-checked-bg-color: var(--el-color-primary-light-9);
  --el-radio-button-checked-text-color: var(--el-color-primary);
  --el-radio-button-checked-border-color: var(--el-color-primary-light-5);
}

</style>

<style>
.todo-delete-scope-dialog {
  --el-messagebox-width: 580px;
}
</style>
