<template>
  <el-drawer
    :model-value="siyuan.drawerVisible.value"
    direction="rtl"
    size="420px"
    wrapper-class="pm-siyuan-drawer"
    destroy-on-close
    :with-header="true"
    @close="siyuan.drawerVisible.value = false"
  >
    <template #header>
      思源配置
    </template>
    <div class="siyuan-drawer-body">
      <el-form label-position="top" size="default">
        <el-form-item label="服务地址">
          <el-input
            v-model="siyuan.form.baseUrl"
            placeholder="http://127.0.0.1:6806"
            clearable
          />
        </el-form-item>
        <el-form-item label="API Token">
          <el-input
            :type="siyuan.showToken.value ? 'text' : 'password'"
            v-model="siyuan.form.token"
            placeholder="填写思源 API Token"
            clearable
          >
            <template #suffix>
              <el-button type="text" size="small" @click="siyuan.showToken.value = !siyuan.showToken.value">
                {{ siyuan.showToken.value ? "隐藏" : "显示" }}
              </el-button>
            </template>
          </el-input>
        </el-form-item>
      </el-form>
      <div class="pm-siyuan-config-card">
        <div class="pm-siyuan-link-title">任务默认存储位置</div>
        <div class="pm-siyuan-config-summary">
          {{ formatPmSiyuanLocationLabel(siyuan.globalSiyuanLocationDraft.value) }}
        </div>
        <div class="pm-siyuan-inline-actions">
          <el-button size="small" @click="siyuan.openLocationPicker('global')">选择位置</el-button>
          <el-button size="small" @click="siyuan.globalSiyuanLocationDraft.value = null">清空</el-button>
        </div>
      </div>
      <div class="siyuan-actions">
        <el-button type="primary" @click="siyuan.saveConfig()">保存配置</el-button>
        <el-button type="info" :loading="siyuan.testing.value" @click="siyuan.testConnection()">测试连接</el-button>
        <el-button type="default" :loading="siyuan.loadingDirectory.value" @click="siyuan.loadDirectory()">加载目录</el-button>
      </div>
      <div class="siyuan-status">
        <el-tag v-if="siyuan.testingVersion.value" type="success" effect="dark">
          已连接 · {{ siyuan.testingVersion.value }}
        </el-tag>
        <el-alert
          v-if="siyuan.error.value"
          :title="siyuan.errorTitle.value"
          :description="siyuan.error.value"
          type="error"
          show-icon
          class="siyuan-error-alert"
        />
        <div v-else-if="siyuan.directoryFetchedAt.value" class="siyuan-fetch-hint">
          最后一次加载：{{ siyuan.directoryFetchedAt.value }}
        </div>
      </div>
      <div class="siyuan-tree-section">
        <div v-if="siyuan.loadingDirectory.value" class="siyuan-loading-hint">
          正在从思源加载目录...
        </div>
        <el-empty
          v-if="!siyuan.directory.value.length && !siyuan.loadingDirectory.value"
          description="暂无目录记录，先点击加载目录"
        />
        <el-tree
          v-else-if="siyuan.directory.value.length"
          :data="siyuan.directory.value"
          :props="siyuan.treeProps.value"
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
    v-model="siyuan.locationDialogVisible.value"
    :title="siyuan.locationPickerTitle.value"
    width="720px"
    destroy-on-close
  >
    <div class="pm-siyuan-dialog-body pm-siyuan-dialog-body--picker">
      <div class="pm-siyuan-picker-intro">
        选择笔记本根目录或某个父文档后，项目内新建思源页面会默认存放到这里。
      </div>
      <el-input
        v-model="siyuan.locationPickerSearch.value"
        class="pm-siyuan-picker-search"
        placeholder="搜索笔记本、文档标题或路径"
        clearable
      />
      <div v-if="siyuan.loadingDirectory.value && !siyuan.directory.value.length" class="pm-siyuan-empty-hint">
        正在同步思源目录，请稍候…
      </div>
      <div v-else-if="!siyuan.directory.value.length" class="pm-siyuan-empty-hint">
        尚未加载思源目录，请先测试连接并加载目录。
      </div>
      <template v-else>
        <div class="pm-siyuan-picker-tree-head">
          <span>{{ siyuan.locationPickerStatusText.value }}</span>
          <div class="pm-siyuan-inline-actions">
            <span v-if="siyuan.directoryFetchedAt.value">最后同步：{{ siyuan.directoryFetchedAt.value }}</span>
            <span v-if="siyuan.locationPickerSearchKeyword.value">清空搜索后会恢复默认折叠。</span>
            <el-button
              size="small"
              text
              :loading="siyuan.loadingDirectory.value"
              @click="siyuan.refreshLocationPickerDirectory()"
            >
              刷新目录
            </el-button>
          </div>
        </div>
        <div class="pm-siyuan-picker-tree-shell">
          <el-empty
            v-if="siyuan.locationPickerTreeData.value.length === 0"
            description="未找到匹配位置，请换个关键词试试"
          />
          <el-tree
            v-else
            :key="siyuan.locationPickerTreeKey.value"
            :data="siyuan.locationPickerTreeData.value"
            :props="siyuan.treeProps.value"
            node-key="id"
            highlight-current
            :current-node-key="siyuan.locationPickerCurrentNodeKey.value"
            :default-expanded-keys="siyuan.locationPickerExpandedKeys.value"
            :expand-on-click-node="false"
            class="siyuan-tree pm-siyuan-picker-tree"
            @node-click="siyuan.handleLocationTreeNodeClick"
          >
            <template #default="{ data, node }">
              <div
                class="siyuan-tree-node siyuan-tree-node--interactive"
                :class="{
                  'is-selected': siyuan.isLocationPickerNodeSelected(data),
                  'is-disabled': siyuan.isLocationPickerNodeDisabled(data, node),
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
        :class="{ 'is-empty': !siyuan.locationPickerValue.value }"
      >
        <div class="pm-siyuan-picker-selection-label">当前选择</div>
        <template v-if="siyuan.locationPickerValue.value">
          <div class="pm-siyuan-picker-selection-title">
            {{ siyuan.locationPickerSelectionTarget.value }}
          </div>
          <div class="pm-siyuan-picker-selection-meta">
            {{ siyuan.locationPickerValue.value.notebookName }}
          </div>
          <div class="pm-siyuan-picker-selection-path">
            {{ siyuan.locationPickerSelectionPath.value }}
          </div>
        </template>
        <div v-else class="pm-siyuan-picker-selection-empty">
          暂未选择位置，点击上方笔记本根目录或父文档即可。
        </div>
      </div>
    </div>
    <template #footer>
      <el-button @click="siyuan.locationDialogVisible.value = false">取消</el-button>
      <el-button @click="siyuan.clearLocationPicker()">清空</el-button>
      <el-button type="primary" @click="siyuan.applyLocationPicker()">确定</el-button>
    </template>
  </el-dialog>

  <el-dialog
    v-model="siyuan.pageDialogVisible.value"
    :title="siyuan.pageDialogTitle.value"
    width="760px"
  >
    <div class="pm-siyuan-dialog-body">
      <div class="pm-siyuan-search-toolbar">
        <el-input
          v-model="siyuan.pageFilterKeyword.value"
          :placeholder="siyuan.pageDialogInputPlaceholder.value"
          clearable
        />
        <el-button v-if="siyuan.pageShowReturnToLocation.value" @click="siyuan.restoreLocationResults()">
          返回当前位置列表
        </el-button>
        <el-button :loading="siyuan.pageSearchingAll.value" @click="siyuan.expandPagesToAll()">扩展到全库</el-button>
        <el-button
          type="success"
          :loading="siyuan.pageCreating.value"
          :disabled="!siyuan.pageCanCreateImmediately.value"
          @click="siyuan.createPageForItem()"
        >
          立即新建
        </el-button>
      </div>

      <div class="pm-siyuan-dialog-hint">{{ siyuan.pageCurrentRangeText.value }}</div>
      <div v-if="siyuan.pageFilterSummary.value" class="pm-siyuan-dialog-hint">{{ siyuan.pageFilterSummary.value }}</div>
      <div class="pm-siyuan-dialog-hint">
        当前创建位置：{{ formatPmSiyuanLocationLabel(siyuan.itemEffectiveLocation.value) }}
      </div>
      <div class="pm-siyuan-dialog-hint">
        当前创建标题：{{ siyuan.pageCreateTitle.value || "请先填写工作项标题或输入想创建的页面标题" }}
      </div>

      <div
        v-if="
          siyuan.pageLocationRefreshError.value &&
          siyuan.pageResultSource.value === 'location' &&
          siyuan.pageLocationState.value !== 'load-error'
        "
        class="pm-siyuan-dialog-notice pm-siyuan-dialog-notice--warning"
      >
        当前位置列表刷新失败，暂展示上一次结果。{{ siyuan.pageLocationRefreshError.value }}
      </div>

      <div v-if="siyuan.pageShowLocationLoading.value" class="pm-siyuan-empty-hint">
        正在加载当前位置文档列表...
      </div>
      <div v-else-if="siyuan.pageShowAllLoading.value" class="pm-siyuan-empty-hint">
        正在搜索全库...
      </div>
      <div v-else-if="siyuan.pageEmptyMessage.value" class="pm-siyuan-empty-hint">
        {{ siyuan.pageEmptyMessage.value }}
      </div>
      <div v-else class="pm-siyuan-page-list">
        <div v-for="page in siyuan.pageDisplayedResults.value" :key="page.docId" class="pm-siyuan-page-row">
          <div class="pm-siyuan-page-main">
            <div class="pm-siyuan-page-title">{{ page.docTitle }}</div>
            <div class="pm-siyuan-page-meta">{{ page.notebookName }} · {{ page.docHpath }}</div>
          </div>
          <div class="pm-siyuan-inline-actions">
            <el-button size="small" type="primary" link @click="siyuan.selectPageResult(page)">
              {{ siyuan.pageDialogMode.value === 'primary' ? '设为主页面' : '添加' }}
            </el-button>
            <el-button size="small" link @click="siyuan.openSiyuanPage(page)">打开</el-button>
          </div>
        </div>
      </div>
    </div>
    <template #footer>
      <el-button @click="siyuan.pageDialogVisible.value = false">关闭</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { inject } from "vue";
import { formatPmSiyuanLocationLabel, isPmSiyuanNotebookDirectory } from "../utils/pmSiyuan";
import { PM_SIYUAN_KEY } from "../composables/pmSiyuanKey";

const siyuan = inject(PM_SIYUAN_KEY)!;
</script>

<style>
/* SiYuan drawer body */
.pm-siyuan-drawer .el-drawer__body {
  padding: 16px;
}

/* SiYuan config & link cards */
.pm-siyuan-config-card,
.pm-siyuan-link-card {
  border-radius: 14px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(244, 247, 251, 0.82));
  border: 1px solid rgba(219, 229, 241, 0.88);
}

.pm-siyuan-link-card {
  width: 100%;
  gap: 10px;
}

.pm-siyuan-config-summary,
.pm-siyuan-link-subtitle,
.pm-siyuan-dialog-hint,
.pm-siyuan-dialog-notice,
.pm-siyuan-picker-intro {
  font-size: 13px;
  color: var(--pm-text-muted);
  line-height: 1.55;
}

.pm-siyuan-dialog-notice {
  border-radius: 8px;
  padding: 10px 12px;
}

.pm-siyuan-dialog-notice--warning {
  background: rgba(230, 162, 60, 0.08);
  border: 1px solid rgba(230, 162, 60, 0.18);
}

.pm-siyuan-inline-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.pm-siyuan-link-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--pm-text-main);
}

.pm-siyuan-link-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.pm-siyuan-link-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.pm-siyuan-link-subtitle {
  margin-top: -2px;
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
  gap: 10px;
  padding: 12px;
  border-radius: 10px;
  background: rgba(244, 247, 251, 0.76);
  border: 1px solid rgba(219, 229, 241, 0.88);
}

.pm-siyuan-page-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.pm-siyuan-page-row-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.pm-siyuan-page-title,
.detail-siyuan-page-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--pm-text-main);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-siyuan-page-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail-siyuan-page-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-siyuan-page-meta,
.detail-siyuan-page-meta {
  font-size: 12px;
  color: var(--pm-text-muted);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-siyuan-page-meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail-siyuan-page-meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-siyuan-page-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.pm-siyuan-more-trigger {
  padding: 4px 8px;
}

.pm-siyuan-empty-inline {
  font-size: 13px;
  color: var(--pm-text-muted);
}

.pm-siyuan-empty-hint {
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px dashed rgba(219, 229, 241, 0.9);
  font-size: 13px;
  color: var(--pm-text-muted);
  text-align: center;
}

/* SiYuan drawer internals */
.siyuan-drawer-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.siyuan-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.siyuan-status {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.siyuan-fetch-hint {
  font-size: 13px;
  color: var(--pm-text-muted);
}

.siyuan-loading-hint {
  font-size: 13px;
  color: var(--pm-text-muted);
}

.siyuan-tree-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.siyuan-tree {
  max-height: 360px;
  overflow-y: auto;
}

.siyuan-tree-node {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
}

.siyuan-tree-node--preview {
  cursor: default;
}

.siyuan-tree-node--interactive {
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 6px;
  transition: background 0.15s;
}

.siyuan-tree-node--interactive.is-selected {
  background: rgba(14, 165, 233, 0.08);
}

.siyuan-tree-node--interactive.is-disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.siyuan-node-main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.siyuan-node-title {
  font-size: 14px;
  color: var(--pm-text-main);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.siyuan-node-badge {
  font-size: 12px;
  color: var(--pm-text-muted);
  flex-shrink: 0;
}

.siyuan-node-badge.is-disabled {
  opacity: 0.6;
}

/* SiYuan picker dialogs */
.pm-siyuan-dialog-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.pm-siyuan-search-toolbar {
  display: flex;
  gap: 10px;
  align-items: center;
}

.pm-siyuan-search-toolbar .el-input {
  flex: 1;
}

.pm-siyuan-dialog-body--picker {
  gap: 16px;
}

.pm-siyuan-picker-selection {
  border: 1px solid rgba(219, 229, 241, 0.9);
  border-radius: 12px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  background: rgba(244, 247, 251, 0.6);
}

.pm-siyuan-picker-selection.is-empty {
  background: rgba(244, 247, 251, 0.3);
}

.pm-siyuan-picker-selection-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--pm-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.pm-siyuan-picker-selection-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--pm-text-main);
}

.pm-siyuan-picker-selection-meta {
  font-size: 13px;
  color: var(--pm-text-muted);
}

.pm-siyuan-picker-selection-path {
  font-size: 12px;
  color: var(--pm-text-muted);
}

.pm-siyuan-picker-selection-empty {
  font-size: 13px;
  color: var(--pm-text-muted);
}

.pm-siyuan-picker-tree-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
}

.pm-siyuan-picker-tree-shell {
  border-radius: 16px;
  border: 1px solid rgba(219, 229, 241, 0.88);
  padding: 10px;
  max-height: 320px;
  overflow-y: auto;
}

.siyuan-error-alert {
  padding: 8px 10px;
}

/* Tree depth overrides */
:deep(.siyuan-tree .el-tree-node__content) {
  height: 34px;
  border-radius: 6px;
}

:deep(.siyuan-tree .el-tree-node__content:hover) {
  background: rgba(14, 165, 233, 0.04);
}

:deep(.pm-siyuan-picker-tree .el-tree-node.is-current > .el-tree-node__content) {
  background: rgba(14, 165, 233, 0.08);
}

:deep(.siyuan-tree .el-tree-node__expand-icon) {
  color: var(--pm-text-muted);
}

:deep(.siyuan-tree .el-tree-node__expand-icon.expanded) {
  color: var(--pm-text-muted);
}

:deep(.siyuan-tree .el-tree-node__label) {
  width: 100%;
}

@media (max-width: 900px) {
  .pm-siyuan-link-header,
  .pm-siyuan-page-row,
  .detail-siyuan-page {
    flex-direction: column;
  }

  .pm-siyuan-page-actions {
    align-self: flex-end;
  }
}
</style>
