<template>
  <el-config-provider :locale="zhCn">
  <div class="pm-panel">
    <div class="pm-layout">
      <!-- Left: Project list -->
      <aside class="pm-sidebar">
        <div class="sidebar-header">
          <div>
            <div class="sidebar-eyebrow">项目空间</div>
            <span class="sidebar-title">{{ projects.length }} 个项目</span>
          </div>
          <el-button class="sidebar-create-btn" type="primary" @click="showCreateProject">
            <el-icon><Plus /></el-icon>
            新建
          </el-button>
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
            <span class="project-count">{{ overviewUndoneCount }}</span>
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
            <span class="sidebar-projects-sort">按任务总数排序</span>
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
            @contextmenu.prevent="onProjectContext($event, p)"
            @dragover.prevent="onProjectDragOver(p)"
            @dragleave="onProjectDragLeave(p)"
            @drop.prevent="onProjectDrop(p)"
          >
            <div class="project-card-main">
              <span class="project-color" :style="{ backgroundColor: p.color }" />
              <span class="project-name">{{ p.name }}</span>
              <el-tag v-if="p.status === 'archived'" size="small" effect="plain" class="project-archived-tag">已归档</el-tag>
              <span class="project-count">{{ p.pendingCount }}</span>
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
          <div class="toolbar-row">
            <div class="toolbar-left">
              <span class="project-title-display" :style="{ color: isOverview ? '' : selectedProject.color }">{{ selectedProject.name }}</span>
              <el-tag v-if="!isOverview && selectedProject.status === 'archived'" size="small" type="info">已归档</el-tag>
            </div>
            <div class="toolbar-right">
              <el-radio-group v-model="viewMode" size="default">
                <el-radio-button value="kanban">看板</el-radio-button>
                <el-radio-button value="gantt">甘特图</el-radio-button>
              </el-radio-group>
              <el-button type="primary" @click="showCreateItem">新建工作项</el-button>
            </div>
          </div>
          <div class="toolbar-row toolbar-filters">
            <el-input
              v-model="searchText"
              size="default"
              placeholder="搜索工作项..."
              clearable
              style="width: 180px"
            />
            <el-select v-model="filterType" size="default" placeholder="类型" clearable style="width: 100px">
              <el-option v-for="(meta, key) in PM_ITEM_TYPE_MAP" :key="key" :label="meta.label" :value="key" />
            </el-select>
            <el-select v-model="filterPriority" size="default" placeholder="优先级" clearable style="width: 100px">
              <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key" />
            </el-select>
            <el-button type="default" @click="openSiyuanDrawer">思源设置</el-button>
          </div>
        </div>

        <div v-if="selectedProject && viewMode === 'kanban'" class="kanban-board">
          <div v-for="col in PM_STATUS_COLUMNS" :key="col.key" class="kanban-column" :class="{ 'is-drag-over': draggingOverColumn === col.key }">
            <div class="column-header" :style="{ borderBottomColor: col.color }">
              <span class="column-title">{{ col.label }}</span>
              <span class="column-count" :style="{ background: col.color + '1a', color: col.color }">{{ columnItems(col.key).length }}</span>
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
                  ['is-' + item.priority.toLowerCase()]: true,
                }"
                :style="{ borderLeftColor: PM_PRIORITY_MAP[item.priority]?.color }"
                :data-id="item.id"
                @click="onCardClick($event, item)"
                @dblclick="onCardDblclick(item)"
                @contextmenu.prevent="onItemContext($event, item)"
              >
                <div class="card-topbar" :class="{ 'is-overview': isOverview }">
                  <div class="card-topbar-left">
                    <template v-if="isOverview && item.projectName">
                      <span class="card-project-badge" :style="{ backgroundColor: (item.projectColor || '#4d7df2') + '18', color: item.projectColor || '#4d7df2' }">
                        <span class="card-project-dot" :style="{ backgroundColor: item.projectColor || '#4d7df2' }" />
                        <span class="card-project-name">{{ item.projectName }}</span>
                      </span>
                    </template>
                    <template v-else>
                      <span class="card-meta-pill" :style="{ color: PM_ITEM_TYPE_MAP[item.itemType]?.color, borderColor: PM_ITEM_TYPE_MAP[item.itemType]?.color + '40' }">
                        {{ PM_ITEM_TYPE_MAP[item.itemType]?.label }}
                      </span>
                      <span class="card-meta-pill" :style="{ color: PM_PRIORITY_MAP[item.priority]?.color, borderColor: PM_PRIORITY_MAP[item.priority]?.color + '40' }">
                        {{ PM_PRIORITY_MAP[item.priority]?.label }}
                      </span>
                    </template>
                  </div>
                  <div class="card-topbar-right">
                    <el-icon v-if="item.pinned" class="badge-pin" title="已置顶"><Top /></el-icon>
                    <el-icon v-if="isOverdue(item)" class="badge-overdue" title="已逾期"><AlarmClock /></el-icon>
                    <span
                      v-if="hasPmDateSchedule(item.startAt, item.endAt)"
                      class="card-date-chip"
                      :class="{ 'is-overdue-date': isOverdue(item) }"
                    >
                      {{ formatPmDateRangeForDisplay(item.startAt, item.endAt, { mode: 'short', emptyText: '' }) }}
                    </span>
                  </div>
                </div>
                <div class="card-title-row">
                  <span class="card-title">{{ item.title }}</span>
                  <el-tooltip v-if="item.status !== 'done'" :content="'推进到「' + nextStatusLabel(item) + '」'" placement="top">
                    <button
                      class="card-advance-btn"
                      @click.stop="quickAdvance(item)"
                    >
                      <el-icon :size="12"><CaretRight /></el-icon>
                    </button>
                  </el-tooltip>
                </div>
                <div v-if="isOverview" class="card-meta">
                  <span class="card-meta-pill" :style="{ color: PM_ITEM_TYPE_MAP[item.itemType]?.color, borderColor: PM_ITEM_TYPE_MAP[item.itemType]?.color + '40' }">
                    {{ PM_ITEM_TYPE_MAP[item.itemType]?.label }}
                  </span>
                  <span class="card-meta-pill" :style="{ color: PM_PRIORITY_MAP[item.priority]?.color, borderColor: PM_PRIORITY_MAP[item.priority]?.color + '40' }">
                    {{ PM_PRIORITY_MAP[item.priority]?.label }}
                  </span>
                </div>
                <div v-if="item.tags.length > 0" class="card-tags">
                  <el-tag v-for="tag in getItemTagSummary(item).visibleTags" :key="tag" size="small" type="info">{{ tag }}</el-tag>
                  <el-tag v-if="getItemTagSummary(item).hiddenCount > 0" size="small" type="info">+{{ getItemTagSummary(item).hiddenCount }}</el-tag>
                </div>
              </div>
              <div v-if="columnItems(col.key).length === 0 && draggingItemId" class="column-drop-hint">
                拖放到此列
              </div>
              <div v-if="columnItems(col.key).length === 0 && !draggingItemId" class="column-empty-state">
                <span class="column-empty-text">{{ col.key === 'todo' ? '暂无待办事项' : col.key === 'done' ? '还没有完成的工作项' : '暂无工作项' }}</span>
                <el-button v-if="col.key === 'todo'" size="small" type="primary" link @click="showCreateItem">新建工作项</el-button>
              </div>
            </div>
          </div>
        </div>

        <PmGanttView
          v-if="selectedProject && viewMode === 'gantt'"
          :items="ganttFilteredItems"
          :selected-statuses="ganttSelectedStatuses"
          :selected-item-id="selectedItemId"
          :show-project-meta="isOverview"
          @select="selectItem"
          @edit="editItem"
          @item-context="onGanttItemContext"
          @date-change="onGanttDateChange"
          @toggle-status="onGanttToggleStatus"
          @select-all-statuses="selectAllGanttStatuses"
          @clear-statuses="clearGanttStatuses"
          @view-change="closeCtxMenu"
          @viewport-scroll="closeCtxMenu"
        />

        <div v-if="!selectedProject" class="pm-empty">
          <el-empty description="选择一个项目查看看板" />
        </div>

        <!-- Right: Detail panel (floating) -->
        <Transition name="pm-detail-slide">
          <aside v-if="selectedItem" class="pm-detail">
            <div class="detail-header">
              <div>
                <div class="detail-header-eyebrow">工作项详情</div>
                <span class="detail-title">详情</span>
              </div>
              <el-button size="small" link @click="selectedItemId = null">
                <el-icon><Close /></el-icon>
              </el-button>
            </div>
            <div class="detail-form">
              <div class="detail-hero">
                <div class="detail-hero-head">
                  <div class="detail-project-chip">
                    <span class="detail-project-dot" :style="{ backgroundColor: selectedItemProject?.color ?? selectedItem.projectColor ?? '#4d7df2' }" />
                    <span class="detail-project-name">{{ selectedItemProject?.name ?? selectedItem.projectName ?? "-" }}</span>
                    <el-tag v-if="selectedItemProject?.status === 'archived'" size="small" effect="plain" class="detail-project-archived-tag">
                      已归档
                    </el-tag>
                  </div>
                  <el-tag v-if="isOverdue(selectedItem)" size="small" effect="light" type="danger">已逾期</el-tag>
                </div>
                <div class="detail-item-title">{{ selectedItem.title }}</div>
                <div class="detail-field-inline">
                  <el-tag
                    size="small"
                    effect="light"
                    round
                    :style="getPmLightTagStyle(PM_ITEM_TYPE_MAP[selectedItem.itemType]?.color)"
                  >
                    {{ PM_ITEM_TYPE_MAP[selectedItem.itemType]?.label ?? selectedItem.itemType }}
                  </el-tag>
                  <el-tag size="small" effect="light" round>
                    <span class="priority-dot" :style="{ backgroundColor: PM_PRIORITY_MAP[selectedItem.priority]?.color }" />
                    {{ PM_PRIORITY_MAP[selectedItem.priority]?.label ?? selectedItem.priority }}
                  </el-tag>
                  <el-tag size="small" :type="selectedItem.status === 'done' ? 'success' : selectedItem.status === 'in_progress' ? 'primary' : selectedItem.status === 'testing' ? 'warning' : 'info'" effect="light" round>
                    {{ PM_STATUS_COLUMNS.find(c => c.key === selectedItem.status)?.label ?? selectedItem.status }}
                  </el-tag>
                  <el-tag v-for="tag in selectedItem.tags" :key="tag" size="small" type="info">{{ tag }}</el-tag>
                </div>
              </div>

              <div class="detail-section">
                <div class="detail-section-head">
                  <span class="detail-section-title">时间轨迹</span>
                  <span class="detail-section-subtitle">按状态推进记录</span>
                </div>
                <div class="detail-timeline-grid">
                  <div class="detail-timeline-card">
                    <span class="detail-label">时间安排</span>
                    <span class="detail-value" :class="{ 'is-overdue-date': isOverdue(selectedItem) }">
                      {{ formatPmDateRangeForDisplay(selectedItem.startAt, selectedItem.endAt) }}
                    </span>
                  </div>
                  <div class="detail-timeline-card">
                    <span class="detail-label">创建时间</span>
                    <span class="detail-value">{{ formatDateTime(selectedItem.createdAt) }}</span>
                  </div>
                  <div class="detail-timeline-card">
                    <span class="detail-label">开始执行</span>
                    <span class="detail-value">{{ selectedItem.startedAt ? formatDateTime(selectedItem.startedAt) : "-" }}</span>
                  </div>
                  <div class="detail-timeline-card">
                    <span class="detail-label">开始测试</span>
                    <span class="detail-value">{{ selectedItem.testingAt ? formatDateTime(selectedItem.testingAt) : "-" }}</span>
                  </div>
                  <div v-if="selectedItem.completedAt" class="detail-timeline-card">
                    <span class="detail-label">完成时间</span>
                    <span class="detail-value">{{ formatDateTime(selectedItem.completedAt) }}</span>
                  </div>
                </div>
              </div>

              <div v-if="selectedItemDescriptionText" class="detail-section">
                <div class="detail-section-head">
                  <span class="detail-section-title">描述</span>
                  <span class="detail-section-subtitle">保留换行</span>
                </div>
                <pre class="detail-value detail-description">{{ selectedItemDescriptionText }}</pre>
              </div>

              <div class="detail-section">
                <div class="detail-section-head">
                  <span class="detail-section-title">资源关联</span>
                  <span class="detail-section-subtitle">链接与思源页面</span>
                </div>
                <div class="detail-resource-list">
                  <div class="detail-resource-card">
                    <div class="detail-resource-main">
                      <span class="detail-label">链接</span>
                      <span class="detail-value detail-link-text">{{ selectedItem.linkUrl || "-" }}</span>
                    </div>
                    <el-button v-if="selectedItem.linkUrl" size="small" link @click="openItemLink(selectedItem.linkUrl)">
                      打开
                    </el-button>
                  </div>
                  <div class="detail-resource-card">
                    <template v-if="selectedItem.siyuanPrimaryPage">
                      <div class="detail-siyuan-page-main">
                        <span class="detail-label">思源主页面</span>
                        <span class="detail-siyuan-page-title">{{ selectedItem.siyuanPrimaryPage.docTitle }}</span>
                        <span class="detail-siyuan-page-meta">
                          {{ selectedItem.siyuanPrimaryPage.notebookName }} · {{ selectedItem.siyuanPrimaryPage.docHpath }}
                        </span>
                      </div>
                      <el-button size="small" link @click="openSiyuanPage(selectedItem.siyuanPrimaryPage)">打开</el-button>
                    </template>
                    <template v-else>
                      <div class="detail-resource-main">
                        <span class="detail-label">思源主页面</span>
                        <span class="detail-value">-</span>
                      </div>
                    </template>
                  </div>
                  <div v-for="page in selectedItem.siyuanExtraPages" :key="page.docId" class="detail-resource-card">
                    <div class="detail-siyuan-page-main">
                      <span class="detail-label">附加页面</span>
                      <span class="detail-siyuan-page-title">{{ page.docTitle }}</span>
                      <span class="detail-siyuan-page-meta">{{ page.notebookName }} · {{ page.docHpath }}</span>
                    </div>
                    <el-button size="small" link @click="openSiyuanPage(page)">打开</el-button>
                  </div>
                </div>
              </div>

              <div v-if="!selectedItemDescriptionText" class="detail-section detail-section--muted">
                <div class="detail-section-head">
                  <span class="detail-section-title">描述</span>
                  <span class="detail-section-subtitle">暂无内容</span>
                </div>
                <div class="detail-empty-text">暂无描述</div>
              </div>

              <div class="detail-actions">
                <el-button size="small" @click="togglePin">{{ selectedItem.pinned ? '取消置顶' : '置顶' }}</el-button>
                <el-button v-if="selectedItem.status !== 'done'" size="small" type="primary" plain @click="advanceStatus">
                  推进状态
                </el-button>
                <el-button size="small" type="danger" plain @click="deleteItem">删除</el-button>
              </div>
            </div>
          </aside>
        </Transition>
      </div>
    </div>
    <el-dialog v-model="projectDialogVisible" :title="editingProject ? '编辑项目' : '新建项目'" width="520px" @close="resetProjectForm">
      <el-form :model="projectForm" label-width="60px" size="default" @submit.prevent="submitProject">
        <el-form-item label="名称">
          <el-input v-model="projectForm.name" placeholder="项目名称" @keyup.enter="submitProject" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="projectForm.description" type="textarea" :rows="2" placeholder="项目描述（可选）" />
        </el-form-item>
        <el-form-item label="颜色">
          <el-color-picker v-model="projectForm.color" :predefine="presetColors" />
        </el-form-item>
        <el-form-item label="思源位置" class="pm-form-item-top">
          <div class="pm-siyuan-config-card">
            <el-radio-group v-model="projectForm.useSiyuanOverride">
              <el-radio :value="false">继承全局默认</el-radio>
              <el-radio :value="true">使用项目专属位置</el-radio>
            </el-radio-group>
            <div class="pm-siyuan-config-summary">
              当前：{{
                projectForm.useSiyuanOverride
                  ? formatPmSiyuanLocationLabel(projectForm.siyuanLocationOverride)
                  : formatPmSiyuanLocationLabel(globalSiyuanLocation)
              }}
            </div>
            <div v-if="projectForm.useSiyuanOverride" class="pm-siyuan-inline-actions">
              <el-button size="small" @click="openSiyuanLocationPicker('project')">选择位置</el-button>
              <el-button size="small" @click="clearProjectSiyuanOverride">清空</el-button>
            </div>
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="projectDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="submitProject">确定</el-button>
      </template>
    </el-dialog>

    <!-- Item dialog -->
    <el-dialog v-model="itemDialogVisible" :title="editingItem ? '编辑工作项' : '新建工作项'" width="860px" @close="resetItemForm">
      <el-form :model="itemForm" label-position="top" size="default" class="pm-item-dialog-form">
        <div class="pm-item-project-card">
          <div class="pm-item-project-card-head">
            <div>
              <div class="pm-item-section-eyebrow">所属项目</div>
              <div class="pm-item-project-card-main">
                <span class="pm-item-project-card-dot" :style="{ backgroundColor: itemDialogProjectDisplayColor }" />
                <span class="pm-item-project-card-name">{{ itemDialogProjectDisplayName }}</span>
                <el-tag v-if="itemDialogProject?.status === 'archived'" size="small" effect="plain" class="project-archived-tag">已归档</el-tag>
              </div>
            </div>
            <div class="pm-item-project-card-actions">
              <el-tag v-if="itemDialogProject" size="small" effect="plain" class="pm-item-project-count-tag">待办 {{ itemDialogProjectPendingCount }}</el-tag>
              <el-select
                v-if="isOverview || editingItem"
                v-model="itemFormProjectId"
                placeholder="切换项目"
                size="small"
                class="pm-item-project-select"
              >
                <el-option
                  v-for="p in itemProjectOptions"
                  :key="p.id"
                  :label="formatItemProjectOptionLabel(p)"
                  :value="p.id"
                />
              </el-select>
            </div>
          </div>
        </div>

        <div class="pm-item-section">
          <div class="pm-item-section-title">核心信息</div>
          <el-form-item label="标题">
            <el-input v-model="itemForm.title" placeholder="工作项标题" />
          </el-form-item>
          <div class="pm-item-dialog-core-grid">
            <el-form-item label="类型" class="pm-item-dialog-inline-field">
              <el-select v-model="itemForm.itemType">
                <el-option v-for="(meta, key) in PM_ITEM_TYPE_MAP" :key="key" :label="meta.label" :value="key" />
              </el-select>
            </el-form-item>
            <el-form-item label="优先级" class="pm-item-dialog-inline-field">
              <el-select v-model="itemForm.priority">
                <template #prefix>
                  <span class="priority-dot" :style="{ backgroundColor: PM_PRIORITY_MAP[itemForm.priority]?.color }" />
                </template>
                <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key">
                  <span class="priority-dot" :style="{ backgroundColor: meta.color }" />
                  {{ meta.label }}
                </el-option>
              </el-select>
            </el-form-item>
            <el-form-item label="状态" class="pm-item-dialog-inline-field">
              <el-select v-model="itemForm.status">
                <el-option v-for="col in PM_STATUS_COLUMNS" :key="col.key" :label="col.label" :value="col.key" />
              </el-select>
            </el-form-item>
          </div>
        </div>

        <div class="pm-item-section">
          <el-form-item label="时间安排">
            <el-date-picker
              v-model="itemFormDateRange"
              type="daterange"
              value-format="YYYY-MM-DD"
              range-separator="~"
              start-placeholder="开始日期"
              end-placeholder="截止日期"
              clearable
              style="width: 100%"
            />
          </el-form-item>
        </div>

        <div class="pm-item-section">
          <el-form-item label="链接">
            <el-input v-model="itemForm.linkUrl" placeholder="https://example.com 或 localhost:3000">
              <template #append>
                <div class="pm-input-append-actions">
                  <el-button :disabled="!itemFormOpenableLink" @click="openItemFormLink">打开</el-button>
                  <el-button :disabled="!itemForm.linkUrl" @click="itemForm.linkUrl = ''">清空</el-button>
                </div>
              </template>
            </el-input>
          </el-form-item>
        </div>

        <div class="pm-item-section">
          <el-form-item label="描述">
            <el-input v-model="itemForm.description" type="textarea" :rows="4" />
          </el-form-item>
        </div>

        <div class="pm-item-section">
          <el-form-item label="思源关联" class="pm-form-item-top">
            <div class="pm-siyuan-link-card">
              <div class="pm-siyuan-link-header">
                <div class="pm-siyuan-link-heading">
                  <div class="pm-siyuan-link-title">思源关联</div>
                  <div class="pm-siyuan-link-subtitle">{{ itemSiyuanLocationSummary }}</div>
                </div>
                <div class="pm-siyuan-inline-actions">
                  <el-button size="small" type="primary" plain @click="openSiyuanLinkPicker()">关联页面</el-button>
                </div>
              </div>
              <div v-if="itemLinkedPages.length > 0" class="pm-siyuan-page-list">
                <div v-for="row in itemLinkedPages" :key="row.page.docId" class="pm-siyuan-page-row">
                  <div class="pm-siyuan-page-main">
                    <div class="pm-siyuan-page-row-head">
                      <div class="pm-siyuan-page-title">{{ row.page.docTitle }}</div>
                      <el-tag v-if="row.kind === 'primary'" size="small" effect="plain" type="primary">主页面</el-tag>
                    </div>
                    <div class="pm-siyuan-page-meta">{{ row.page.notebookName }} · {{ row.page.docHpath }}</div>
                  </div>
                  <div class="pm-siyuan-page-actions">
                    <el-button size="small" link @click="openSiyuanPage(row.page)">打开</el-button>
                    <el-dropdown trigger="click" @command="(command) => handleItemSiyuanPageCommand(row, command)">
                      <el-button size="small" link class="pm-siyuan-more-trigger">更多</el-button>
                      <template #dropdown>
                        <el-dropdown-menu>
                          <el-dropdown-item v-if="row.kind === 'primary'" command="replace-primary">更换主页面</el-dropdown-item>
                          <el-dropdown-item v-else command="promote-primary">设为主页面</el-dropdown-item>
                          <el-dropdown-item command="remove">移除</el-dropdown-item>
                        </el-dropdown-menu>
                      </template>
                    </el-dropdown>
                  </div>
                </div>
              </div>
              <div v-else class="pm-siyuan-empty-inline">尚未关联页面，点击右上角开始关联。</div>
            </div>
          </el-form-item>
        </div>

        <div v-if="editingItem" class="pm-item-section pm-item-section--muted">
          <div class="pm-item-section-title">流转记录</div>
          <div class="pm-item-dialog-core-grid">
            <el-form-item label="开始执行" class="pm-item-dialog-inline-field">
              <el-date-picker v-model="itemForm.startedAt" type="datetime" clearable placeholder="自动记录，可手动修改" style="width: 100%" />
            </el-form-item>
            <el-form-item label="开始测试" class="pm-item-dialog-inline-field">
              <el-date-picker v-model="itemForm.testingAt" type="datetime" clearable placeholder="自动记录，可手动修改" style="width: 100%" />
            </el-form-item>
            <el-form-item label="完成时间" class="pm-item-dialog-inline-field">
              <el-date-picker v-model="itemForm.completedAt" type="datetime" clearable placeholder="自动记录，可手动修改" style="width: 100%" />
            </el-form-item>
          </div>
        </div>
      </el-form>
      <template #footer>
        <el-button @click="itemDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="submitItem">确定</el-button>
      </template>
    </el-dialog>

    <!-- Context menu (Vue reactive) -->
    <Teleport to="body">
      <Transition name="ctx-fade">
        <div
          v-if="ctxMenuVisible"
          ref="ctxMenuRef"
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

    <el-drawer
      :model-value="siyuanDrawerVisible"
      direction="rtl"
      size="420px"
      wrapper-class="pm-siyuan-drawer"
      destroy-on-close
      :with-header="true"
      @close="siyuanDrawerVisible = false"
    >
      <template #title>
        思源配置
      </template>
      <div class="siyuan-drawer-body">
        <el-form label-position="top" size="default">
          <el-form-item label="服务地址">
            <el-input
              v-model="siyuanForm.baseUrl"
              placeholder="http://127.0.0.1:6806"
              clearable
            />
          </el-form-item>
          <el-form-item label="API Token">
            <el-input
              :type="siyuanShowToken ? 'text' : 'password'"
              v-model="siyuanForm.token"
              placeholder="填写思源 API Token"
              clearable
            >
              <template #suffix>
                <el-button type="text" size="small" @click="siyuanShowToken = !siyuanShowToken">
                  {{ siyuanShowToken ? "隐藏" : "显示" }}
                </el-button>
              </template>
            </el-input>
          </el-form-item>
        </el-form>
        <div class="pm-siyuan-config-card">
          <div class="pm-siyuan-link-title">任务默认存储位置</div>
          <div class="pm-siyuan-config-summary">
            {{ formatPmSiyuanLocationLabel(globalSiyuanLocationDraft) }}
          </div>
          <div class="pm-siyuan-inline-actions">
            <el-button size="small" @click="openSiyuanLocationPicker('global')">选择位置</el-button>
            <el-button size="small" @click="globalSiyuanLocationDraft = null">清空</el-button>
          </div>
        </div>
        <div class="siyuan-actions">
          <el-button type="primary" @click="saveSiyuanConfig">保存配置</el-button>
          <el-button type="info" :loading="siyuanTesting" @click="handleTestConnection">测试连接</el-button>
          <el-button type="default" :loading="siyuanLoadingDirectory" @click="handleLoadDirectory">加载目录</el-button>
        </div>
        <div class="siyuan-status">
          <el-tag v-if="siyuanTestingVersion" type="success" effect="dark">
            已连接 · {{ siyuanTestingVersion }}
          </el-tag>
          <el-alert
            v-if="siyuanError"
            :title="siyuanErrorTitle"
            :description="siyuanError"
            type="error"
            show-icon
            class="siyuan-error-alert"
          />
          <div v-else-if="siyuanDirectoryFetchedAt" class="siyuan-fetch-hint">
            最后一次加载：{{ siyuanDirectoryFetchedAt }}
          </div>
        </div>
        <div class="siyuan-tree-section">
          <div v-if="siyuanLoadingDirectory" class="siyuan-loading-hint">
            正在从思源加载目录...
          </div>
          <el-empty
            v-if="!siyuanDirectory.length && !siyuanLoadingDirectory"
            description="暂无目录记录，先点击“加载目录”"
          />
          <el-tree
            v-else-if="siyuanDirectory.length"
            :data="siyuanDirectory"
            :props="siyuanTreeProps"
            node-key="id"
            :expand-on-click-node="false"
            class="siyuan-tree"
          >
            <template #default="{ data }">
              <div class="siyuan-tree-node siyuan-tree-node--preview">
                <div class="siyuan-node-main">
                  <span class="siyuan-node-title">{{ data.name }}</span>
                  <span
                    v-if="isPmSiyuanNotebookDirectory(data)"
                    class="siyuan-node-badge"
                    :class="{ 'is-disabled': data.closed }"
                  >
                    {{ data.closed ? "已关闭" : `${data.docCount} 篇` }}
                  </span>
                </div>
              </div>
            </template>
          </el-tree>
        </div>
      </div>
    </el-drawer>

    <el-dialog
      v-model="siyuanLocationDialogVisible"
      :title="siyuanLocationPickerTitle"
      width="720px"
      destroy-on-close
    >
      <div class="pm-siyuan-dialog-body pm-siyuan-dialog-body--picker">
        <div class="pm-siyuan-picker-intro">
          选择笔记本根目录或某个父文档后，项目内新建思源页面会默认存放到这里。
        </div>
        <el-input
          v-model="siyuanLocationPickerSearch"
          class="pm-siyuan-picker-search"
          placeholder="搜索笔记本、文档标题或路径"
          clearable
        />
        <div v-if="siyuanLoadingDirectory && !siyuanDirectory.length" class="pm-siyuan-empty-hint">
          正在同步思源目录，请稍候…
        </div>
        <div v-else-if="!siyuanDirectory.length" class="pm-siyuan-empty-hint">
          尚未加载思源目录，请先测试连接并加载目录。
        </div>
        <template v-else>
          <div class="pm-siyuan-picker-tree-head">
            <span>{{ siyuanLocationPickerStatusText }}</span>
            <div class="pm-siyuan-inline-actions">
              <span v-if="siyuanDirectoryFetchedAt">最后同步：{{ siyuanDirectoryFetchedAt }}</span>
              <span v-if="siyuanLocationPickerSearchKeyword">清空搜索后会恢复默认折叠。</span>
              <el-button
                size="small"
                text
                :loading="siyuanLoadingDirectory"
                @click="handleRefreshSiyuanLocationPicker"
              >
                刷新目录
              </el-button>
            </div>
          </div>
          <div class="pm-siyuan-picker-tree-shell">
            <el-empty
              v-if="siyuanLocationPickerTreeData.length === 0"
              description="未找到匹配位置，请换个关键词试试"
            />
            <el-tree
              v-else
              :key="siyuanLocationPickerTreeKey"
              :data="siyuanLocationPickerTreeData"
              :props="siyuanTreeProps"
              node-key="id"
              highlight-current
              :current-node-key="siyuanLocationPickerCurrentNodeKey"
              :default-expanded-keys="siyuanLocationPickerExpandedKeys"
              :expand-on-click-node="false"
              class="siyuan-tree pm-siyuan-picker-tree"
              @node-click="handleSiyuanLocationTreeNodeClick"
            >
              <template #default="{ data, node }">
                <div
                  class="siyuan-tree-node siyuan-tree-node--interactive"
                  :class="{
                    'is-selected': isSiyuanLocationPickerNodeSelected(data),
                    'is-disabled': isSiyuanLocationPickerNodeDisabled(data, node),
                  }"
                >
                  <div class="siyuan-node-main">
                    <span class="siyuan-node-title">{{ data.name }}</span>
                    <span
                      v-if="isPmSiyuanNotebookDirectory(data)"
                      class="siyuan-node-badge"
                      :class="{ 'is-disabled': data.closed }"
                    >
                      {{ data.closed ? "已关闭" : `${data.docCount} 篇` }}
                    </span>
                  </div>
                </div>
              </template>
            </el-tree>
          </div>
        </template>
        <div
          class="pm-siyuan-picker-selection"
          :class="{ 'is-empty': !siyuanLocationPickerValue }"
        >
          <div class="pm-siyuan-picker-selection-label">当前选择</div>
          <template v-if="siyuanLocationPickerValue">
            <div class="pm-siyuan-picker-selection-title">
              {{ siyuanLocationPickerSelectionTarget }}
            </div>
            <div class="pm-siyuan-picker-selection-meta">
              {{ siyuanLocationPickerValue.notebookName }}
            </div>
            <div class="pm-siyuan-picker-selection-path">
              {{ siyuanLocationPickerSelectionPath }}
            </div>
          </template>
          <div v-else class="pm-siyuan-picker-selection-empty">
            暂未选择位置，点击上方笔记本根目录或父文档即可。
          </div>
        </div>
      </div>
      <template #footer>
        <el-button @click="siyuanLocationDialogVisible = false">取消</el-button>
        <el-button @click="clearSiyuanLocationPicker">清空</el-button>
        <el-button type="primary" @click="applySiyuanLocationPicker">确定</el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="siyuanPageDialogVisible"
      :title="siyuanPageDialogTitle"
      width="760px"
    >
      <div class="pm-siyuan-dialog-body">
        <div class="pm-siyuan-search-toolbar">
          <el-input
            v-model="siyuanPageFilterKeyword"
            :placeholder="siyuanPageDialogInputPlaceholder"
            clearable
          />
          <el-button v-if="siyuanPageShowReturnToLocation" @click="restoreSiyuanLocationResults()">
            返回当前位置列表
          </el-button>
          <el-button :loading="siyuanPageSearchingAll" @click="expandSiyuanPagesToAll()">扩展到全库</el-button>
          <el-button
            type="success"
            :loading="siyuanPageCreating"
            :disabled="!siyuanPageCanCreateImmediately"
            @click="createSiyuanPageForItem"
          >
            立即新建
          </el-button>
        </div>

        <div class="pm-siyuan-dialog-hint">{{ siyuanPageCurrentRangeText }}</div>
        <div v-if="siyuanPageFilterSummary" class="pm-siyuan-dialog-hint">{{ siyuanPageFilterSummary }}</div>
        <div class="pm-siyuan-dialog-hint">
          当前创建位置：{{ formatPmSiyuanLocationLabel(itemEffectiveLocation) }}
        </div>
        <div class="pm-siyuan-dialog-hint">
          当前创建标题：{{ siyuanPageCreateTitle || "请先填写工作项标题或输入想创建的页面标题" }}
        </div>

        <div
          v-if="
            siyuanPageLocationRefreshError &&
            siyuanPageResultSource === 'location' &&
            siyuanPageLocationState !== 'load-error'
          "
          class="pm-siyuan-dialog-notice pm-siyuan-dialog-notice--warning"
        >
          当前位置列表刷新失败，暂展示上一次结果。{{ siyuanPageLocationRefreshError }}
        </div>

        <div v-if="siyuanPageShowLocationLoading" class="pm-siyuan-empty-hint">
          正在加载当前位置文档列表...
        </div>
        <div v-else-if="siyuanPageShowAllLoading" class="pm-siyuan-empty-hint">
          正在搜索全库...
        </div>
        <div v-else-if="siyuanPageEmptyMessage" class="pm-siyuan-empty-hint">
          {{ siyuanPageEmptyMessage }}
        </div>
        <div v-else class="pm-siyuan-page-list">
          <div v-for="page in siyuanPageDisplayedResults" :key="page.docId" class="pm-siyuan-page-row">
            <div class="pm-siyuan-page-main">
              <div class="pm-siyuan-page-title">{{ page.docTitle }}</div>
              <div class="pm-siyuan-page-meta">{{ page.notebookName }} · {{ page.docHpath }}</div>
            </div>
            <div class="pm-siyuan-inline-actions">
              <el-button size="small" type="primary" link @click="selectSiyuanPageResult(page)">
                {{ siyuanPageDialogMode === 'primary' ? '设为主页面' : '添加' }}
              </el-button>
              <el-button size="small" link @click="openSiyuanPage(page)">打开</el-button>
            </div>
          </div>
        </div>
      </div>
      <template #footer>
        <el-button @click="siyuanPageDialogVisible = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
  </el-config-provider>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount, reactive } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import { Plus, Close, Top, CaretRight, AlarmClock } from "@element-plus/icons-vue";
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
} from "../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../types/pm";
import Sortable from "sortablejs";
import PmGanttView from "./PmGanttView.vue";
import { clampContextMenuPosition } from "../utils/contextMenu";
import {
  addPmSiyuanExtraPage,
  collectPmSiyuanExpandedKeys,
  collectPmSiyuanPagesForLocation,
  filterPmSiyuanDirectory,
  filterPmSiyuanPages,
  formatPmSiyuanLocationLabel,
  formatPmSiyuanLocationPathLabel,
  formatPmSiyuanLocationTargetLabel,
  isPmSiyuanNotebookDirectory,
  removePmSiyuanPage,
  resolvePmSiyuanEffectiveLocation,
  setPmSiyuanPrimaryPage,
} from "../utils/pmSiyuan";
import {
  formatPmDateRangeForDisplay,
  getPmDateRangeValue,
  hasPmDateSchedule,
  isPmItemOverdue,
  normalizePmDateRangeForDraft,
} from "../utils/pmDate";
import {
  getPmPendingCount,
  sortPmProjectsForSidebar,
  summarizePmItemTags,
} from "../utils/pmVisual";
import {
  clearPmGanttStatuses,
  filterPmItemsByGanttStatuses,
  getPmGanttDefaultStatuses,
  selectAllPmGanttStatuses,
  togglePmGanttStatus,
} from "../utils/pmGanttFilter";

const { invoke } = useToolInvoke();
const defaultBaseUrl = "http://127.0.0.1:6806";
const PM_SIYUAN_DEFAULT_LOCATION_KEY = "pm_siyuan_default_location";

// ── Types ────────────────────────────────────────────────

interface CtxMenuAction {
  label: string;
  action: () => void | Promise<void>;
  danger?: boolean;
  divider?: boolean;
}

interface ItemSiyuanLinkedRow {
  page: PmSiyuanPageRef;
  kind: "primary" | "extra";
}

type PmSiyuanPageLocationState =
  | "ready"
  | "missing-location"
  | "missing-config"
  | "load-error"
  | "invalid-location"
  | "empty";

// ── State ────────────────────────────────────────────────

const projects = ref<PmProject[]>([]);
const items = ref<PmItem[]>([]);
const projectItemCounts = ref<Record<number, { total: number; done: number }>>({});
const selectedProjectId = ref<number | "overview" | null>(null);
const selectedItemId = ref<number | null>(null);
const searchText = ref("");
const filterType = ref<PmItemType | "">("");
const filterPriority = ref<PmPriority | "">("");
const viewMode = ref<"kanban" | "gantt">("kanban");
const ganttSelectedStatuses = ref<PmItemStatus[]>(getPmGanttDefaultStatuses());

// Project dialog
const projectDialogVisible = ref(false);
const editingProject = ref<PmProject | null>(null);
const projectForm = ref({
  name: "",
  description: "",
  color: "#409eff",
  useSiyuanOverride: false,
  siyuanLocationOverride: null as PmSiyuanLocation | null,
});
const presetColors = ["#7eb8f7", "#95d4a1", "#f7c97e", "#f7a1a1", "#b0bec5", "#80d8e8", "#ce93d8", "#ffab91", "#a5d6a7", "#fff176", "#80cbc4", "#ef9a9a"];

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
  linkUrl: "",
  description: "",
  startedAt: null as string | null,
  testingAt: null as string | null,
  completedAt: null as string | null,
});
const itemPrimaryPage = ref<PmSiyuanPageRef | null>(null);
const itemExtraPages = ref<PmSiyuanPageRef[]>([]);

const siyuanDrawerVisible = ref(false);
const siyuanForm = reactive({
  baseUrl: getSetting("pm_siyuan_base_url") ?? defaultBaseUrl,
  token: getSetting("pm_siyuan_token") ?? "",
});
const globalSiyuanLocation = ref<PmSiyuanLocation | null>(
  getSettingJson<PmSiyuanLocation | null>(PM_SIYUAN_DEFAULT_LOCATION_KEY, null),
);
const globalSiyuanLocationDraft = ref<PmSiyuanLocation | null>(
  globalSiyuanLocation.value ? { ...globalSiyuanLocation.value } : null,
);
const siyuanShowToken = ref(false);
const siyuanTesting = ref(false);
const siyuanTestingVersion = ref("");
const siyuanLoadingDirectory = ref(false);
const siyuanDirectory = ref<PmSiyuanNotebookDirectory[]>([]);
const siyuanDirectoryFetchedAt = ref("");
let siyuanDirectoryLoadPromise: Promise<boolean> | null = null;
const siyuanError = ref("");
const siyuanErrorContext = ref<"test" | "directory" | null>(null);
const siyuanTreeProps = { label: "name", children: "children" };
const siyuanLocationDialogVisible = ref(false);
const siyuanLocationPickerTarget = ref<"global" | "project">("global");
const siyuanLocationPickerValue = ref<PmSiyuanLocation | null>(null);
const siyuanLocationPickerSearch = ref("");
const siyuanPageDialogVisible = ref(false);
const siyuanPageDialogMode = ref<"primary" | "extra">("primary");
const siyuanPageDialogIntent = ref<"link" | "replace-primary">("link");
const siyuanPageDialogSessionId = ref(0);
const siyuanPageFilterKeyword = ref("");
const siyuanPageResultSource = ref<"location" | "all">("location");
const siyuanPageLocationResults = ref<PmSiyuanPageRef[]>([]);
const siyuanPageAllResults = ref<PmSiyuanPageRef[]>([]);
const siyuanPageSearchingAll = ref(false);
const siyuanPageLocationState = ref<PmSiyuanPageLocationState>("ready");
const siyuanPageLocationRefreshError = ref("");
const siyuanPageCreating = ref(false);
const siyuanErrorTitle = computed(() => {
  if (siyuanErrorContext.value === "test") {
    return "连接失败";
  }
  if (siyuanErrorContext.value === "directory") {
    return "目录加载失败";
  }
  return "错误";
});

// Click debounce
const clickTimer = ref<ReturnType<typeof setTimeout> | null>(null);
const PM_CARD_SINGLE_CLICK_DELAY_MS = 320;

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
const ctxMenuRef = ref<HTMLElement | null>(null);

const PM_CTX_MENU_WIDTH = 168;
const PM_CTX_MENU_ITEM_HEIGHT = 34;
const PM_CTX_MENU_DIVIDER_HEIGHT = 9;
const PM_CTX_MENU_VERTICAL_PADDING = 8;
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
const selectedItemDescriptionText = computed(() => selectedItem.value?.description?.trim() ?? "");
const itemDialogProjectId = computed<number | null>(() => {
  if (itemFormProjectId.value !== null) {
    return itemFormProjectId.value;
  }
  if (editingItem.value) {
    return editingItem.value.projectId;
  }
  return typeof selectedProjectId.value === "number" ? selectedProjectId.value : null;
});
const itemProjectOptions = computed(() => {
  const active = [...activeProjects.value];
  const currentProjectId = itemFormProjectId.value ?? editingItem.value?.projectId ?? null;
  if (currentProjectId === null || active.some((project) => project.id === currentProjectId)) {
    return active;
  }

  const currentProject = projects.value.find((project) => project.id === currentProjectId);
  return currentProject ? [...active, currentProject] : active;
});
const itemDialogProject = computed(() =>
  projects.value.find((project) => project.id === itemDialogProjectId.value) ?? null,
);
const itemDialogProjectDisplayName = computed(() => {
  if (itemDialogProject.value) {
    return itemDialogProject.value.name;
  }
  if (typeof selectedProjectId.value === "number") {
    return selectedProject.value?.name ?? "未选择项目";
  }
  return "未选择项目";
});
const itemDialogProjectDisplayColor = computed(() => {
  if (itemDialogProject.value?.color) {
    return itemDialogProject.value.color;
  }
  if (typeof selectedProjectId.value === "number") {
    return selectedProject.value?.color ?? "#4d7df2";
  }
  return "#4d7df2";
});
const itemDialogProjectPendingCount = computed(() =>
  itemDialogProject.value ? getPmPendingCount(projectItemCounts.value[itemDialogProject.value.id]) : 0,
);
const itemEffectiveLocation = computed(() =>
  resolvePmSiyuanEffectiveLocation(itemDialogProject.value?.siyuanLocationOverride, globalSiyuanLocation.value),
);
const itemEffectiveLocationSource = computed(() => {
  if (itemDialogProject.value?.siyuanLocationOverride) {
    return "项目专属位置";
  }
  if (globalSiyuanLocation.value) {
    return "全局默认位置";
  }
  return "未配置";
});
const itemSiyuanLocationSummary = computed(() => {
  if (!itemEffectiveLocation.value) {
    return "未配置默认位置";
  }
  if (
    siyuanErrorContext.value === "directory" &&
    !siyuanDirectoryFetchedAt.value &&
    siyuanDirectory.value.length === 0
  ) {
    return "位置读取失败";
  }
  if (siyuanDirectoryFetchedAt.value || siyuanDirectory.value.length > 0) {
    const result = collectPmSiyuanPagesForLocation(siyuanDirectory.value, itemEffectiveLocation.value);
    if (result.state === "invalid-location") {
      return "位置已失效";
    }
  }
  return `${itemEffectiveLocationSource.value} · ${formatPmSiyuanLocationLabel(itemEffectiveLocation.value)}`;
});
const itemLinkedPages = computed<ItemSiyuanLinkedRow[]>(() => [
  ...(itemPrimaryPage.value ? [{ page: itemPrimaryPage.value, kind: "primary" as const }] : []),
  ...itemExtraPages.value.map((page) => ({ page, kind: "extra" as const })),
]);
const itemFormDateRange = computed<[string, string] | null>({
  get() {
    return getPmDateRangeValue(itemForm.value.startAt, itemForm.value.endAt);
  },
  set(value) {
    if (!value || value.length < 2) {
      itemForm.value.startAt = null;
      itemForm.value.endAt = null;
      return;
    }
    const normalizedRange = normalizePmDateRangeForDraft(value[0], value[1]);
    itemForm.value.startAt = normalizedRange.startAt;
    itemForm.value.endAt = normalizedRange.endAt;
  },
});
const itemFormOpenableLink = computed(() => Boolean(normalizeItemLinkUrl(itemForm.value.linkUrl)));
const siyuanLocationPickerTitle = computed(() =>
  siyuanLocationPickerTarget.value === "global" ? "选择任务默认存储位置" : "选择项目专属存储位置",
);
const siyuanLocationPickerSearchKeyword = computed(() => siyuanLocationPickerSearch.value.trim());
const siyuanLocationPickerTreeData = computed(() => {
  if (!siyuanLocationPickerSearchKeyword.value) {
    return siyuanDirectory.value;
  }
  return filterPmSiyuanDirectory(siyuanDirectory.value, siyuanLocationPickerSearchKeyword.value);
});
const siyuanLocationPickerExpandedKeys = computed(() => {
  if (!siyuanLocationPickerSearchKeyword.value) {
    return [];
  }
  return collectPmSiyuanExpandedKeys(siyuanLocationPickerTreeData.value);
});
const siyuanLocationPickerTreeKey = computed(
  () =>
    `${siyuanLocationPickerTarget.value}:${siyuanLocationPickerSearchKeyword.value}:${
      siyuanLocationPickerExpandedKeys.value.join("|")
    }:${siyuanLocationPickerValue.value?.parentDocId ?? siyuanLocationPickerValue.value?.notebookId ?? "none"}`,
);
const siyuanLocationPickerCurrentNodeKey = computed(
  () => siyuanLocationPickerValue.value?.parentDocId ?? siyuanLocationPickerValue.value?.notebookId ?? undefined,
);
const siyuanLocationPickerSelectionTarget = computed(() =>
  formatPmSiyuanLocationTargetLabel(siyuanLocationPickerValue.value),
);
const siyuanLocationPickerSelectionPath = computed(() =>
  formatPmSiyuanLocationPathLabel(siyuanLocationPickerValue.value),
);
const siyuanLocationPickerStatusText = computed(() =>
  siyuanLocationPickerSearchKeyword.value
    ? `已按“${siyuanLocationPickerSearchKeyword.value}”过滤目录，只保留命中的目录路径。`
    : "默认仅展开笔记本一级，点击文档后会把新页面放到该文档下面。",
);
const siyuanConfigReady = computed(() => Boolean(getSiyuanConfigSnapshot()));
const siyuanPageFilterKeywordTrimmed = computed(() => siyuanPageFilterKeyword.value.trim());
const siyuanPageFilteredLocationResults = computed(() =>
  filterPmSiyuanPages(siyuanPageLocationResults.value, siyuanPageFilterKeyword.value),
);
const siyuanPageDisplayedResults = computed(() =>
  siyuanPageResultSource.value === "all" ? siyuanPageAllResults.value : siyuanPageFilteredLocationResults.value,
);
const siyuanPageDialogInputPlaceholder = computed(() =>
  itemEffectiveLocation.value && siyuanConfigReady.value
    ? "输入标题或路径过滤当前列表"
    : "输入关键词后点击扩展到全库",
);
const siyuanPageCreateTitle = computed(() => {
  const itemTitle = itemForm.value.title.trim();
  if (!itemEffectiveLocation.value) {
    return "";
  }
  if (
    siyuanPageFilterKeywordTrimmed.value &&
    !siyuanPageShowLocationLoading.value &&
    !siyuanPageShowAllLoading.value &&
    siyuanPageDisplayedResults.value.length === 0
  ) {
    return siyuanPageFilterKeywordTrimmed.value;
  }
  return itemTitle;
});
const siyuanPageCanCreateImmediately = computed(() => {
  if (!itemEffectiveLocation.value || !siyuanPageCreateTitle.value) {
    return false;
  }
  if (
    siyuanPageLocationState.value === "missing-location" ||
    siyuanPageLocationState.value === "invalid-location" ||
    siyuanPageLocationState.value === "missing-config"
  ) {
    return false;
  }
  if (siyuanPageLocationState.value === "load-error" && siyuanPageLocationResults.value.length === 0) {
    return false;
  }
  return true;
});
const siyuanPageDialogTitle = computed(() => {
  if (siyuanPageDialogIntent.value === "replace-primary") {
    return "更换思源主页面";
  }
  return siyuanPageDialogMode.value === "primary" ? "关联思源主页面" : "添加思源附加页面";
});
const siyuanPageCurrentRangeText = computed(() => {
  if (siyuanPageResultSource.value === "all") {
    return `当前列表范围：本次全库搜索结果（当前显示 ${siyuanPageAllResults.value.length} 条）`;
  }
  if (itemEffectiveLocation.value) {
    return `当前列表范围：${formatPmSiyuanLocationLabel(itemEffectiveLocation.value)}（共 ${siyuanPageLocationResults.value.length} 篇）`;
  }
  return "当前列表范围：未配置当前位置";
});
const siyuanPageFilterSummary = computed(() => {
  if (siyuanPageResultSource.value === "all") {
    return siyuanPageFilterKeywordTrimmed.value ? `当前关键词：${siyuanPageFilterKeywordTrimmed.value}` : "";
  }
  if (siyuanPageLocationState.value !== "ready") {
    return "";
  }
  if (!siyuanPageFilterKeywordTrimmed.value) {
    return "";
  }
  return `当前过滤命中 ${siyuanPageFilteredLocationResults.value.length} 条，完整列表共 ${siyuanPageLocationResults.value.length} 篇。`;
});
const siyuanPageShowReturnToLocation = computed(
  () => siyuanPageResultSource.value === "all" && Boolean(itemEffectiveLocation.value),
);
const siyuanPageShowLocationLoading = computed(
  () =>
    siyuanPageResultSource.value === "location" &&
    siyuanLoadingDirectory.value &&
    siyuanPageLocationResults.value.length === 0 &&
    siyuanPageLocationState.value === "ready" &&
    siyuanConfigReady.value &&
    Boolean(itemEffectiveLocation.value),
);
const siyuanPageShowAllLoading = computed(
  () => siyuanPageResultSource.value === "all" && siyuanPageSearchingAll.value && siyuanPageAllResults.value.length === 0,
);
const siyuanPageEmptyMessage = computed(() => {
  if (siyuanPageResultSource.value === "all") {
    return siyuanPageSearchingAll.value ? "" : "全库中没有找到匹配文档，请调整关键词后重试。";
  }

  switch (siyuanPageLocationState.value) {
    case "missing-location":
      return "当前未配置项目专属位置或全局默认位置，无法展示当前位置列表；你仍可输入关键词后手动扩展到全库搜索。";
    case "missing-config":
      return "当前缺少思源服务地址或 API Token，请先完成思源配置。";
    case "load-error":
      return "当前位置列表加载失败，请稍后重试。";
    case "invalid-location":
      return "当前默认位置已失效，或所在笔记本已关闭，请重新选择位置。";
    case "empty":
      return "当前位置暂无可关联文档，可以直接新建页面。";
    case "ready":
      return siyuanPageFilteredLocationResults.value.length === 0 ? "当前过滤条件下没有匹配文档。" : "";
    default:
      return "";
  }
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

const ganttFilteredItems = computed(() =>
  filterPmItemsByGanttStatuses(baseFilteredItems.value, ganttSelectedStatuses.value),
);

const columnItemsMap = computed(() => {
  const items = baseFilteredItems.value;
  const map = new Map<PmItemStatus, PmItem[]>();
  for (const col of PM_STATUS_COLUMNS) {
    map.set(col.key, []);
  }
  for (const item of items) {
    map.get(item.status)?.push(item);
  }
  return map;
});

function columnItems(status: PmItemStatus) {
  return columnItemsMap.value.get(status) ?? [];
}

function onGanttToggleStatus(payload: { status: PmItemStatus }) {
  ganttSelectedStatuses.value = togglePmGanttStatus(ganttSelectedStatuses.value, payload.status);
  closeCtxMenu();
}

function selectAllGanttStatuses() {
  ganttSelectedStatuses.value = selectAllPmGanttStatuses();
  closeCtxMenu();
}

function clearGanttStatuses() {
  ganttSelectedStatuses.value = clearPmGanttStatuses();
  closeCtxMenu();
}

// ── Helpers ──────────────────────────────────────────────

function isOverdue(item: PmItem): boolean {
  return isPmItemOverdue(item);
}

function getItemTagSummary(item: PmItem) {
  return summarizePmItemTags(item.tags);
}

function getPmLightTagStyle(color?: string | null) {
  const resolvedColor = color ?? "#409eff";
  return {
    "--el-tag-bg-color": `${resolvedColor}14`,
    "--el-tag-border-color": `${resolvedColor}33`,
    "--el-tag-text-color": resolvedColor,
  };
}

function nextStatusLabel(item: PmItem): string {
  const idx = PM_STATUS_COLUMNS.findIndex((c) => c.key === item.status);
  return idx >= 0 && idx < PM_STATUS_COLUMNS.length - 1 ? PM_STATUS_COLUMNS[idx + 1].label : "";
}

function formatItemProjectOptionLabel(project: PmProject): string {
  return project.status === "archived" ? `${project.name}（已归档）` : project.name;
}

function cloneSiyuanLocation(location: PmSiyuanLocation | null | undefined): PmSiyuanLocation | null {
  return location ? { ...location } : null;
}

function cloneSiyuanPage(page: PmSiyuanPageRef | null | undefined): PmSiyuanPageRef | null {
  return page ? { ...page } : null;
}

function cloneSiyuanPages(pages: PmSiyuanPageRef[] | null | undefined): PmSiyuanPageRef[] {
  return (pages ?? []).map((page) => ({ ...page }));
}

function resetSiyuanPageDialogState(mode: "primary" | "extra") {
  siyuanPageDialogSessionId.value += 1;
  siyuanPageDialogMode.value = mode;
  siyuanPageDialogIntent.value = "link";
  siyuanPageFilterKeyword.value = "";
  siyuanPageResultSource.value = "location";
  siyuanPageLocationResults.value = [];
  siyuanPageAllResults.value = [];
  siyuanPageSearchingAll.value = false;
  siyuanPageLocationState.value = "ready";
  siyuanPageLocationRefreshError.value = "";
}

function applySiyuanPageLocationResultsFromDirectory() {
  const location = itemEffectiveLocation.value;
  if (!siyuanConfigReady.value) {
    siyuanPageLocationResults.value = [];
    siyuanPageLocationState.value = "missing-config";
    siyuanPageLocationRefreshError.value = "";
    return;
  }
  if (!location) {
    siyuanPageLocationResults.value = [];
    siyuanPageLocationState.value = "missing-location";
    siyuanPageLocationRefreshError.value = "";
    return;
  }

  const result = collectPmSiyuanPagesForLocation(siyuanDirectory.value, location);
  siyuanPageLocationResults.value = cloneSiyuanPages(result.pages);
  siyuanPageLocationState.value = result.state;
  siyuanPageLocationRefreshError.value = "";
}

async function refreshSiyuanPageLocationResults(options: { keepResultsOnError?: boolean; sessionId?: number } = {}) {
  const { keepResultsOnError = false, sessionId = siyuanPageDialogSessionId.value } = options;

  if (!siyuanConfigReady.value) {
    siyuanPageLocationResults.value = [];
    siyuanPageLocationState.value = "missing-config";
    siyuanPageLocationRefreshError.value = "";
    return;
  }
  if (!itemEffectiveLocation.value) {
    siyuanPageLocationResults.value = [];
    siyuanPageLocationState.value = "missing-location";
    siyuanPageLocationRefreshError.value = "";
    return;
  }

  const success = await refreshSiyuanDirectory({ showSuccess: false });
  if (sessionId !== siyuanPageDialogSessionId.value) {
    return;
  }
  if (success) {
    applySiyuanPageLocationResultsFromDirectory();
    return;
  }

  if (!keepResultsOnError) {
    siyuanPageLocationResults.value = [];
    siyuanPageLocationState.value = "load-error";
  }
  siyuanPageLocationRefreshError.value = siyuanError.value || "当前位置列表加载失败，请稍后重试。";
}

async function ensureSiyuanDirectoryLoaded() {
  if (siyuanDirectory.value.length > 0) {
    return;
  }
  await refreshSiyuanDirectory({ showSuccess: false });
}

function applyItemPrimaryPage(page: PmSiyuanPageRef | null) {
  const result = setPmSiyuanPrimaryPage(itemPrimaryPage.value, itemExtraPages.value, page);
  itemPrimaryPage.value = result.primaryPage ? { ...result.primaryPage } : null;
  itemExtraPages.value = cloneSiyuanPages(result.extraPages);
}

function addItemExtraPage(page: PmSiyuanPageRef) {
  itemExtraPages.value = addPmSiyuanExtraPage(itemPrimaryPage.value, itemExtraPages.value, page).map(
    (item) => ({ ...item }),
  );
}

function hasItemLinkedPage(docId: string): boolean {
  return itemPrimaryPage.value?.docId === docId || itemExtraPages.value.some((page) => page.docId === docId);
}

function removeItemLinkedPage(docId: string) {
  const result = removePmSiyuanPage(itemPrimaryPage.value, itemExtraPages.value, docId);
  itemPrimaryPage.value = result.primaryPage ? { ...result.primaryPage } : null;
  itemExtraPages.value = cloneSiyuanPages(result.extraPages);
}

function buildSiyuanLocationFromTreeNode(
  data: PmSiyuanNotebookDirectory | PmSiyuanTreeNode,
  node: { level: number; parent?: { level: number; data?: unknown; parent?: unknown } | null },
): PmSiyuanLocation | null {
  if (isPmSiyuanNotebookDirectory(data)) {
    if (data.closed) {
      return null;
    }
    return {
      notebookId: data.id,
      notebookName: data.name,
      parentDocId: null,
      parentDocTitle: null,
      parentHpath: null,
      parentPath: null,
    };
  }

  let current = node.parent ?? null;
  while (current && current.level > 1) {
    current = (current.parent as typeof current | null) ?? null;
  }
  const notebook = current?.data as PmSiyuanNotebookDirectory | undefined;
  if (!notebook || notebook.closed) {
    return null;
  }

  return {
    notebookId: notebook.id,
    notebookName: notebook.name,
    parentDocId: data.id,
    parentDocTitle: data.name,
    parentHpath: data.hpath,
    parentPath: data.path,
  };
}

function isSiyuanLocationPickerNodeSelected(data: PmSiyuanNotebookDirectory | PmSiyuanTreeNode) {
  return siyuanLocationPickerCurrentNodeKey.value === data.id;
}

function isSiyuanLocationPickerNodeDisabled(
  data: PmSiyuanNotebookDirectory | PmSiyuanTreeNode,
  node: { level: number; parent?: { level: number; data?: unknown; parent?: unknown } | null },
) {
  return buildSiyuanLocationFromTreeNode(data, node) === null;
}

async function openSiyuanLocationPicker(target: "global" | "project") {
  siyuanLocationPickerTarget.value = target;
  siyuanLocationPickerSearch.value = "";
  siyuanLocationPickerValue.value = cloneSiyuanLocation(
    target === "global" ? globalSiyuanLocationDraft.value : projectForm.value.siyuanLocationOverride,
  );
  siyuanLocationDialogVisible.value = true;
  if (siyuanDirectory.value.length === 0) {
    await ensureSiyuanDirectoryLoaded();
    return;
  }
  void refreshSiyuanDirectory({ showSuccess: false });
}

function handleSiyuanLocationTreeNodeClick(
  data: PmSiyuanNotebookDirectory | PmSiyuanTreeNode,
  node: { level: number; parent?: { level: number; data?: unknown; parent?: unknown } | null },
) {
  const location = buildSiyuanLocationFromTreeNode(data, node);
  if (!location) {
    ElMessage.warning("关闭的笔记本不能作为默认存储位置");
    return;
  }
  siyuanLocationPickerValue.value = location;
}

function applySiyuanLocationPicker() {
  const location = cloneSiyuanLocation(siyuanLocationPickerValue.value);
  if (siyuanLocationPickerTarget.value === "global") {
    globalSiyuanLocationDraft.value = location;
  } else {
    projectForm.value.useSiyuanOverride = Boolean(location);
    projectForm.value.siyuanLocationOverride = location;
  }
  siyuanLocationDialogVisible.value = false;
}

function clearSiyuanLocationPicker() {
  siyuanLocationPickerValue.value = null;
}

function clearProjectSiyuanOverride() {
  projectForm.value.useSiyuanOverride = false;
  projectForm.value.siyuanLocationOverride = null;
}

async function openSiyuanPageDialog(
  mode: "primary" | "extra",
  intent: "link" | "replace-primary" = "link",
) {
  resetSiyuanPageDialogState(mode);
  siyuanPageDialogIntent.value = intent;
  if (!editingItem.value && mode === "primary") {
    siyuanPageFilterKeyword.value = itemForm.value.title.trim();
  }
  const sessionId = siyuanPageDialogSessionId.value;
  siyuanPageDialogVisible.value = true;

  if (!siyuanConfigReady.value) {
    siyuanPageLocationState.value = "missing-config";
    return;
  }
  if (!itemEffectiveLocation.value) {
    siyuanPageLocationState.value = "missing-location";
    return;
  }

  if (siyuanDirectory.value.length > 0) {
    applySiyuanPageLocationResultsFromDirectory();
    void refreshSiyuanPageLocationResults({ keepResultsOnError: true, sessionId });
    return;
  }

  await refreshSiyuanPageLocationResults({ sessionId });
}

function restoreSiyuanLocationResults() {
  siyuanPageResultSource.value = "location";
  siyuanPageAllResults.value = [];
}

async function expandSiyuanPagesToAll() {
  const keyword = siyuanPageFilterKeywordTrimmed.value;
  const sessionId = siyuanPageDialogSessionId.value;
  if (keyword.length < 2) {
    ElMessage.warning("请输入至少 2 个字符后再扩展到全库");
    return;
  }

  try {
    ensureSiyuanConfig();
  } catch (error) {
    ElMessage.warning((error as Error).message);
    return;
  }

  try {
    siyuanPageSearchingAll.value = true;
    const result = (await invoke<PmSiyuanSearchResult>("tool:pm:siyuan-search-pages", {
      keyword,
      searchAll: true,
      location: null,
    })) ?? { items: [], scope: "all" };
    if (sessionId !== siyuanPageDialogSessionId.value) {
      return;
    }
    siyuanPageAllResults.value = cloneSiyuanPages(result.items ?? []);
    siyuanPageResultSource.value = "all";
  } catch (error) {
    if (sessionId !== siyuanPageDialogSessionId.value) {
      return;
    }
    ElMessage.error((error as Error).message);
  } finally {
    if (sessionId === siyuanPageDialogSessionId.value) {
      siyuanPageSearchingAll.value = false;
    }
  }
}

function selectSiyuanPageResult(page: PmSiyuanPageRef) {
  if (siyuanPageDialogIntent.value === "replace-primary") {
    if (itemPrimaryPage.value?.docId === page.docId) {
      siyuanPageDialogVisible.value = false;
      return;
    }
    applyItemPrimaryPage(page);
    siyuanPageDialogVisible.value = false;
    return;
  }

  if (hasItemLinkedPage(page.docId)) {
    ElMessage.info("该页面已存在，无需重复关联。");
    siyuanPageDialogVisible.value = false;
    return;
  }

  if (itemPrimaryPage.value) {
    addItemExtraPage(page);
  } else {
    applyItemPrimaryPage(page);
  }
  siyuanPageDialogVisible.value = false;
}

async function createSiyuanPageForItem() {
  const title = siyuanPageCreateTitle.value;
  if (!title) {
    ElMessage.warning("请先填写工作项标题，或输入想创建的页面标题");
    return;
  }
  if (!itemEffectiveLocation.value) {
    ElMessage.warning("当前没有可用的思源默认位置，请先在配置或项目设置里指定");
    return;
  }

  try {
    siyuanPageCreating.value = true;
    const result = (await invoke<{ created: boolean; page: PmSiyuanPageRef }>(
      "tool:pm:siyuan-create-page",
      {
        title,
        description: itemForm.value.description,
        projectName: itemDialogProject.value?.name ?? "未归项目",
        status: itemForm.value.status,
        priority: itemForm.value.priority,
        startAt: normalizePmDateRangeForDraft(itemForm.value.startAt, itemForm.value.endAt).startAt,
        endAt: normalizePmDateRangeForDraft(itemForm.value.startAt, itemForm.value.endAt).endAt,
        location: itemEffectiveLocation.value,
      },
    )) as { created: boolean; page: PmSiyuanPageRef };

    if (!result?.page) {
      throw new Error("思源页面创建结果为空");
    }

    if (!result.created) {
      await ElMessageBox.confirm(
        `同一路径下已存在页面「${result.page.docTitle}」，是否直接关联这个已有页面？`,
        "页面已存在",
        {
          type: "warning",
          confirmButtonText: "关联现有页面",
          cancelButtonText: "取消",
        },
      );
    } else {
      ElMessage.success("思源页面已创建。若稍后取消工作项保存，该页面会保留但不会自动绑定。");
    }

    selectSiyuanPageResult(result.page);
  } catch (error) {
    if ((error as string) !== "cancel") {
      ElMessage.error((error as Error).message);
    }
  } finally {
    siyuanPageCreating.value = false;
  }
}

async function openSiyuanPage(page: PmSiyuanPageRef | null | undefined) {
  if (!page) return;
  try {
    await invoke("tool:pm:siyuan-open-page", { docId: page.docId });
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
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
  loadItemCounts();
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

function openSiyuanDrawer() {
  siyuanDrawerVisible.value = true;
}

function normalizeBaseUrl(value: string): string {
  let url = value.trim();
  if (!url) return "";
  if (!/^https?:\/\//i.test(url)) {
    url = `http://${url}`;
  }
  while (url.endsWith("/")) {
    url = url.slice(0, -1);
  }
  return url;
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
  if (!normalized) {
    return;
  }
  try {
    await invoke("tool:pm:open-link", { url: normalized });
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function openItemFormLink() {
  await openItemLink(itemForm.value.linkUrl);
}

function getSiyuanConfigSnapshot(): { baseUrl: string; token: string } | null {
  const baseUrl = normalizeBaseUrl(siyuanForm.baseUrl);
  const token = (siyuanForm.token ?? "").trim();
  if (!baseUrl || !token) {
    return null;
  }
  return { baseUrl, token };
}

function ensureSiyuanConfig(): { baseUrl: string; token: string } {
  const baseUrl = normalizeBaseUrl(siyuanForm.baseUrl);
  if (!baseUrl) {
    throw new Error("请填写思源服务地址");
  }
  const token = (siyuanForm.token ?? "").trim();
  if (!token) {
    throw new Error("请填写 API Token");
  }
  return { baseUrl, token };
}

function saveSiyuanConfig() {
  try {
    const { baseUrl, token } = ensureSiyuanConfig();
    setSetting("pm_siyuan_base_url", baseUrl);
    setSetting("pm_siyuan_token", token);
    setSettingJson(PM_SIYUAN_DEFAULT_LOCATION_KEY, globalSiyuanLocationDraft.value);
    siyuanForm.baseUrl = baseUrl;
    siyuanForm.token = token;
    globalSiyuanLocation.value = cloneSiyuanLocation(globalSiyuanLocationDraft.value);
    ElMessage.success("配置已保存");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function handleTestConnection() {
  try {
    const { baseUrl, token } = ensureSiyuanConfig();
    siyuanTesting.value = true;
    siyuanError.value = "";
    siyuanErrorContext.value = null;
    const result = (await invoke<{ version?: string }>("tool:pm:siyuan-test", { baseUrl, token })) ?? {};
    siyuanTestingVersion.value = result.version ?? "未知版本";
    ElMessage.success("连接成功");
  } catch (error) {
    siyuanTestingVersion.value = "";
    siyuanError.value = (error as Error).message;
    siyuanErrorContext.value = "test";
  } finally {
    siyuanTesting.value = false;
  }
}

async function handleLoadDirectory() {
  await refreshSiyuanDirectory({ showSuccess: true });
}

async function refreshSiyuanDirectory(options: { showSuccess?: boolean } = {}): Promise<boolean> {
  if (siyuanDirectoryLoadPromise) {
    return siyuanDirectoryLoadPromise;
  }

  const { showSuccess = true } = options;
  siyuanDirectoryLoadPromise = (async () => {
    try {
      const { baseUrl, token } = ensureSiyuanConfig();
      siyuanLoadingDirectory.value = true;
      siyuanError.value = "";
      siyuanErrorContext.value = null;
      const directory =
        (await invoke<PmSiyuanDirectoryResult>("tool:pm:siyuan-directory", { baseUrl, token })) ?? {
          notebooks: [],
          fetchedAt: "",
        };
      siyuanDirectory.value = directory.notebooks;
      siyuanDirectoryFetchedAt.value = directory.fetchedAt
        ? formatDateTime(directory.fetchedAt)
        : new Date().toLocaleString();
      if (showSuccess) {
        ElMessage.success("目录已加载");
      }
      return true;
    } catch (error) {
      siyuanError.value = (error as Error).message;
      siyuanErrorContext.value = "directory";
      return false;
    } finally {
      siyuanLoadingDirectory.value = false;
      siyuanDirectoryLoadPromise = null;
    }
  })();

  return siyuanDirectoryLoadPromise;
}

function handleRefreshSiyuanLocationPicker() {
  void refreshSiyuanDirectory({ showSuccess: true });
}

function selectProject(id: number | "overview") {
  selectedProjectId.value = id;
  selectedItemId.value = null;
}

function selectItem(item: PmItem) {
  selectedItemId.value = item.id;
}

function onCardClick(event: MouseEvent, item: PmItem) {
  if (event.detail > 1 || clickTimer.value) return;
  clickTimer.value = setTimeout(() => {
    clickTimer.value = null;
    selectItem(item);
  }, PM_CARD_SINGLE_CLICK_DELAY_MS);
}

function onCardDblclick(item: PmItem) {
  if (clickTimer.value) {
    clearTimeout(clickTimer.value);
    clickTimer.value = null;
  }
  editItem(item);
}

watch(selectedProjectId, () => {
  loadItems();
});

watch(siyuanDrawerVisible, (visible) => {
  if (visible) {
    siyuanForm.baseUrl = getSetting("pm_siyuan_base_url") ?? defaultBaseUrl;
    siyuanForm.token = getSetting("pm_siyuan_token") ?? "";
    globalSiyuanLocationDraft.value = cloneSiyuanLocation(
      getSettingJson<PmSiyuanLocation | null>(PM_SIYUAN_DEFAULT_LOCATION_KEY, null),
    );
  }
});

watch(siyuanLocationDialogVisible, (visible) => {
  if (!visible) {
    siyuanLocationPickerSearch.value = "";
  }
});

watch(siyuanPageDialogVisible, (visible, previousVisible) => {
  if (!visible && previousVisible) {
    siyuanPageDialogSessionId.value += 1;
    siyuanPageSearchingAll.value = false;
  }
});

watch(siyuanPageFilterKeyword, (keyword, previousKeyword) => {
  if (!siyuanPageDialogVisible.value) {
    return;
  }
  if (siyuanPageResultSource.value !== "all" || keyword === previousKeyword) {
    return;
  }
  restoreSiyuanLocationResults();
});

watch(
  [siyuanPageDialogVisible, itemEffectiveLocation, siyuanConfigReady, siyuanDirectory, siyuanDirectoryFetchedAt],
  ([visible]) => {
    if (!visible) {
      return;
    }
    if (!siyuanConfigReady.value) {
      siyuanPageLocationResults.value = [];
      siyuanPageLocationState.value = "missing-config";
      siyuanPageLocationRefreshError.value = "";
      return;
    }
    if (!itemEffectiveLocation.value) {
      siyuanPageLocationResults.value = [];
      siyuanPageLocationState.value = "missing-location";
      siyuanPageLocationRefreshError.value = "";
      return;
    }
    if (!siyuanDirectoryFetchedAt.value && siyuanDirectory.value.length === 0) {
      return;
    }
    applySiyuanPageLocationResultsFromDirectory();
  },
  { flush: "post" },
);

// ── Project CRUD ─────────────────────────────────────────

function showCreateProject() {
  editingProject.value = null;
  const randomColor = presetColors[Math.floor(Math.random() * presetColors.length)];
  projectForm.value = {
    name: "",
    description: "",
    color: randomColor,
    useSiyuanOverride: false,
    siyuanLocationOverride: null,
  };
  projectDialogVisible.value = true;
}

function showEditProject(p: PmProject) {
  editingProject.value = p;
  projectForm.value = {
    name: p.name,
    description: p.description,
    color: p.color,
    useSiyuanOverride: Boolean(p.siyuanLocationOverride),
    siyuanLocationOverride: cloneSiyuanLocation(p.siyuanLocationOverride),
  };
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
    const payload = {
      name: projectForm.value.name,
      description: projectForm.value.description,
      color: projectForm.value.color,
      siyuanLocationOverride: projectForm.value.useSiyuanOverride
        ? projectForm.value.siyuanLocationOverride
        : null,
    };
    if (editingProject.value) {
      await invoke("tool:pm:project-update", {
        id: editingProject.value.id,
        ...payload,
        sortOrder: editingProject.value.sortOrder,
      });
    } else {
      await invoke("tool:pm:project-create", payload);
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
    linkUrl: "",
    description: "",
  };
  itemPrimaryPage.value = null;
  itemExtraPages.value = [];
  itemDialogVisible.value = true;
  if (siyuanConfigReady.value && itemEffectiveLocation.value) {
    void ensureSiyuanDirectoryLoaded();
  }
}

function editItem(item: PmItem) {
  const normalizedDateRange = normalizePmDateRangeForDraft(item.startAt, item.endAt);
  editingItem.value = item;
  itemFormProjectId.value = item.projectId;
  itemForm.value = {
    title: item.title,
    itemType: item.itemType,
    priority: item.priority,
    status: item.status,
    startAt: normalizedDateRange.startAt,
    endAt: normalizedDateRange.endAt,
    linkUrl: item.linkUrl ?? "",
    description: item.description,
    startedAt: item.startedAt ?? null,
    testingAt: item.testingAt ?? null,
    completedAt: item.completedAt ?? null,
  };
  itemPrimaryPage.value = cloneSiyuanPage(item.siyuanPrimaryPage);
  itemExtraPages.value = cloneSiyuanPages(item.siyuanExtraPages);
  itemDialogVisible.value = true;
  if (siyuanConfigReady.value && itemEffectiveLocation.value) {
    void ensureSiyuanDirectoryLoaded();
  }
}

function resetItemForm() {
  editingItem.value = null;
  itemFormProjectId.value = null;
  itemPrimaryPage.value = null;
  itemExtraPages.value = [];
}

async function submitItem() {
  if (!itemForm.value.title.trim()) {
    ElMessage.warning("请输入标题");
    return;
  }
  try {
    const normalizedDateRange = normalizePmDateRangeForDraft(itemForm.value.startAt, itemForm.value.endAt);
    // Format timestamp fields: el-date-picker with type="datetime" may return Date objects
    const fmtTs = (v: string | Date | null): string | null => {
      if (!v) return null;
      return v instanceof Date ? v.toISOString() : v;
    };
    const payload = {
      ...itemForm.value,
      startAt: normalizedDateRange.startAt,
      endAt: normalizedDateRange.endAt,
      linkUrl: normalizeItemLinkUrl(itemForm.value.linkUrl) || null,
      siyuanPrimaryPage: itemPrimaryPage.value,
      siyuanExtraPages: itemExtraPages.value,
      startedAt: fmtTs(itemForm.value.startedAt),
      testingAt: fmtTs(itemForm.value.testingAt),
      completedAt: fmtTs(itemForm.value.completedAt),
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
      await invoke("tool:pm:item-create", {
        projectId,
        ...payload,
      });
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

async function openSiyuanLinkPicker() {
  await openSiyuanPageDialog(itemPrimaryPage.value ? "extra" : "primary", "link");
}

async function openReplacePrimarySiyuanDialog() {
  await openSiyuanPageDialog("primary", "replace-primary");
}

function handleItemSiyuanPageCommand(
  row: ItemSiyuanLinkedRow,
  command: string | number | object,
) {
  switch (command) {
    case "replace-primary":
      void openReplacePrimarySiyuanDialog();
      return;
    case "promote-primary":
      applyItemPrimaryPage(row.page);
      return;
    case "remove":
      removeItemLinkedPage(row.page.docId);
      return;
    default:
      return;
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
  openCtxMenuAt(anchorX, anchorY, buildItemContextActions(item));
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

// 过滤条件变化不需要重建 Sortable 实例：
// Sortable 绑定的是 DOM 容器，过滤只改变内部子元素渲染，实例自动跟随

watch(
  () => [selectedProjectId.value, viewMode.value],
  () => closeCtxMenu(),
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
  openCtxMenuAt(event.clientX, event.clientY, actions);
}

function estimateCtxMenuHeight(actions: CtxMenuAction[]): number {
  const dividerCount = actions.filter((action) => action.divider).length;
  const itemCount = actions.length - dividerCount;
  return (
    itemCount * PM_CTX_MENU_ITEM_HEIGHT +
    dividerCount * PM_CTX_MENU_DIVIDER_HEIGHT +
    PM_CTX_MENU_VERTICAL_PADDING
  );
}

function positionCtxMenu(anchorX: number, anchorY: number) {
  const menuWidth = ctxMenuRef.value?.offsetWidth ?? PM_CTX_MENU_WIDTH;
  const menuHeight = ctxMenuRef.value?.offsetHeight ?? estimateCtxMenuHeight(ctxMenuActions.value);
  const position = clampContextMenuPosition({
    anchorX,
    anchorY,
    menuWidth,
    menuHeight,
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
  });
  ctxMenuX.value = position.x;
  ctxMenuY.value = position.y;
}

function openCtxMenuAt(anchorX: number, anchorY: number, actions: CtxMenuAction[]) {
  closeCtxMenu();
  ctxMenuActions.value = actions;
  ctxMenuVisible.value = true;
  ctxMenuX.value = anchorX;
  ctxMenuY.value = anchorY;
  nextTick(() => {
    positionCtxMenu(anchorX, anchorY);
  });
  // 用 setTimeout（宏任务）注册监听器，避免 nextTick（微任务）
  // 在当前 contextmenu 事件冒泡完成前执行导致菜单被立即关闭
  setTimeout(() => {
    if (!ctxMenuVisible.value) return;
    document.addEventListener("pointerdown", handleCtxClickAway);
    document.addEventListener("keydown", handleCtxKeydown);
    document.addEventListener("contextmenu", handleCtxGlobalContextMenu);
    document.addEventListener("scroll", handleCtxViewportChange, true);
    window.addEventListener("resize", handleCtxViewportChange);
  }, 0);
}

function closeCtxMenu() {
  ctxMenuVisible.value = false;
  document.removeEventListener("pointerdown", handleCtxClickAway);
  document.removeEventListener("keydown", handleCtxKeydown);
  document.removeEventListener("contextmenu", handleCtxGlobalContextMenu);
  document.removeEventListener("scroll", handleCtxViewportChange, true);
  window.removeEventListener("resize", handleCtxViewportChange);
}

function handleCtxClickAway(e: PointerEvent) {
  const target = e.target;
  if (!(target instanceof Element) || !target.closest(".pm-ctx-menu")) {
    closeCtxMenu();
  }
}

function executeCtxAction(act: CtxMenuAction) {
  closeCtxMenu();
  void act.action();
}

function handleCtxKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeCtxMenu();
  }
}

function handleCtxGlobalContextMenu(event: MouseEvent) {
  const target = event.target;
  if (target instanceof Element && target.closest(".pm-ctx-menu")) {
    return;
  }
  closeCtxMenu();
}

function handleCtxViewportChange() {
  closeCtxMenu();
}

// ── Formatting ───────────────────────────────────────────

function formatDateTime(dateStr: string): string {
  if (!dateStr) return "";
  const d = new Date(dateStr);
  return d.toLocaleString("zh-CN");
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

/* Detail panel (floating overlay) */
.pm-detail {
  position: absolute;
  top: 0;
  right: 0;
  width: 320px;
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
  font-size: 16px;
}
.detail-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.detail-section {
  background: var(--el-fill-color-lighter);
  border-radius: 8px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.detail-field-inline {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}
.detail-item-title {
  font-size: 16px;
  font-weight: 600;
  line-height: 1.4;
}
.detail-field {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.detail-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-weight: 500;
}
.detail-value {
  font-size: 14px;
  color: var(--el-text-color-primary);
  word-break: break-word;
}
.detail-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.detail-link-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}
.detail-link-text {
  flex: 1;
  min-width: 0;
  word-break: break-all;
}
.detail-description {
  margin: 0;
  font-family: inherit;
  white-space: pre-wrap;
  background: var(--el-fill-color-lighter);
  border-radius: 4px;
  padding: 6px 8px;
  font-size: 13px;
  color: var(--el-text-color-regular);
}
.detail-readonly {
  font-size: 14px;
  color: var(--el-text-color-secondary);
}
.detail-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 12px;
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

.sidebar-title {
  font-size: 18px;
  font-weight: 700;
  line-height: 1.2;
  color: var(--pm-text-main);
}

.sidebar-create-btn {
  min-height: 36px;
  padding-inline: 14px;
  border-radius: 12px;
  box-shadow: 0 10px 22px rgba(77, 125, 242, 0.2);
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

.project-card .project-count,
.sidebar-overview-card .project-count {
  min-width: 28px;
  padding: 0 8px;
  line-height: 22px;
  font-size: 12px;
  font-weight: 700;
  text-align: center;
  color: var(--pm-accent);
  background: rgba(77, 125, 242, 0.1);
  border-radius: 999px;
}

.project-card .project-count {
  margin-left: auto;
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
  padding: 14px 18px;
  border-bottom: 1px solid var(--pm-edge);
  background: rgba(244, 247, 251, 0.88);
  backdrop-filter: blur(12px);
}

.project-title-display {
  font-size: 18px;
  font-weight: 700;
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

.pm-detail {
  width: 364px;
  padding: 16px;
  border-left: 1px solid var(--pm-edge);
  background: linear-gradient(180deg, rgba(249, 251, 255, 0.97), rgba(242, 246, 252, 0.98));
  box-shadow: -16px 0 30px rgba(34, 48, 66, 0.08);
}

.detail-header {
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(219, 229, 241, 0.9);
}

.detail-header-eyebrow {
  margin-bottom: 4px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--pm-text-muted);
}

.detail-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.detail-form {
  min-height: calc(100% - 58px);
  gap: 12px;
}

.detail-hero,
.detail-section {
  padding: 14px 16px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 10px 24px rgba(34, 48, 66, 0.05);
}

.detail-hero {
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(244, 247, 251, 0.9));
}

.detail-hero-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
}

.detail-project-chip {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: 100%;
  padding: 6px 10px;
  border: 1px solid rgba(77, 125, 242, 0.14);
  border-radius: 999px;
  background: rgba(77, 125, 242, 0.08);
}

.detail-project-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  box-shadow: 0 0 0 3px rgba(77, 125, 242, 0.12);
  flex-shrink: 0;
}

.detail-project-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 600;
  color: var(--pm-text-main);
}

.detail-item-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.detail-field-inline {
  gap: 8px;
}

.detail-label {
  color: var(--pm-text-muted);
}

.detail-value {
  color: var(--pm-text-main);
}

.detail-section--muted {
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.84), rgba(244, 247, 251, 0.82));
}

.detail-section-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.detail-section-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.detail-section-subtitle {
  font-size: 12px;
  color: var(--pm-text-muted);
}

.detail-timeline-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.detail-timeline-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 76px;
  padding: 12px;
  border: 1px solid rgba(219, 229, 241, 0.95);
  border-radius: 12px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(244, 247, 251, 0.84));
}

.detail-resource-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.detail-resource-card {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 12px;
  border: 1px solid rgba(219, 229, 241, 0.96);
  border-radius: 12px;
  background: rgba(244, 247, 251, 0.76);
}

.detail-resource-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.detail-link-text {
  line-height: 1.55;
}

.detail-description {
  padding: 10px 12px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 12px;
  background: rgba(244, 247, 251, 0.88);
  line-height: 1.7;
  color: var(--pm-text-main);
}

.detail-empty-text {
  color: var(--pm-text-muted);
  font-size: 13px;
  line-height: 1.6;
}

.detail-actions {
  position: sticky;
  bottom: -16px;
  z-index: 1;
  gap: 8px;
  margin-top: auto;
  padding: 14px 0 2px;
  background: linear-gradient(180deg, rgba(242, 246, 252, 0), rgba(242, 246, 252, 0.94) 32%, rgba(242, 246, 252, 1));
}

.detail-actions :deep(.el-button) {
  flex: 1;
}

@media (max-width: 1280px) {
  .pm-detail {
    width: 350px;
  }
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
  font-size: 15px;
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

.pm-siyuan-drawer .el-drawer__body {
  padding: 20px;
}

.pm-form-item-top :deep(.el-form-item__content) {
  align-items: flex-start;
}

.pm-item-dialog-form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.pm-item-project-card {
  border: 1px solid #dbe5f1;
  border-radius: 16px;
  padding: 14px 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(239, 245, 255, 0.94));
  box-shadow: 0 12px 24px rgba(34, 48, 66, 0.06);
}

.pm-item-project-card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}

.pm-item-section-eyebrow {
  margin-bottom: 6px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  color: #6f8098;
}

.pm-item-project-card-main {
  display: flex;
  align-items: center;
  gap: 10px;
}

.pm-item-project-card-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  box-shadow: 0 0 0 4px rgba(77, 125, 242, 0.12);
  flex-shrink: 0;
}

.pm-item-project-card-name {
  font-size: 16px;
  font-weight: 700;
  line-height: 1.3;
  color: #223042;
}

.pm-item-project-card-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin-left: auto;
}

.pm-item-project-select {
  min-width: 176px;
}

.pm-item-project-count-tag,
.project-archived-tag,
.detail-project-archived-tag {
  border-color: rgba(111, 128, 152, 0.24) !important;
  background: rgba(111, 128, 152, 0.1) !important;
  color: #6f8098 !important;
}

.pm-item-project-count-tag {
  padding-inline: 6px;
  font-weight: 700;
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

.pm-item-dialog-core-grid .el-select,
.pm-item-dialog-core-grid .el-date-editor {
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

.pm-input-append-actions {
  display: flex;
  align-items: stretch;
}

.pm-input-append-actions .el-button {
  margin: 0 !important;
  border-radius: 0;
  padding-inline: 12px;
}

.pm-item-project-select .el-input__wrapper {
  min-height: 34px;
}

.pm-siyuan-config-card,
.pm-siyuan-link-card {
  width: 100%;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 14px;
  padding: 14px 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.96), var(--el-fill-color-blank));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.75);
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.pm-siyuan-link-card {
  gap: 14px;
}

.pm-siyuan-config-summary,
.pm-siyuan-link-subtitle,
.pm-siyuan-dialog-hint,
.pm-siyuan-dialog-notice,
.pm-siyuan-picker-intro {
  font-size: 13px;
  line-height: 1.6;
  color: var(--el-text-color-secondary);
}

.pm-siyuan-dialog-notice {
  border-radius: 10px;
  padding: 10px 12px;
  background: var(--el-fill-color-light);
}

.pm-siyuan-dialog-notice--warning {
  color: var(--el-color-warning-dark-2);
  background: var(--el-color-warning-light-9);
  border: 1px solid var(--el-color-warning-light-7);
}

.pm-siyuan-inline-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.pm-siyuan-link-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.pm-siyuan-link-heading {
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-wrap: wrap;
}

.pm-siyuan-link-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.pm-siyuan-link-subtitle {
  display: flex;
  flex-wrap: wrap;
  margin-top: 0;
}

.pm-siyuan-page-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.pm-siyuan-page-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 10px;
  padding: 8px 10px;
  background: var(--el-bg-color);
}

.pm-siyuan-page-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.pm-siyuan-page-row-head {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.pm-siyuan-page-title,
.detail-siyuan-page-title {
  font-size: 14px;
  font-weight: 600;
  line-height: 1.4;
  color: var(--el-text-color-primary);
}

.pm-siyuan-page-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail-siyuan-page-title {
  word-break: break-word;
}

.pm-siyuan-page-meta,
.detail-siyuan-page-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.5;
}

.pm-siyuan-page-meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail-siyuan-page-meta {
  word-break: break-word;
}

.pm-siyuan-page-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.pm-siyuan-more-trigger {
  padding-left: 4px;
  padding-right: 4px;
}

.pm-siyuan-empty-inline {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  line-height: 1.6;
  padding: 2px 0;
}

.pm-siyuan-empty-hint {
  border: 1px dashed var(--el-border-color);
  border-radius: 12px;
  padding: 14px 16px;
  background: var(--el-fill-color-lighter);
  color: var(--el-text-color-secondary);
  font-size: 13px;
  line-height: 1.6;
}

.detail-siyuan-pages {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.detail-siyuan-page {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 10px;
  background: var(--el-fill-color-blank);
}

.detail-siyuan-page-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.siyuan-drawer-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
}

.siyuan-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.siyuan-status {
  min-height: 32px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.siyuan-fetch-hint {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.siyuan-loading-hint {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  margin-bottom: 8px;
}

.siyuan-tree-section {
  flex: 1;
  border-top: 1px solid var(--el-border-color-extra-light);
  padding-top: 12px;
  min-height: 0;
}

.siyuan-tree {
  max-height: 360px;
  overflow: auto;
  padding-right: 4px;
}

.siyuan-tree-node {
  display: flex;
  align-items: center;
  width: 100%;
  min-width: 0;
}

.siyuan-tree-node--preview {
  padding: 2px 0;
}

.siyuan-tree-node--interactive {
  padding: 6px 8px;
  border: 1px solid transparent;
  border-radius: 12px;
  transition: background 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease;
}

.siyuan-tree-node--interactive.is-selected {
  border-color: var(--el-color-primary-light-5);
  background: linear-gradient(135deg, var(--el-color-primary-light-9), rgba(255, 255, 255, 0.96));
  box-shadow: 0 6px 16px rgba(64, 158, 255, 0.08);
}

.siyuan-tree-node--interactive.is-disabled {
  opacity: 0.68;
}

.siyuan-node-main {
  width: 100%;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.siyuan-node-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.siyuan-node-badge {
  flex-shrink: 0;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--el-fill-color);
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.45;
}

.siyuan-node-badge.is-disabled {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}

.pm-siyuan-dialog-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.pm-siyuan-search-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.pm-siyuan-search-toolbar .el-input {
  flex: 1;
  min-width: 220px;
}

.pm-siyuan-dialog-body--picker {
  gap: 14px;
}

.pm-siyuan-picker-selection {
  display: flex;
  flex-direction: column;
  gap: 6px;
  border: 1px solid var(--el-color-primary-light-5);
  border-radius: 12px;
  padding: 12px 16px;
  background: linear-gradient(135deg, var(--el-color-primary-light-9), rgba(255, 255, 255, 0.96));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.86);
}

.pm-siyuan-picker-selection.is-empty {
  border-style: dashed;
  border-color: var(--el-border-color);
  background: linear-gradient(180deg, var(--el-fill-color-lighter), rgba(255, 255, 255, 0.98));
  box-shadow: none;
}

.pm-siyuan-picker-selection-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-weight: 600;
  letter-spacing: 0.04em;
}

.pm-siyuan-picker-selection-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  line-height: 1.3;
}

.pm-siyuan-picker-selection-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.pm-siyuan-picker-selection-path {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
}

.pm-siyuan-picker-selection-empty {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

.pm-siyuan-picker-tree-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.pm-siyuan-picker-tree-shell {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 16px;
  padding: 10px 12px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.97), var(--el-fill-color-blank));
}

.siyuan-error-alert {
  padding: 6px 8px;
}

:deep(.siyuan-tree .el-tree-node__content) {
  min-height: 42px;
  height: auto;
  align-items: center;
  border-radius: 12px;
  margin-bottom: 4px;
  padding-right: 6px;
}

:deep(.siyuan-tree .el-tree-node__content:hover) {
  background: rgba(64, 158, 255, 0.08);
}

:deep(.pm-siyuan-picker-tree .el-tree-node.is-current > .el-tree-node__content) {
  background: rgba(64, 158, 255, 0.12);
}

:deep(.siyuan-tree .el-tree-node__expand-icon) {
  color: var(--el-text-color-secondary);
}

:deep(.siyuan-tree .el-tree-node__expand-icon.expanded) {
  color: var(--el-color-primary);
}

:deep(.siyuan-tree .el-tree-node__label) {
  width: 100%;
  min-width: 0;
}

@media (max-width: 900px) {
  .pm-item-project-card-head,
  .pm-siyuan-link-header,
  .pm-siyuan-page-row,
  .detail-siyuan-page,
  .detail-resource-card {
    flex-direction: column;
    align-items: stretch;
  }

  .pm-item-project-card-actions {
    width: 100%;
    margin-left: 0;
    justify-content: space-between;
  }

  .pm-item-project-select {
    width: 100%;
    min-width: 0;
  }

  .pm-item-dialog-core-grid {
    grid-template-columns: 1fr;
    gap: 0;
  }

  .detail-timeline-grid {
    grid-template-columns: 1fr;
  }

  .pm-sidebar {
    width: 224px;
    min-width: 224px;
  }

  .pm-siyuan-page-actions {
    justify-content: flex-start;
  }
}
</style>
