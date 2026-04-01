<template>
  <el-config-provider :locale="zhCn">
  <div class="pm-panel">
    <div class="pm-layout">
      <!-- Left: Project list -->
      <aside class="pm-sidebar">
        <div class="sidebar-header">
          <span class="sidebar-title">项目</span>
          <el-button type="primary" link @click="showCreateProject">
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
            <el-radio-group v-model="viewMode" size="default">
              <el-radio-button value="kanban">看板</el-radio-button>
              <el-radio-button value="gantt">甘特图</el-radio-button>
            </el-radio-group>
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
            <el-button type="primary" @click="showCreateItem">新建工作项</el-button>
            <el-button type="default" @click="openSiyuanDrawer">思源设置</el-button>
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
                @click="onCardClick(item)"
                @dblclick="onCardDblclick(item)"
                @contextmenu.prevent="onItemContext($event, item)"
              >
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
          :selected-item-id="selectedItemId"
          :show-project-meta="isOverview"
          @select="selectItem"
          @edit="editItem"
          @item-context="onGanttItemContext"
          @date-change="onGanttDateChange"
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
              <span class="detail-title">详情</span>
              <el-button size="small" link @click="selectedItemId = null">
                <el-icon><Close /></el-icon>
              </el-button>
            </div>
            <div class="detail-form">
              <div class="detail-field">
                <span class="detail-label">所属项目</span>
                <span class="detail-value">{{ activeProjects.find(p => p.id === selectedItem.projectId)?.name ?? '-' }}</span>
              </div>
              <div class="detail-field">
                <span class="detail-label">标题</span>
                <span class="detail-value">{{ selectedItem.title }}</span>
              </div>
              <div class="detail-field">
                <span class="detail-label">类型</span>
                <span class="detail-value">{{ PM_ITEM_TYPE_MAP[selectedItem.itemType]?.label ?? selectedItem.itemType }}</span>
              </div>
              <div class="detail-field">
                <span class="detail-label">优先级</span>
                <span class="detail-value">
                  <span class="priority-dot" :style="{ backgroundColor: PM_PRIORITY_MAP[selectedItem.priority]?.color }" />
                  {{ PM_PRIORITY_MAP[selectedItem.priority]?.label ?? selectedItem.priority }}
                </span>
              </div>
              <div class="detail-field">
                <span class="detail-label">状态</span>
                <span class="detail-value">{{ PM_STATUS_COLUMNS.find(c => c.key === selectedItem.status)?.label ?? selectedItem.status }}</span>
              </div>
              <div class="detail-field">
                <span class="detail-label">开始日期</span>
                <span class="detail-value">{{ selectedItem.startAt ?? '-' }}</span>
              </div>
              <div class="detail-field">
                <span class="detail-label">截止日期</span>
                <span class="detail-value" :class="{ 'is-overdue-date': isOverdue(selectedItem) }">{{ selectedItem.endAt ?? '-' }}</span>
              </div>
              <div class="detail-field">
                <span class="detail-label">标签</span>
                <span class="detail-value detail-tags">
                  <el-tag v-for="tag in selectedItem.tags" :key="tag" size="small" type="info">{{ tag }}</el-tag>
                  <span v-if="selectedItem.tags.length === 0">-</span>
                </span>
              </div>
              <div class="detail-field">
                <span class="detail-label">思源主页面</span>
                <div class="detail-value detail-siyuan-pages">
                  <template v-if="selectedItem.siyuanPrimaryPage">
                    <div class="detail-siyuan-page">
                      <div class="detail-siyuan-page-main">
                        <span class="detail-siyuan-page-title">{{ selectedItem.siyuanPrimaryPage.docTitle }}</span>
                        <span class="detail-siyuan-page-meta">
                          {{ selectedItem.siyuanPrimaryPage.notebookName }} · {{ selectedItem.siyuanPrimaryPage.docHpath }}
                        </span>
                      </div>
                      <el-button size="small" link @click="openSiyuanPage(selectedItem.siyuanPrimaryPage)">打开</el-button>
                    </div>
                  </template>
                  <span v-else>-</span>
                </div>
              </div>
              <div class="detail-field" v-if="selectedItem.siyuanExtraPages.length > 0">
                <span class="detail-label">附加页面</span>
                <div class="detail-value detail-siyuan-pages">
                  <div v-for="page in selectedItem.siyuanExtraPages" :key="page.docId" class="detail-siyuan-page">
                    <div class="detail-siyuan-page-main">
                      <span class="detail-siyuan-page-title">{{ page.docTitle }}</span>
                      <span class="detail-siyuan-page-meta">{{ page.notebookName }} · {{ page.docHpath }}</span>
                    </div>
                    <el-button size="small" link @click="openSiyuanPage(page)">打开</el-button>
                  </div>
                </div>
              </div>
              <div class="detail-field">
                <span class="detail-label">描述</span>
                <pre class="detail-value detail-description">{{ selectedItem.description || '-' }}</pre>
              </div>
              <div v-if="selectedItem.completedAt" class="detail-field">
                <span class="detail-label">完成时间</span>
                <span class="detail-value">{{ formatDateTime(selectedItem.completedAt) }}</span>
              </div>
              <div class="detail-field">
                <span class="detail-label">创建时间</span>
                <span class="detail-value">{{ formatDateTime(selectedItem.createdAt) }}</span>
              </div>

              <div class="detail-actions">
                <el-button size="small" @click="togglePin">{{ selectedItem.pinned ? '取消置顶' : '置顶' }}</el-button>
                <el-button v-if="selectedItem.status !== 'done'" size="small" type="success" @click="advanceStatus">
                  推进状态
                </el-button>
                <el-button size="small" type="danger" @click="deleteItem">删除</el-button>
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
    <el-dialog v-model="itemDialogVisible" :title="editingItem ? '编辑工作项' : '新建工作项'" width="720px" @close="resetItemForm">
      <el-form :model="itemForm" label-width="80px" size="default">
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
            <template #prefix>
              <span class="priority-dot" :style="{ backgroundColor: PM_PRIORITY_MAP[itemForm.priority]?.color }" />
            </template>
            <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key">
              <span class="priority-dot" :style="{ backgroundColor: meta.color }" />
              {{ meta.label }}
            </el-option>
          </el-select>
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="itemForm.status">
            <el-option v-for="col in PM_STATUS_COLUMNS" :key="col.key" :label="col.label" :value="col.key" />
          </el-select>
        </el-form-item>
        <el-form-item label="开始日期">
          <el-date-picker v-model="itemForm.startAt" type="date" value-format="YYYY-MM-DD" clearable style="width:100%" :disabled-date="(d) => !!itemForm.endAt && d > new Date(itemForm.endAt + 'T23:59:59')" />
        </el-form-item>
        <el-form-item label="截止日期">
          <el-date-picker v-model="itemForm.endAt" type="date" value-format="YYYY-MM-DD" clearable style="width:100%" :disabled-date="(d) => !!itemForm.startAt && d < new Date(itemForm.startAt + 'T00:00:00')" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="itemForm.description" type="textarea" :rows="3" />
        </el-form-item>
        <el-form-item label="思源关联" class="pm-form-item-top">
          <div class="pm-siyuan-link-card">
            <div class="pm-siyuan-link-header">
              <div>
                <div class="pm-siyuan-link-title">页面关联</div>
                <div class="pm-siyuan-link-subtitle">
                  当前有效位置：{{ formatPmSiyuanLocationLabel(itemEffectiveLocation) }}
                  <el-tag size="small" type="info">{{ itemEffectiveLocationSource }}</el-tag>
                </div>
              </div>
              <div class="pm-siyuan-inline-actions">
                <el-button size="small" @click="openSiyuanPageDialog('primary')">
                  {{ itemPrimaryPage ? "更换主页面" : "关联主页面" }}
                </el-button>
                <el-button size="small" @click="openSiyuanPageDialog('extra')">添加附加页面</el-button>
              </div>
            </div>

            <div class="pm-siyuan-linked-section">
              <div class="pm-siyuan-section-label">主页面</div>
              <div v-if="itemPrimaryPage" class="pm-siyuan-page-row">
                <div class="pm-siyuan-page-main">
                  <div class="pm-siyuan-page-title">{{ itemPrimaryPage.docTitle }}</div>
                  <div class="pm-siyuan-page-meta">
                    {{ itemPrimaryPage.notebookName }} · {{ itemPrimaryPage.docHpath }}
                  </div>
                </div>
                <div class="pm-siyuan-inline-actions">
                  <el-button size="small" link @click="openSiyuanPage(itemPrimaryPage)">打开</el-button>
                  <el-button size="small" link @click="removeItemLinkedPage(itemPrimaryPage.docId)">移除</el-button>
                </div>
              </div>
              <div v-else class="pm-siyuan-empty-hint">暂未关联主页面</div>
            </div>

            <div class="pm-siyuan-linked-section">
              <div class="pm-siyuan-section-label">附加页面</div>
              <div v-if="itemExtraPages.length > 0" class="pm-siyuan-page-list">
                <div v-for="page in itemExtraPages" :key="page.docId" class="pm-siyuan-page-row">
                  <div class="pm-siyuan-page-main">
                    <div class="pm-siyuan-page-title">{{ page.docTitle }}</div>
                    <div class="pm-siyuan-page-meta">{{ page.notebookName }} · {{ page.docHpath }}</div>
                  </div>
                  <div class="pm-siyuan-inline-actions">
                    <el-button size="small" link @click="applyItemPrimaryPage(page)">设为主页面</el-button>
                    <el-button size="small" link @click="openSiyuanPage(page)">打开</el-button>
                    <el-button size="small" link @click="removeItemLinkedPage(page.docId)">移除</el-button>
                  </div>
                </div>
              </div>
              <div v-else class="pm-siyuan-empty-hint">暂无附加页面</div>
            </div>
          </div>
        </el-form-item>
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
      :title="siyuanPageDialogMode === 'primary' ? '关联思源主页面' : '添加思源附加页面'"
      width="760px"
    >
      <div class="pm-siyuan-dialog-body">
        <div class="pm-siyuan-search-toolbar">
          <el-input
            v-model="siyuanPageSearchKeyword"
            placeholder="按页面标题或路径搜索"
            clearable
            @keyup.enter="searchSiyuanPages()"
          />
          <el-button type="primary" :loading="siyuanPageSearching" @click="searchSiyuanPages()">搜索</el-button>
          <el-button :loading="siyuanPageSearching" @click="searchSiyuanPages(true)">扩展到全库</el-button>
          <el-button
            type="success"
            :loading="siyuanPageCreating"
            :disabled="!itemEffectiveLocation || !itemForm.title.trim()"
            @click="createSiyuanPageForItem"
          >
            立即新建
          </el-button>
        </div>

        <div class="pm-siyuan-dialog-hint">
          当前搜索范围：
          {{ siyuanPageSearchResultScope === 'all' ? '全部已打开笔记本' : formatPmSiyuanLocationLabel(itemEffectiveLocation) }}
        </div>
        <div class="pm-siyuan-dialog-hint">
          当前创建位置：{{ formatPmSiyuanLocationLabel(itemEffectiveLocation) }}
        </div>

        <div v-if="!itemEffectiveLocation" class="pm-siyuan-empty-hint">
          当前没有可用的默认位置，因此只能搜索已有页面，不能直接新建页面。
        </div>

        <el-empty
          v-if="!siyuanPageSearchResults.length && !siyuanPageSearching"
          description="暂无搜索结果"
        />
        <div v-else class="pm-siyuan-page-list">
          <div v-for="page in siyuanPageSearchResults" :key="page.docId" class="pm-siyuan-page-row">
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
  filterPmSiyuanDirectory,
  formatPmSiyuanLocationLabel,
  formatPmSiyuanLocationPathLabel,
  formatPmSiyuanLocationTargetLabel,
  isPmSiyuanNotebookDirectory,
  removePmSiyuanPage,
  resolvePmSiyuanEffectiveLocation,
  setPmSiyuanPrimaryPage,
} from "../utils/pmSiyuan";

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
  description: "",
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
let siyuanDirectoryLoadPromise: Promise<void> | null = null;
const siyuanError = ref("");
const siyuanErrorContext = ref<"test" | "directory" | null>(null);
const siyuanTreeProps = { label: "name", children: "children" };
const siyuanLocationDialogVisible = ref(false);
const siyuanLocationPickerTarget = ref<"global" | "project">("global");
const siyuanLocationPickerValue = ref<PmSiyuanLocation | null>(null);
const siyuanLocationPickerSearch = ref("");
const siyuanPageDialogVisible = ref(false);
const siyuanPageDialogMode = ref<"primary" | "extra">("primary");
const siyuanPageSearchKeyword = ref("");
const siyuanPageSearchScope = ref<"location" | "all">("location");
const siyuanPageSearchResults = ref<PmSiyuanPageRef[]>([]);
const siyuanPageSearchResultScope = ref<"location" | "all">("location");
const siyuanPageSearching = ref(false);
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
const archivedProjects = computed(() => projects.value.filter((p) => p.status === "archived"));
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
const itemDialogProjectId = computed<number | null>(() => {
  if (editingItem.value) {
    return editingItem.value.projectId;
  }
  if (isOverview.value) {
    return itemFormProjectId.value;
  }
  return typeof selectedProjectId.value === "number" ? selectedProjectId.value : null;
});
const itemDialogProject = computed(() =>
  projects.value.find((project) => project.id === itemDialogProjectId.value) ?? null,
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

function cloneSiyuanLocation(location: PmSiyuanLocation | null | undefined): PmSiyuanLocation | null {
  return location ? { ...location } : null;
}

function cloneSiyuanPage(page: PmSiyuanPageRef | null | undefined): PmSiyuanPageRef | null {
  return page ? { ...page } : null;
}

function cloneSiyuanPages(pages: PmSiyuanPageRef[] | null | undefined): PmSiyuanPageRef[] {
  return (pages ?? []).map((page) => ({ ...page }));
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

async function openSiyuanPageDialog(mode: "primary" | "extra") {
  siyuanPageDialogMode.value = mode;
  siyuanPageSearchScope.value = itemEffectiveLocation.value ? "location" : "all";
  siyuanPageSearchResultScope.value = siyuanPageSearchScope.value;
  siyuanPageSearchKeyword.value = itemForm.value.title.trim();
  siyuanPageSearchResults.value = [];
  siyuanPageDialogVisible.value = true;
  if (siyuanPageSearchKeyword.value.length >= 2) {
    await searchSiyuanPages();
  }
}

async function searchSiyuanPages(forceAll = false) {
  const keyword = siyuanPageSearchKeyword.value.trim();
  if (keyword.length < 2) {
    ElMessage.warning("请输入至少 2 个字符再搜索");
    return;
  }

  const searchAll = forceAll || siyuanPageSearchScope.value === "all" || !itemEffectiveLocation.value;
  try {
    siyuanPageSearching.value = true;
    const result = (await invoke<PmSiyuanSearchResult>("tool:pm:siyuan-search-pages", {
      keyword,
      searchAll,
      location: searchAll ? null : itemEffectiveLocation.value,
    })) ?? { items: [], scope: searchAll ? "all" : "location" };
    siyuanPageSearchResults.value = result.items ?? [];
    siyuanPageSearchResultScope.value = result.scope ?? (searchAll ? "all" : "location");
    siyuanPageSearchScope.value = result.scope ?? (searchAll ? "all" : "location");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    siyuanPageSearching.value = false;
  }
}

function selectSiyuanPageResult(page: PmSiyuanPageRef) {
  if (siyuanPageDialogMode.value === "primary") {
    applyItemPrimaryPage(page);
  } else {
    addItemExtraPage(page);
  }
  siyuanPageDialogVisible.value = false;
}

async function createSiyuanPageForItem() {
  const title = itemForm.value.title.trim();
  if (!title) {
    ElMessage.warning("请先填写工作项标题，再创建思源页面");
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
        startAt: itemForm.value.startAt,
        endAt: itemForm.value.endAt,
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

async function refreshSiyuanDirectory(options: { showSuccess?: boolean } = {}) {
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
    } catch (error) {
      siyuanError.value = (error as Error).message;
      siyuanErrorContext.value = "directory";
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

function onCardClick(item: PmItem) {
  if (clickTimer.value) return;
  clickTimer.value = setTimeout(() => {
    clickTimer.value = null;
    selectItem(item);
  }, 220);
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
    description: "",
  };
  itemPrimaryPage.value = null;
  itemExtraPages.value = [];
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
  itemPrimaryPage.value = cloneSiyuanPage(item.siyuanPrimaryPage);
  itemExtraPages.value = cloneSiyuanPages(item.siyuanExtraPages);
  itemDialogVisible.value = true;
}

function resetItemForm() {
  editingItem.value = null;
  itemPrimaryPage.value = null;
  itemExtraPages.value = [];
}

async function submitItem() {
  if (!itemForm.value.title.trim()) {
    ElMessage.warning("请输入标题");
    return;
  }
  try {
    const payload = {
      ...itemForm.value,
      siyuanPrimaryPage: itemPrimaryPage.value,
      siyuanExtraPages: itemExtraPages.value,
    };
    if (editingItem.value) {
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

// 过滤条件变化 → 延迟重建，跳过拖拽中
watch(
  [searchText, filterType, filterPriority],
  () => { nextTick(() => { if (!draggingItemId.value) initSortable(); }); },
  { flush: 'post' }
);

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
  nextTick(() => positionCtxMenu(anchorX, anchorY));
  setTimeout(() => {
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
.project-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  cursor: pointer;
  font-size: 15px;
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
  width: 12px;
  height: 12px;
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
  padding: 12px 12px 12px 12px;
  margin-bottom: 8px;
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
  font-size: 13px;
  border: 2px dashed var(--el-border-color-light);
  border-radius: 6px;
  pointer-events: none;
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
.pm-siyuan-picker-intro {
  font-size: 13px;
  line-height: 1.6;
  color: var(--el-text-color-secondary);
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

.pm-siyuan-link-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.pm-siyuan-link-subtitle {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 4px;
}

.pm-siyuan-linked-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pm-siyuan-section-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  letter-spacing: 0.04em;
}

.pm-siyuan-page-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pm-siyuan-page-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 10px;
  padding: 10px 12px;
  background: var(--el-bg-color);
}

.pm-siyuan-page-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.pm-siyuan-page-title,
.detail-siyuan-page-title {
  font-size: 14px;
  font-weight: 600;
  line-height: 1.4;
  color: var(--el-text-color-primary);
  word-break: break-word;
}

.pm-siyuan-page-meta,
.detail-siyuan-page-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.5;
  word-break: break-word;
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
  .pm-siyuan-page-row,
  .detail-siyuan-page {
    flex-direction: column;
  }

  .pm-siyuan-link-header {
    flex-direction: column;
  }
}
</style>
