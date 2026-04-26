<template>
  <div class="pm-list-view">
    <!-- Toolbar: tag / date / groupBy / cols (search/type/priority/status come from PmPanel toolbar) -->
    <div class="pm-list-toolbar">
      <div class="pm-list-toolbar-inner">
        <el-select
          v-if="availableTags.length > 0"
          v-model="filters.tags"
          multiple
          collapse-tags
          collapse-tags-tooltip
          filterable
          placeholder="标签"
          size="default"
          class="toolbar-select"
        >
          <el-option v-for="tag in availableTags" :key="tag" :label="tag" :value="tag" />
        </el-select>

        <el-date-picker
          v-model="dateRangeModel"
          type="daterange"
          size="default"
          value-format="YYYY-MM-DD"
          range-separator="~"
          start-placeholder="起始时间"
          end-placeholder="截止时间"
          class="toolbar-date"
          :clearable="true"
        />

        <el-button
          size="default"
          class="toolbar-reset-btn"
          :disabled="!hasActiveFilters"
          @click="onClearFilters"
        >
          重置
        </el-button>

        <div class="toolbar-spacer" />

        <el-select
          :model-value="groupBy"
          size="default"
          class="toolbar-group"
          placeholder="分组"
          @update:model-value="(v) => setGroupBy(v as PmListGroupBy)"
        >
          <el-option label="不分组" value="none" />
          <el-option label="按项目" value="project" :disabled="!isOverview" />
          <el-option label="按状态" value="status" />
          <el-option label="按优先级" value="priority" />
          <el-option label="按标签" value="tag" />
        </el-select>

        <el-popover placement="bottom-end" trigger="click" :width="180">
          <template #reference>
            <el-button size="default" class="toolbar-col-btn">
              <el-icon><Grid /></el-icon>
              <span class="btn-label">列</span>
            </el-button>
          </template>
          <div class="cols-popover">
            <el-checkbox-group :model-value="visibleCols" @change="onToggleCols">
              <el-checkbox
                v-for="col in ALL_LIST_COLS"
                :key="col"
                :value="col"
                :disabled="col === 'title' || (col === 'project' && !isOverview)"
              >
                {{ COL_LABELS[col] }}
              </el-checkbox>
            </el-checkbox-group>
          </div>
        </el-popover>
      </div>
    </div>

    <div v-if="hasActiveFilters" class="pm-list-filter-bar">
      <span class="pm-list-filter-bar-label">已筛选：</span>
      <el-tag
        v-for="tag in filters.tags"
        :key="`tag-${tag}`"
        size="small"
        closable
        class="pm-list-filter-chip"
        @close="removeTagFilter(tag)"
      >
        标签：{{ tag }}
      </el-tag>
      <el-tag
        v-if="filters.dateRange"
        size="small"
        closable
        class="pm-list-filter-chip"
        @close="clearDateFilter"
      >
        日期：{{ filters.dateRange[0] }} ~ {{ filters.dateRange[1] }}
      </el-tag>
      <el-button size="small" text class="pm-list-filter-clear" @click="onClearFilters">
        清除全部
      </el-button>
    </div>

    <!-- Data area -->
    <div
      ref="scrollEl"
      class="pm-list-scroll"
      :class="{ 'has-batch': selectedIds.size > 0 }"
      @scroll="onScroll"
    >
      <div
        v-if="filteredItems.length === 0"
        class="pm-list-empty"
      >
        <el-empty :description="hasActiveFilters ? '无匹配工作项，试试清空筛选' : '暂无工作项'" />
      </div>
      <template v-else>
        <div v-for="group in groups" :key="group.key" class="pm-list-group">
          <div
            v-if="group.key !== 'all'"
            class="pm-list-group-header"
            @click="toggleGroup(group.key)"
          >
            <el-icon class="group-caret" :class="{ 'is-open': isGroupOpen(group.key) }">
              <CaretRight />
            </el-icon>
            <span v-if="group.color" class="group-color-dot" :style="{ backgroundColor: group.color }" />
            <span class="group-label">{{ group.label }}</span>
            <span class="group-count">{{ group.items.length }}</span>
            <span v-if="group.metrics" class="group-metrics">{{ group.metrics }}</span>
          </div>
          <el-table
            v-show="group.key === 'all' || isGroupOpen(group.key)"
            :ref="(el) => setTableRef(group.key, el)"
            :data="windowedItemsOf(group)"
            size="small"
            stripe
            row-key="id"
            empty-text="该组无数据"
            class="pm-list-table"
            :row-class-name="rowClassName"
            @selection-change="(rows) => onSelectionChange(group.key, rows)"
            @row-click="(row) => onRowClick(row, group.key)"
            @row-dblclick="onRowDblclick"
            @row-contextmenu="onRowContextmenu"
            @sort-change="onSortChange"
          >
            <el-table-column type="expand" width="36">
              <template #default="{ row }">
                <div class="row-expand">
                  <div class="row-expand-info">
                    <div v-if="row.description" class="row-expand-desc">
                      {{ row.description }}
                    </div>
                    <div v-else class="row-expand-desc is-empty">暂无描述</div>
                    <div class="row-expand-meta">
                      <span v-if="row.startAt" class="row-expand-meta-item">
                        <span class="meta-label">开始</span>
                        <span class="meta-value">{{ formatPmDateForDisplay(row.startAt, 'short') }}</span>
                      </span>
                      <span v-if="row.endAt" class="row-expand-meta-item">
                        <span class="meta-label">截止</span>
                        <span class="meta-value" :class="{ 'is-overdue': isPmItemOverdue(row) }">
                          {{ formatPmDateForDisplay(row.endAt, 'short') }}
                        </span>
                      </span>
                      <span v-if="row.startedAt" class="row-expand-meta-item">
                        <span class="meta-label">实际开始</span>
                        <span class="meta-value">{{ formatDateTime(row.startedAt) }}</span>
                      </span>
                      <span v-if="row.completedAt" class="row-expand-meta-item">
                        <span class="meta-label">完成时间</span>
                        <span class="meta-value">{{ formatDateTime(row.completedAt) }}</span>
                      </span>
                      <span class="row-expand-meta-item">
                        <span class="meta-label">更新</span>
                        <span class="meta-value">{{ formatDateTime(row.updatedAt) }}</span>
                      </span>
                    </div>
                  </div>
                  <div class="row-expand-actions">
                    <el-button
                      v-if="row.status === 'todo'"
                      size="small"
                      @click.stop="onQuickStart(row)"
                    >
                      开始做
                    </el-button>
                    <el-button
                      v-if="row.status !== 'done' && row.endAt"
                      size="small"
                      @click.stop="onQuickPostpone(row)"
                    >
                      推到明天
                    </el-button>
                    <el-button
                      v-if="row.status !== 'done'"
                      size="small"
                      type="success"
                      @click.stop="onQuickComplete(row)"
                    >
                      标记完成
                    </el-button>
                    <el-button size="small" @click.stop="emit('edit', row)">编辑</el-button>
                    <el-button size="small" type="primary" @click.stop="emit('select', row)">
                      打开详情面板
                    </el-button>
                  </div>
                </div>
              </template>
            </el-table-column>

            <el-table-column type="selection" width="42" :selectable="rowSelectable" />

            <el-table-column
              prop="title"
              label="标题"
              min-width="220"
              sortable="custom"
              show-overflow-tooltip
            >
              <template #default="{ row }">
                <div v-if="editingTitleId === row.id" class="cell-title-editor" @click.stop>
                  <el-input
                    :ref="(el) => setTitleInputRef(row.id, el)"
                    v-model="titleDraft"
                    size="small"
                    @keydown.enter.prevent="commitTitleEdit(row)"
                    @keydown.esc.prevent="cancelTitleEdit"
                    @blur="commitTitleEdit(row)"
                  />
                </div>
                <div v-else class="cell-title">
                  <span v-if="row.pinned" class="title-pin" title="已置顶">📌</span>
                  <span class="title-text">{{ row.title }}</span>
                  <el-icon
                    class="title-edit-icon"
                    title="编辑标题"
                    @click.stop="beginTitleEdit(row)"
                  >
                    <Edit />
                  </el-icon>
                </div>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('project') && isOverview"
              label="项目"
              min-width="140"
              prop="projectName"
              sortable="custom"
            >
              <template #default="{ row }">
                <template v-if="movableProjects.length > 0">
                  <template v-if="isEditorActive(row.id, 'project')">
                    <el-dropdown
                      :ref="(el) => setLazyDropdownRef(row.id, 'project', el)"
                      trigger="click"
                      @command="(cmd) => onInlineProject(row, cmd)"
                    >
                      <span
                        v-if="row.projectName"
                        class="cell-project cell-editable"
                        :style="{
                          backgroundColor: (row.projectColor || '#0ea5e9') + '18',
                          color: row.projectColor || '#0ea5e9',
                        }"
                        @click.stop
                      >
                        <span
                          class="cell-project-dot"
                          :style="{ backgroundColor: row.projectColor || '#0ea5e9' }"
                        />
                        {{ row.projectName }}
                      </span>
                      <span v-else class="cell-empty cell-editable" @click.stop>选择项目</span>
                      <template #dropdown>
                        <el-dropdown-menu>
                          <el-dropdown-item
                            v-for="project in movableProjects"
                            :key="project.id"
                            :command="project.id"
                            :disabled="row.projectId === project.id"
                          >
                            <span
                              class="cell-project-dot"
                              :style="{ backgroundColor: project.color || '#0ea5e9', marginRight: '6px' }"
                            />
                            {{ project.name }}
                          </el-dropdown-item>
                        </el-dropdown-menu>
                      </template>
                    </el-dropdown>
                  </template>
                  <span
                    v-else-if="row.projectName"
                    class="cell-project cell-editable"
                    :style="{
                      backgroundColor: (row.projectColor || '#0ea5e9') + '18',
                      color: row.projectColor || '#0ea5e9',
                    }"
                    @click.stop="activateDropdown(row.id, 'project')"
                  >
                    <span
                      class="cell-project-dot"
                      :style="{ backgroundColor: row.projectColor || '#0ea5e9' }"
                    />
                    {{ row.projectName }}
                  </span>
                  <span
                    v-else
                    class="cell-empty cell-editable"
                    @click.stop="activateDropdown(row.id, 'project')"
                  >
                    选择项目
                  </span>
                </template>
                <span
                  v-else-if="row.projectName"
                  class="cell-project"
                  :style="{
                    backgroundColor: (row.projectColor || '#0ea5e9') + '18',
                    color: row.projectColor || '#0ea5e9',
                  }"
                >
                  <span
                    class="cell-project-dot"
                    :style="{ backgroundColor: row.projectColor || '#0ea5e9' }"
                  />
                  {{ row.projectName }}
                </span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('refCode')"
              label="编号"
              width="110"
              prop="refCode"
            >
              <template #default="{ row }">
                <span v-if="row.refCode" class="cell-ref-code">{{ row.refCode }}</span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('itemType')"
              label="类型"
              width="92"
              prop="itemType"
              sortable="custom"
            >
              <template #default="{ row }">
                <span
                  class="cell-pill"
                  :style="{
                    color: PM_ITEM_TYPE_MAP[row.itemType]?.color,
                    borderColor: PM_ITEM_TYPE_MAP[row.itemType]?.color + '40',
                  }"
                >
                  {{ PM_ITEM_TYPE_MAP[row.itemType]?.label }}
                </span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('priority')"
              label="优先级"
              width="100"
              prop="priority"
              sortable="custom"
            >
              <template #default="{ row }">
                <template v-if="isEditorActive(row.id, 'priority')">
                  <el-dropdown
                    :ref="(el) => setLazyDropdownRef(row.id, 'priority', el)"
                    trigger="click"
                    @command="(cmd) => onInlinePriority(row, cmd)"
                  >
                    <span
                      class="cell-pill cell-editable"
                      :style="{
                        color: PM_PRIORITY_MAP[row.priority]?.color,
                        borderColor: PM_PRIORITY_MAP[row.priority]?.color + '40',
                      }"
                      @click.stop
                    >
                      {{ PM_PRIORITY_MAP[row.priority]?.label }}
                    </span>
                    <template #dropdown>
                      <el-dropdown-menu>
                        <el-dropdown-item
                          v-for="(meta, key) in PM_PRIORITY_MAP"
                          :key="key"
                          :command="key"
                          :disabled="row.priority === key"
                        >
                          <span
                            class="cell-pill"
                            :style="{
                              color: meta.color,
                              borderColor: meta.color + '40',
                            }"
                          >
                            {{ meta.label }}
                          </span>
                        </el-dropdown-item>
                      </el-dropdown-menu>
                    </template>
                  </el-dropdown>
                </template>
                <span
                  v-else
                  class="cell-pill cell-editable"
                  :style="{
                    color: PM_PRIORITY_MAP[row.priority]?.color,
                    borderColor: PM_PRIORITY_MAP[row.priority]?.color + '40',
                  }"
                  @click.stop="activateDropdown(row.id, 'priority')"
                >
                  {{ PM_PRIORITY_MAP[row.priority]?.label }}
                </span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('status')"
              label="状态"
              width="100"
              prop="status"
              sortable="custom"
            >
              <template #default="{ row }">
                <template v-if="isEditorActive(row.id, 'status')">
                  <el-dropdown
                    :ref="(el) => setLazyDropdownRef(row.id, 'status', el)"
                    trigger="click"
                    @command="(cmd) => onInlineStatus(row, cmd)"
                  >
                    <span
                      class="cell-pill cell-editable"
                      :style="{
                        color: statusMeta(row.status).color,
                        borderColor: statusMeta(row.status).color + '40',
                      }"
                      @click.stop
                    >
                      {{ statusMeta(row.status).label }}
                    </span>
                    <template #dropdown>
                      <el-dropdown-menu>
                        <el-dropdown-item
                          v-for="col in PM_STATUS_COLUMNS"
                          :key="col.key"
                          :command="col.key"
                          :disabled="row.status === col.key"
                        >
                          <span
                            class="cell-pill"
                            :style="{
                              color: col.color,
                              borderColor: col.color + '40',
                            }"
                          >
                            {{ col.label }}
                          </span>
                        </el-dropdown-item>
                      </el-dropdown-menu>
                    </template>
                  </el-dropdown>
                </template>
                <span
                  v-else
                  class="cell-pill cell-editable"
                  :style="{
                    color: statusMeta(row.status).color,
                    borderColor: statusMeta(row.status).color + '40',
                  }"
                  @click.stop="activateDropdown(row.id, 'status')"
                >
                  {{ statusMeta(row.status).label }}
                </span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('endAt')"
              label="截止"
              width="130"
              prop="endAt"
              sortable="custom"
            >
              <template #default="{ row }">
                <el-popover
                  v-if="isEditorActive(row.id, 'endAt')"
                  :visible="lazyPopoverVisible[editorKey(row.id, 'endAt')]"
                  trigger="click"
                  placement="bottom-start"
                  :width="260"
                  :popper-options="{ modifiers: [{ name: 'preventOverflow', enabled: true }] }"
                  @update:visible="(v) => (lazyPopoverVisible[editorKey(row.id, 'endAt')] = v)"
                >
                  <template #reference>
                    <span class="cell-date-trigger" @click.stop>
                      <span
                        v-if="row.endAt"
                        class="cell-date"
                        :class="{ 'is-overdue': isPmItemOverdue(row) }"
                      >
                        {{ formatPmDateForDisplay(row.endAt, 'short') }}
                      </span>
                      <span v-else class="cell-empty">设置日期</span>
                    </span>
                  </template>
                  <div class="inline-date-editor">
                    <el-date-picker
                      :model-value="row.endAt"
                      type="date"
                      value-format="YYYY-MM-DD"
                      placeholder="选择截止日期"
                      size="small"
                      style="width: 100%;"
                      @update:model-value="(val) => onInlineEndAt(row, val as string | null)"
                    />
                    <el-button
                      v-if="row.endAt"
                      size="small"
                      text
                      class="inline-date-clear"
                      @click="onInlineEndAt(row, null)"
                    >
                      清除
                    </el-button>
                  </div>
                </el-popover>
                <span
                  v-else
                  class="cell-date-trigger"
                  @click.stop="activatePopover(row.id, 'endAt')"
                >
                  <span
                    v-if="row.endAt"
                    class="cell-date"
                    :class="{ 'is-overdue': isPmItemOverdue(row) }"
                  >
                    {{ formatPmDateForDisplay(row.endAt, 'short') }}
                  </span>
                  <span v-else class="cell-empty">设置日期</span>
                </span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('tags')"
              label="标签"
              min-width="160"
            >
              <template #default="{ row }">
                <el-popover
                  v-if="isEditorActive(row.id, 'tags')"
                  :visible="lazyPopoverVisible[editorKey(row.id, 'tags')]"
                  trigger="click"
                  placement="bottom-start"
                  :width="260"
                  @update:visible="(v) => (lazyPopoverVisible[editorKey(row.id, 'tags')] = v)"
                >
                  <template #reference>
                    <span class="cell-tags" @click.stop>
                      <el-tag
                        v-for="tag in (row.tags || []).slice(0, 3)"
                        :key="tag"
                        size="small"
                        class="cell-tag"
                      >
                        {{ tag }}
                      </el-tag>
                      <span v-if="(row.tags || []).length > 3" class="tag-more">
                        +{{ row.tags.length - 3 }}
                      </span>
                      <span v-if="(row.tags || []).length === 0" class="cell-empty">
                        添加标签
                      </span>
                    </span>
                  </template>
                  <el-select
                    :model-value="row.tags"
                    multiple
                    filterable
                    allow-create
                    default-first-option
                    placeholder="输入标签后回车"
                    size="small"
                    style="width: 100%;"
                    @update:model-value="(val) => onInlineTags(row, val as string[])"
                  >
                    <el-option v-for="tag in availableTags" :key="tag" :label="tag" :value="tag" />
                  </el-select>
                </el-popover>
                <span
                  v-else
                  class="cell-tags"
                  @click.stop="activatePopover(row.id, 'tags')"
                >
                  <el-tag
                    v-for="tag in (row.tags || []).slice(0, 3)"
                    :key="tag"
                    size="small"
                    class="cell-tag"
                  >
                    {{ tag }}
                  </el-tag>
                  <span v-if="(row.tags || []).length > 3" class="tag-more">
                    +{{ row.tags.length - 3 }}
                  </span>
                  <span v-if="(row.tags || []).length === 0" class="cell-empty">
                    添加标签
                  </span>
                </span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('updatedAt')"
              label="更新"
              width="110"
              prop="updatedAt"
              sortable="custom"
            >
              <template #default="{ row }">
                <span class="cell-date">{{ formatUpdatedAt(row.updatedAt) }}</span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('startAt')"
              label="开始"
              width="110"
              prop="startAt"
              sortable="custom"
            >
              <template #default="{ row }">
                <span v-if="row.startAt" class="cell-date">
                  {{ formatPmDateForDisplay(row.startAt, 'short') }}
                </span>
                <span v-else class="cell-empty">—</span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('startedAt')"
              label="实际开始"
              width="130"
              prop="startedAt"
            >
              <template #default="{ row }">
                <span v-if="row.startedAt" class="cell-date">{{ formatDateTime(row.startedAt) }}</span>
                <span v-else class="cell-empty">—</span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('completedAt')"
              label="完成时间"
              width="130"
              prop="completedAt"
            >
              <template #default="{ row }">
                <span v-if="row.completedAt" class="cell-date">{{ formatDateTime(row.completedAt) }}</span>
                <span v-else class="cell-empty">—</span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('description')"
              label="描述摘要"
              min-width="200"
            >
              <template #default="{ row }">
                <span v-if="row.description" class="cell-desc" :title="row.description">
                  {{ truncateDesc(row.description) }}
                </span>
                <span v-else class="cell-empty">—</span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('linkUrl')"
              label="链接"
              min-width="140"
            >
              <template #default="{ row }">
                <a
                  v-if="row.linkUrl"
                  :href="row.linkUrl"
                  class="cell-link"
                  :title="row.linkUrl"
                  target="_blank"
                  rel="noreferrer"
                  @click.stop
                >
                  {{ shortenLink(row.linkUrl) }}
                </a>
                <span v-else class="cell-empty">—</span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('todoCount')"
              label="Todo 数"
              width="92"
              prop="todoCount"
              sortable="custom"
            >
              <template #default="{ row }">
                <span class="cell-todo-count">{{ row.todoCount ?? 0 }}</span>
              </template>
            </el-table-column>
          </el-table>
        </div>
        <div v-if="virtualActive && renderedTotal < filteredItems.length" class="pm-list-more-hint">
          已加载 {{ renderedTotal }} / {{ filteredItems.length }} 项，继续滚动加载更多
        </div>
      </template>
    </div>

    <!-- Batch bar -->
    <Transition name="pm-list-batch-slide">
      <div v-if="selectedIds.size > 0" class="pm-list-batch-bar">
        <div class="batch-info">
          <span class="batch-count">已选 {{ selectedIds.size }} 项</span>
          <el-button size="small" text @click="clearSelection">清除</el-button>
        </div>
        <div class="batch-actions">
          <el-button size="small" type="success" @click="onBatchComplete">标记完成</el-button>

          <el-dropdown trigger="click" @command="onBatchStatus">
            <el-button size="small">
              改状态
              <el-icon class="dropdown-caret"><CaretBottom /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item
                  v-for="col in PM_STATUS_COLUMNS"
                  :key="col.key"
                  :command="col.key"
                >
                  {{ col.label }}
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <el-dropdown trigger="click" @command="onBatchPriority">
            <el-button size="small">
              改优先级
              <el-icon class="dropdown-caret"><CaretBottom /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item
                  v-for="(meta, key) in PM_PRIORITY_MAP"
                  :key="key"
                  :command="key"
                >
                  {{ meta.label }}
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <el-dropdown
            v-if="movableProjects.length > 0"
            trigger="click"
            @command="onBatchProject"
          >
            <el-button size="small">
              改项目
              <el-icon class="dropdown-caret"><CaretBottom /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item
                  v-for="project in movableProjects"
                  :key="project.id"
                  :command="project.id"
                >
                  {{ project.name }}
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <el-popover
            v-model:visible="batchTagPopoverVisible"
            trigger="click"
            placement="top"
            :width="280"
          >
            <template #reference>
              <el-button size="small">打标签</el-button>
            </template>
            <div class="batch-tag-popover">
              <el-select
                v-model="batchTagDraft"
                multiple
                filterable
                allow-create
                default-first-option
                placeholder="输入或选择标签"
                size="small"
                style="width: 100%;"
              >
                <el-option v-for="tag in availableTags" :key="tag" :label="tag" :value="tag" />
              </el-select>
              <div class="batch-tag-popover-actions">
                <el-button size="small" @click="cancelBatchTag">取消</el-button>
                <el-button
                  size="small"
                  type="primary"
                  :disabled="batchTagDraft.length === 0"
                  @click="confirmBatchTag"
                >
                  追加
                </el-button>
              </div>
            </div>
          </el-popover>

          <el-button size="small" @click="onBatchPin(true)">置顶</el-button>
          <el-button size="small" @click="onBatchPin(false)">取消置顶</el-button>
          <el-button size="small" type="danger" @click="onBatchDelete">删除</el-button>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, nextTick } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { CaretBottom, CaretRight, Edit, Grid } from "@element-plus/icons-vue";
import type { PmItem, PmItemStatus, PmPriority, PmProject } from "../types/pm";
import {
  PM_ITEM_TYPE_MAP,
  PM_PRIORITY_MAP,
  PM_STATUS_COLUMNS,
} from "../types/pm";
import { useToolInvoke } from "../composables/useToolInvoke";
import { formatPmDateForDisplay } from "../utils/pmDate";
import { isPmItemOverdue } from "../utils/pmDate";
import {
  usePmListPrefs,
  ALL_LIST_COLS,
  COL_LABELS,
  type PmListColId,
  type PmListGroupBy,
} from "../composables/usePmListPrefs";
import type { PmContextId } from "../composables/usePmViewMemory";

interface SortState {
  prop: string | null;
  order: "asc" | "desc" | null;
}

interface GroupItem {
  key: string;
  label: string;
  color?: string | null;
  items: PmItem[];
  metrics?: string;
}

const props = defineProps<{
  items: PmItem[];
  projects: PmProject[];
  selectedItemId: number | null;
  isOverview: boolean;
  selectedProjectId: number | "overview" | null;
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", event: MouseEvent, item: PmItem): void;
  (e: "items-changed"): void;
}>();

const { invoke } = useToolInvoke();

// Preferences
const contextRef = computed<PmContextId | null>(() => {
  if (props.selectedProjectId === null) return null;
  return props.selectedProjectId;
});
const { visibleCols, filters, groupBy, setVisibleCols, setFilters, setGroupBy, resetFilters } =
  usePmListPrefs(contextRef);

// Overview 切换时若分组为 project 且当前不是 overview，降级为 none
watch(
  () => props.isOverview,
  (ov) => {
    if (!ov && groupBy.value === "project") {
      setGroupBy("none");
    }
  },
  { immediate: true },
);

// Lazy editor activation (dropdown / popover 按需挂载，避免 23 行 × 5 列一次性渲染 400+ 组件)
const activatedEditors = ref(new Set<string>());
const lazyDropdownRefs = new Map<string, { handleOpen?: () => void }>();
const lazyPopoverVisible = ref<Record<string, boolean>>({});

function editorKey(rowId: number, field: string): string {
  return `${rowId}:${field}`;
}
function isEditorActive(rowId: number, field: string): boolean {
  return activatedEditors.value.has(editorKey(rowId, field));
}
function setLazyDropdownRef(rowId: number, field: string, el: unknown) {
  const key = editorKey(rowId, field);
  if (el) lazyDropdownRefs.set(key, el as { handleOpen?: () => void });
  else lazyDropdownRefs.delete(key);
}
async function activateDropdown(rowId: number, field: string) {
  const key = editorKey(rowId, field);
  activatedEditors.value.add(key);
  // 双 nextTick：等 el-dropdown + 内部 ElTooltip 都就位
  await nextTick();
  await nextTick();
  lazyDropdownRefs.get(key)?.handleOpen?.();
}
async function activatePopover(rowId: number, field: string) {
  const key = editorKey(rowId, field);
  activatedEditors.value.add(key);
  await nextTick();
  lazyPopoverVisible.value[key] = true;
}

// Selection
const tableRefs = ref<Map<string, any>>(new Map());
function setTableRef(key: string, el: unknown) {
  if (el) tableRefs.value.set(key, el);
  else tableRefs.value.delete(key);
}
const selectionMap = ref<Map<string, Set<number>>>(new Map());
const selectedIds = computed<Set<number>>(() => {
  const all = new Set<number>();
  for (const set of selectionMap.value.values()) {
    for (const id of set) all.add(id);
  }
  return all;
});

// Sort
const sortState = ref<SortState>({ prop: null, order: null });

// Progressive rendering (Phase 4.2) — declared after `filteredItems` below
const VIRTUAL_THRESHOLD = 500;
const VIRTUAL_CHUNK = 200;
const SCROLL_TRIGGER_PX = 240;
const scrollEl = ref<HTMLElement | null>(null);
const renderLimit = ref(VIRTUAL_CHUNK);

function statusMeta(status: PmItemStatus) {
  return PM_STATUS_COLUMNS.find((c) => c.key === status) ?? { label: status, color: "#909399" };
}

function formatUpdatedAt(value: string): string {
  if (!value) return "-";
  return value.slice(0, 10);
}

function formatDateTime(value: string | null | undefined): string {
  if (!value) return "-";
  if (value.length >= 16) return value.replace("T", " ").slice(0, 16);
  return value.slice(0, 10);
}

function truncateDesc(value: string): string {
  const trimmed = value.replace(/\s+/g, " ").trim();
  if (trimmed.length <= 40) return trimmed;
  return trimmed.slice(0, 40) + "…";
}

function shortenLink(url: string): string {
  try {
    const u = new URL(url);
    const host = u.hostname.replace(/^www\./, "");
    const path = u.pathname === "/" ? "" : u.pathname;
    return host + path;
  } catch {
    return url.length > 30 ? url.slice(0, 30) + "…" : url;
  }
}

function rowClassName({ row }: { row: PmItem }) {
  return row.id === props.selectedItemId ? "is-selected-row" : "";
}

function rowSelectable(row: PmItem): boolean {
  return row.status !== undefined;
}

const priorityRank: Record<PmPriority, number> = { P0: 0, P1: 1, P2: 2, P3: 3 };
const statusRank: Record<PmItemStatus, number> = {
  todo: 0,
  in_progress: 1,
  testing: 2,
  done: 3,
};

function sortValue(item: PmItem, prop: string): string | number | null {
  switch (prop) {
    case "title":
      return item.title.toLowerCase();
    case "projectName":
      return (item.projectName ?? "").toLowerCase();
    case "itemType":
      return item.itemType;
    case "priority":
      return priorityRank[item.priority] ?? 99;
    case "status":
      return statusRank[item.status] ?? 99;
    case "endAt":
      return item.endAt ?? null;
    case "startAt":
      return item.startAt ?? null;
    case "updatedAt":
      return item.updatedAt ?? null;
    case "todoCount":
      return item.todoCount ?? 0;
    default:
      return null;
  }
}

function sortedItemsOf(list: PmItem[]): PmItem[] {
  const { prop, order } = sortState.value;
  if (!prop || !order) return defaultSorted(list);
  const dir = order === "asc" ? 1 : -1;
  return [...list].sort((a, b) => {
    const va = sortValue(a, prop);
    const vb = sortValue(b, prop);
    if (va === vb) return 0;
    if (va === null || va === undefined) return 1;
    if (vb === null || vb === undefined) return -1;
    if (typeof va === "number" && typeof vb === "number") return (va - vb) * dir;
    return String(va).localeCompare(String(vb)) * dir;
  });
}

function defaultSorted(list: PmItem[]): PmItem[] {
  return [...list].sort((a, b) => {
    const pa = a.pinned ? 1 : 0;
    const pb = b.pinned ? 1 : 0;
    if (pa !== pb) return pb - pa;
    const prA = priorityRank[a.priority] ?? 99;
    const prB = priorityRank[b.priority] ?? 99;
    if (prA !== prB) return prA - prB;
    const eA = a.endAt ?? null;
    const eB = b.endAt ?? null;
    if (eA !== eB) {
      if (eA === null) return 1;
      if (eB === null) return -1;
      return eA.localeCompare(eB);
    }
    const uA = a.updatedAt ?? "";
    const uB = b.updatedAt ?? "";
    return uB.localeCompare(uA);
  });
}

function onSortChange(payload: { prop: string; order: "ascending" | "descending" | null }) {
  if (!payload.order) {
    sortState.value = { prop: null, order: null };
  } else {
    sortState.value = {
      prop: payload.prop,
      order: payload.order === "ascending" ? "asc" : "desc",
    };
  }
}

function onSelectionChange(groupKey: string, rows: PmItem[]) {
  selectionMap.value.set(groupKey, new Set(rows.map((r) => r.id)));
  // trigger reactivity
  selectionMap.value = new Map(selectionMap.value);
}

function onRowClick(row: PmItem, groupKey: string) {
  const table = tableRefs.value.get(groupKey);
  table?.toggleRowExpansion?.(row);
}

function onRowDblclick(row: PmItem) {
  emit("edit", row);
}

function onRowContextmenu(row: PmItem, _column: unknown, event: MouseEvent) {
  emit("item-context", event, row);
}

function clearSelection() {
  for (const [, table] of tableRefs.value) {
    table?.clearSelection?.();
  }
  selectionMap.value = new Map();
}

// Column visibility
function colVisible(id: PmListColId): boolean {
  return visibleCols.value.includes(id);
}
function onToggleCols(next: PmListColId[] | (string | number | boolean)[]) {
  const cleaned = (next as unknown as string[]).filter((v) =>
    (ALL_LIST_COLS as string[]).includes(v),
  ) as PmListColId[];
  if (!cleaned.includes("title")) cleaned.unshift("title");
  setVisibleCols(cleaned);
}

// Filters
const dateRangeModel = computed<[string, string] | null>({
  get: () => filters.value.dateRange,
  set: (val) => {
    setFilters({ ...filters.value, dateRange: val ?? null });
  },
});

watch(
  () => ({ ...filters.value }),
  (next, prev) => {
    if (!prev) return;
    if (JSON.stringify(next) !== JSON.stringify(prev)) {
      setFilters(next);
    }
  },
  { deep: true },
);

const hasActiveFilters = computed(() => {
  const f = filters.value;
  return !!(f.tags.length || f.dateRange);
});

function onClearFilters() {
  resetFilters();
}

function removeTagFilter(tag: string) {
  const next = filters.value.tags.filter((t) => t !== tag);
  setFilters({ ...filters.value, tags: next });
}

function clearDateFilter() {
  setFilters({ ...filters.value, dateRange: null });
}

const availableTags = computed<string[]>(() => {
  const set = new Set<string>();
  for (const item of props.items) {
    for (const tag of item.tags || []) {
      set.add(tag);
    }
  }
  return Array.from(set).sort((a, b) => a.localeCompare(b));
});

const filteredItems = computed<PmItem[]>(() => {
  const f = filters.value;
  return props.items.filter((item) => {
    if (f.tags.length > 0) {
      const itemTags = new Set(item.tags || []);
      const allHit = f.tags.every((t) => itemTags.has(t));
      if (!allHit) return false;
    }
    if (f.dateRange) {
      const [start, end] = f.dateRange;
      const inRange = (d: string | null): boolean =>
        d !== null && d >= start && d <= end;
      if (!inRange(item.startAt) && !inRange(item.endAt)) return false;
    }
    return true;
  });
});

const virtualActive = computed(
  () => groupBy.value === "none" && filteredItems.value.length > VIRTUAL_THRESHOLD,
);

function windowedItemsOf(group: GroupItem): PmItem[] {
  const sorted = sortedItemsOf(group.items);
  if (!virtualActive.value) return sorted;
  return sorted.slice(0, renderLimit.value);
}

const renderedTotal = computed<number>(() => {
  if (!virtualActive.value) return filteredItems.value.length;
  return Math.min(renderLimit.value, filteredItems.value.length);
});

function onScroll() {
  if (!virtualActive.value) return;
  const el = scrollEl.value;
  if (!el) return;
  const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
  if (distanceToBottom > SCROLL_TRIGGER_PX) return;
  if (renderLimit.value >= filteredItems.value.length) return;
  renderLimit.value = Math.min(
    renderLimit.value + VIRTUAL_CHUNK,
    filteredItems.value.length,
  );
}

watch(
  [
    () => filteredItems.value.length,
    () => sortState.value.prop,
    () => sortState.value.order,
    () => groupBy.value,
  ],
  () => {
    renderLimit.value = VIRTUAL_CHUNK;
    nextTick(() => {
      scrollEl.value?.scrollTo({ top: 0 });
    });
  },
);

// Groups
const groupExpanded = ref<Record<string, boolean>>({});
function isGroupOpen(key: string): boolean {
  return groupExpanded.value[key] !== false;
}
function toggleGroup(key: string) {
  groupExpanded.value[key] = !isGroupOpen(key);
}

const groups = computed<GroupItem[]>(() => {
  if (groupBy.value === "none") {
    return [{ key: "all", label: "", items: filteredItems.value }];
  }
  const buckets = new Map<string, GroupItem>();
  for (const item of filteredItems.value) {
    let gkey: string;
    let label: string;
    let color: string | null = null;
    switch (groupBy.value) {
      case "project": {
        const pid = item.projectId ?? 0;
        gkey = `project-${pid}`;
        label = item.projectName ?? `项目 #${pid}`;
        color = item.projectColor ?? null;
        break;
      }
      case "status": {
        gkey = `status-${item.status}`;
        label = statusMeta(item.status).label;
        color = statusMeta(item.status).color;
        break;
      }
      case "priority": {
        gkey = `priority-${item.priority}`;
        label = PM_PRIORITY_MAP[item.priority]?.label ?? item.priority;
        color = PM_PRIORITY_MAP[item.priority]?.color ?? null;
        break;
      }
      case "tag": {
        const tags = item.tags && item.tags.length > 0 ? item.tags : ["(无标签)"];
        for (const tag of tags) {
          const key = `tag-${tag}`;
          if (!buckets.has(key)) {
            buckets.set(key, { key, label: tag, items: [] });
          }
          buckets.get(key)!.items.push(item);
        }
        continue;
      }
      default:
        gkey = "all";
        label = "";
    }
    if (!buckets.has(gkey)) {
      buckets.set(gkey, { key: gkey, label, color, items: [] });
    }
    buckets.get(gkey)!.items.push(item);
  }
  const list = Array.from(buckets.values());
  for (const g of list) {
    g.metrics = buildGroupMetrics(g.items);
  }
  list.sort((a, b) => {
    if (groupBy.value === "priority") {
      const rank = (k: string): number => {
        const p = k.replace("priority-", "") as PmPriority;
        return priorityRank[p] ?? 99;
      };
      return rank(a.key) - rank(b.key);
    }
    if (groupBy.value === "status") {
      const rank = (k: string): number => {
        const s = k.replace("status-", "") as PmItemStatus;
        return statusRank[s] ?? 99;
      };
      return rank(a.key) - rank(b.key);
    }
    return a.label.localeCompare(b.label);
  });
  return list;
});

// Keep selection stable across item refresh
watch(
  () => props.items.map((i) => i.id).join(","),
  () => {
    nextTick(() => {
      const ids = new Set(props.items.map((i) => i.id));
      const next = new Map<string, Set<number>>();
      for (const [gkey, set] of selectionMap.value) {
        const retained = new Set<number>();
        for (const id of set) {
          if (ids.has(id)) retained.add(id);
        }
        if (retained.size > 0) next.set(gkey, retained);
      }
      selectionMap.value = next;
      for (const [gkey, table] of tableRefs.value) {
        table?.clearSelection?.();
        const retained = next.get(gkey);
        if (!retained) continue;
        const group = groups.value.find((g) => g.key === gkey);
        const rows = (group?.items ?? []).filter((i) => retained.has(i.id));
        rows.forEach((row) => table.toggleRowSelection?.(row, true));
      }
    });
  },
);

// Movable projects
const movableProjects = computed(() => {
  return props.projects.filter((p) => p.status === "active");
});

function buildGroupMetrics(list: PmItem[]): string {
  if (list.length === 0) return "";
  const todayStr = toLocalDateStr(new Date());
  let overdue = 0;
  let inProgressCount = 0;
  let dueTodayCount = 0;
  for (const item of list) {
    if (item.status === "done") continue;
    const end = (item.endAt ?? "").slice(0, 10);
    if (end && end < todayStr) overdue += 1;
    else if (end && end === todayStr) dueTodayCount += 1;
    if (item.status === "in_progress") inProgressCount += 1;
  }
  const parts: string[] = [];
  if (overdue > 0) parts.push(`逾期 ${overdue}`);
  if (dueTodayCount > 0) parts.push(`今日 ${dueTodayCount}`);
  if (inProgressCount > 0) parts.push(`进行中 ${inProgressCount}`);
  return parts.join(" · ");
}

function toLocalDateStr(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

// Inline edit
async function onInlineStatus(row: PmItem, command: unknown) {
  const status = command as PmItemStatus;
  if (row.status === status) return;
  try {
    await invoke("tool:pm:item-change-status", { id: row.id, status });
    ElMessage.success({ message: `已改为「${statusMeta(status).label}」`, duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onInlinePriority(row: PmItem, command: unknown) {
  const priority = command as PmPriority;
  if (row.priority === priority) return;
  try {
    await invoke("tool:pm:item-update", { id: row.id, priority });
    ElMessage.success({ message: `已改为 ${PM_PRIORITY_MAP[priority].label}`, duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onInlineEndAt(row: PmItem, value: string | null) {
  if ((row.endAt ?? null) === (value ?? null)) return;
  try {
    await invoke("tool:pm:item-update", { id: row.id, endAt: value });
    ElMessage.success({ message: value ? "已更新截止日期" : "已清除截止日期", duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onInlineTags(row: PmItem, tags: string[]) {
  const current = (row.tags ?? []).slice().sort();
  const next = tags.slice().sort();
  if (current.join("|") === next.join("|")) return;
  try {
    await invoke("tool:pm:item-update", { id: row.id, tags });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onInlineProject(row: PmItem, command: unknown) {
  const projectId = command as number;
  if (row.projectId === projectId) return;
  try {
    await invoke("tool:pm:item-move-project", { id: row.id, projectId });
    const target = props.projects.find((p) => p.id === projectId);
    ElMessage.success({ message: `已移至「${target?.name ?? projectId}」`, duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

const editingTitleId = ref<number | null>(null);
const titleDraft = ref("");
const titleInputRefs = new Map<number, { focus?: () => void } | null>();

function setTitleInputRef(id: number, el: unknown) {
  titleInputRefs.set(id, el as { focus?: () => void } | null);
  if (editingTitleId.value === id) {
    nextTick(() => {
      const inst = titleInputRefs.get(id);
      inst?.focus?.();
    });
  }
}

function beginTitleEdit(row: PmItem) {
  editingTitleId.value = row.id;
  titleDraft.value = row.title;
  nextTick(() => {
    const inst = titleInputRefs.get(row.id);
    inst?.focus?.();
  });
}

function cancelTitleEdit() {
  editingTitleId.value = null;
  titleDraft.value = "";
}

async function commitTitleEdit(row: PmItem) {
  if (editingTitleId.value !== row.id) return;
  const next = titleDraft.value.trim();
  editingTitleId.value = null;
  titleDraft.value = "";
  if (!next || next === row.title) return;
  try {
    await invoke("tool:pm:item-update", { id: row.id, title: next });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onQuickStart(row: PmItem) {
  try {
    await invoke("tool:pm:item-change-status", { id: row.id, status: "in_progress" });
    ElMessage.success({ message: "已开始", duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onQuickComplete(row: PmItem) {
  try {
    await invoke("tool:pm:item-change-status", { id: row.id, status: "done" });
    ElMessage.success({ message: "已标记完成", duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onQuickPostpone(row: PmItem) {
  const currentEnd = row.endAt;
  if (!currentEnd) return;
  const prefix = currentEnd.length >= 10 ? currentEnd.slice(0, 10) : currentEnd;
  const parts = prefix.split("-");
  if (parts.length !== 3) return;
  const date = new Date(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2]));
  date.setDate(date.getDate() + 1);
  const nextEnd = toLocalDateStr(date);
  const nextStart = row.startAt && row.startAt > nextEnd ? nextEnd : row.startAt;
  try {
    await invoke("tool:pm:item-update", {
      id: row.id,
      startAt: nextStart,
      endAt: nextEnd,
    });
    ElMessage.success({ message: "已推到明天", duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

// Batch ops
async function runBatch(fields: Record<string, unknown>, successMsg: string) {
  if (selectedIds.value.size === 0) return;
  const ids = Array.from(selectedIds.value);
  try {
    const result = (await invoke<{ updated: number }>("tool:pm:item-batch-update", {
      ids,
      fields,
    })) ?? { updated: 0 };
    ElMessage.success({ message: `${successMsg}（${result.updated} 项）`, duration: 1500 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onBatchStatus(command: unknown) {
  const status = command as PmItemStatus;
  await runBatch({ status }, `已改为「${statusMeta(status).label}」`);
}

async function onBatchPriority(command: unknown) {
  const priority = command as PmPriority;
  await runBatch({ priority }, `已改为 ${PM_PRIORITY_MAP[priority].label}`);
}

async function onBatchProject(command: unknown) {
  const projectId = command as number;
  const target = props.projects.find((p) => p.id === projectId);
  await runBatch({ projectId }, `已移至「${target?.name ?? projectId}」`);
}

async function onBatchPin(pinned: boolean) {
  await runBatch({ pinned }, pinned ? "已置顶" : "已取消置顶");
}

async function onBatchComplete() {
  await runBatch({ status: "done" }, "已标记完成");
}

const batchTagPopoverVisible = ref(false);
const batchTagDraft = ref<string[]>([]);

function cancelBatchTag() {
  batchTagDraft.value = [];
  batchTagPopoverVisible.value = false;
}

async function confirmBatchTag() {
  if (batchTagDraft.value.length === 0) return;
  const tags = batchTagDraft.value.slice();
  await runBatch({ addTags: tags }, `已追加 ${tags.length} 个标签`);
  batchTagDraft.value = [];
  batchTagPopoverVisible.value = false;
}

async function onBatchDelete() {
  if (selectedIds.value.size === 0) return;
  try {
    await ElMessageBox.confirm(
      `确定删除选中的 ${selectedIds.value.size} 项工作项？`,
      "批量删除确认",
      { type: "warning" },
    );
  } catch {
    return;
  }
  const ids = Array.from(selectedIds.value);
  let success = 0;
  for (const id of ids) {
    try {
      await invoke("tool:pm:item-delete", { id });
      success += 1;
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }
  if (success > 0) {
    ElMessage.success({ message: `已删除 ${success} 项`, duration: 1500 });
    emit("items-changed");
  }
}
</script>

<style scoped>
.pm-list-view {
  position: relative;
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--lc-surface-1);
  border-radius: var(--lc-radius-lg);
}

/* ---------- Toolbar ---------- */
.pm-list-toolbar {
  padding: 12px 16px;
  background: var(--lc-surface-1);
}
.pm-list-toolbar-inner {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  padding: 10px 14px;
  border-radius: 18px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.95), rgba(245, 249, 255, 0.9)),
    radial-gradient(circle at top left, rgba(14, 165, 233, 0.07), transparent 36%);
  border: 1px solid rgba(255, 255, 255, 0.92);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.8);
}
.toolbar-select {
  width: 140px;
  min-width: 110px;
}
.toolbar-select :deep(.el-select__wrapper) {
  min-height: 36px;
  padding: 0 10px;
  border-radius: 12px;
  border-color: rgba(14, 165, 233, 0.12);
  background: rgba(255, 255, 255, 0.96);
  box-shadow: none;
  transition: box-shadow 0.18s var(--lc-ease), border-color 0.18s var(--lc-ease);
}
.toolbar-select :deep(.el-select__wrapper.is-focused) {
  box-shadow: 0 6px 16px rgba(14, 165, 233, 0.10);
  border-color: rgba(14, 165, 233, 0.28);
}
.toolbar-select :deep(.el-select__placeholder) {
  color: #5a748f;
  font-weight: 500;
  font-size: 13px;
}
.toolbar-date {
  width: 260px;
}
.toolbar-date :deep(.el-range-editor) {
  min-height: 36px;
  border-radius: 12px;
  border-color: rgba(14, 165, 233, 0.12);
  background: rgba(255, 255, 255, 0.96);
  box-shadow: none;
}
.toolbar-date :deep(.el-date-editor .el-input__wrapper) {
  min-height: 36px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: inset 0 0 0 1px rgba(14, 165, 233, 0.12);
  transition: box-shadow 0.18s var(--lc-ease);
}
.toolbar-date :deep(.el-date-editor .el-input__wrapper.is-focus) {
  box-shadow:
    inset 0 0 0 1px rgba(14, 165, 233, 0.28),
    0 6px 16px rgba(14, 165, 233, 0.10);
}
.toolbar-group {
  width: 120px;
}
.toolbar-spacer {
  flex: 1 1 auto;
}
.toolbar-reset-btn,
.toolbar-col-btn {
  min-height: 36px;
  border-radius: 12px;
  font-weight: 500;
  border-color: rgba(14, 165, 233, 0.12);
  background: rgba(255, 255, 255, 0.96);
  transition: all 0.18s var(--lc-ease);
}
.toolbar-reset-btn:hover,
.toolbar-col-btn:hover {
  border-color: rgba(14, 165, 233, 0.25);
  background: rgba(255, 255, 255, 1);
  box-shadow: 0 4px 12px rgba(14, 165, 233, 0.08);
}
.btn-label {
  margin-left: 4px;
}

.cols-popover {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.cols-popover :deep(.el-checkbox) {
  margin-right: 0;
  display: flex;
}

/* ---------- Filter bar ---------- */
.pm-list-filter-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 6px 16px;
  background: var(--lc-accent-dim);
  border-bottom: 1px solid var(--lc-border);
}
.pm-list-filter-bar-label {
  font-size: 12px;
  color: var(--lc-text-secondary);
  font-weight: 500;
}
.pm-list-filter-chip {
  margin-right: 0;
}
.pm-list-filter-clear {
  margin-left: auto;
}

/* ---------- Scrollable data area ---------- */
.pm-list-scroll {
  flex: 1;
  overflow: auto;
  padding: 12px 16px 24px;
  transition: padding-bottom 0.2s var(--lc-ease);
}
.pm-list-scroll.has-batch {
  padding-bottom: 72px;
}

.pm-list-empty {
  padding: 48px 0;
  display: flex;
  justify-content: center;
}

.pm-list-more-hint {
  padding: 14px 0 4px;
  text-align: center;
  font-size: 12px;
  color: var(--lc-text-muted);
  letter-spacing: 0.02em;
}

/* ---------- Group ---------- */
.pm-list-group {
  margin-bottom: 12px;
  background: var(--lc-surface-0);
  border-radius: var(--lc-radius-sm);
  overflow: hidden;
  box-shadow: var(--lc-shadow-sm);
}
.pm-list-group-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  cursor: pointer;
  user-select: none;
  background: linear-gradient(135deg, var(--lc-surface-1), var(--lc-surface-0));
  transition: background 0.15s var(--lc-ease);
}
.pm-list-group-header:hover {
  background: var(--lc-accent-dim);
}
.group-caret {
  font-size: 12px;
  color: var(--lc-text-muted);
  transition: transform 0.2s var(--lc-ease);
}
.group-caret.is-open {
  transform: rotate(90deg);
  color: var(--lc-accent);
}
.group-color-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.group-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--lc-text);
}
.group-count {
  font-size: 11px;
  color: var(--lc-accent);
  background: var(--lc-accent-dim);
  padding: 1px 8px;
  border-radius: 10px;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}
.group-metrics {
  margin-left: 4px;
  font-size: 12px;
  color: var(--lc-text-muted);
  letter-spacing: 0.02em;
}

/* ---------- Table row states ---------- */
.pm-list-table :deep(.el-table__row) {
  cursor: pointer;
  transition: background 0.15s var(--lc-ease);
}
.pm-list-table :deep(.el-table__row:hover td) {
  background: var(--lc-surface-1) !important;
}
.pm-list-table :deep(.el-table__row.is-selected-row) {
  background-color: var(--lc-accent-dim) !important;
}
.pm-list-table :deep(.el-table__row.is-selected-row td) {
  background-color: var(--lc-accent-dim) !important;
}

/* ---------- Title cell ---------- */
.cell-title {
  display: flex;
  align-items: center;
  gap: 6px;
}
.title-pin {
  font-size: 12px;
  line-height: 1;
}
.title-text {
  font-weight: 500;
  color: var(--lc-text);
}
.title-edit-icon {
  font-size: 13px;
  color: var(--lc-text-muted);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s, color 0.15s;
}
.cell-title:hover .title-edit-icon {
  opacity: 1;
}
.title-edit-icon:hover {
  color: var(--lc-accent);
}
.cell-title-editor {
  display: flex;
  align-items: center;
}

/* ---------- Project cell ---------- */
.cell-project {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 10px;
  border-radius: 12px;
  font-size: 12px;
  line-height: 1.6;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cell-project-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

/* ---------- Pill (priority / status / type) ---------- */
.cell-pill {
  display: inline-block;
  font-size: 11px;
  line-height: 1.6;
  padding: 1px 8px;
  border: 1px solid;
  border-radius: 10px;
  font-weight: 500;
}
.cell-ref-code {
  font-size: 11px;
  color: var(--lc-text-secondary);
  font-family: var(--lc-font-mono);
  letter-spacing: 0.02em;
}
.cell-editable {
  cursor: pointer;
  transition: background 0.15s var(--lc-ease), filter 0.15s;
}
.cell-editable:hover {
  filter: brightness(1.05);
  background: var(--lc-accent-dim);
}

/* ---------- Date cell ---------- */
.cell-date {
  font-size: 12px;
  color: var(--lc-text-secondary);
}
.cell-date.is-overdue {
  color: var(--lc-danger);
  font-weight: 500;
}
.cell-empty {
  color: var(--lc-text-muted);
  font-size: 12px;
}
.cell-date-trigger {
  display: inline-block;
  padding: 2px 6px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.15s var(--lc-ease);
}
.cell-date-trigger:hover {
  background: var(--lc-accent-dim);
}

.inline-date-editor {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.inline-date-clear {
  align-self: flex-end;
}

/* ---------- Tags cell ---------- */
.cell-tags {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
  align-items: center;
  max-width: 100%;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 6px;
  transition: background 0.15s var(--lc-ease);
}
.cell-tags:hover {
  background: var(--lc-accent-dim);
}
.cell-tag {
  max-width: 120px;
}
.tag-more {
  font-size: 11px;
  color: var(--lc-text-muted);
  padding: 0 4px;
}

/* ---------- Batch bar ---------- */
.pm-list-batch-bar {
  position: absolute;
  left: 16px;
  right: 16px;
  bottom: 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 10px 16px;
  background: rgba(255, 255, 255, 0.92);
  backdrop-filter: blur(12px);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  box-shadow: var(--lc-shadow-lg);
}

.batch-info {
  display: flex;
  align-items: center;
  gap: 8px;
}
.batch-count {
  font-size: 13px;
  color: var(--lc-text);
  font-weight: 600;
}

.batch-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.dropdown-caret {
  margin-left: 4px;
  font-size: 11px;
}

/* ---------- Batch slide transition ---------- */
.pm-list-batch-slide-enter-active,
.pm-list-batch-slide-leave-active {
  transition: transform 0.25s var(--lc-ease-out), opacity 0.25s var(--lc-ease-out);
}
.pm-list-batch-slide-enter-from,
.pm-list-batch-slide-leave-to {
  transform: translateY(16px);
  opacity: 0;
}

/* ---------- Description / Link / TodoCount ---------- */
.cell-desc {
  display: inline-block;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lc-text-secondary);
  font-size: 12px;
}

.cell-link {
  color: var(--lc-accent);
  text-decoration: none;
  font-size: 12px;
  max-width: 200px;
  display: inline-block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: middle;
  transition: color 0.15s;
}
.cell-link:hover {
  text-decoration: underline;
  color: var(--lc-accent-light);
}

.cell-todo-count {
  font-size: 12px;
  color: var(--lc-text-secondary);
  font-variant-numeric: tabular-nums;
}

/* ---------- Row expand ---------- */
.row-expand {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 10px 16px;
  background: var(--lc-surface-1);
  border-radius: var(--lc-radius-sm);
  margin: 0 8px 8px;
}
.row-expand-info {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.row-expand-desc {
  font-size: 13px;
  color: var(--lc-text-secondary);
  line-height: 1.6;
  white-space: pre-wrap;
}
.row-expand-desc.is-empty {
  color: var(--lc-text-muted);
  font-style: italic;
}
.row-expand-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  font-size: 12px;
}
.row-expand-meta-item {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.meta-label {
  color: var(--lc-text-muted);
}
.meta-value {
  color: var(--lc-text);
  font-variant-numeric: tabular-nums;
}
.meta-value.is-overdue {
  color: var(--lc-danger);
  font-weight: 500;
}
.row-expand-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: center;
  padding-top: 8px;
  border-top: 1px dashed var(--lc-border);
}

.batch-tag-popover {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.batch-tag-popover-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}
</style>
