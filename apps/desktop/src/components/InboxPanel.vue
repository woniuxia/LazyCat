<template>
  <div class="inbox-panel">
    <aside class="inbox-sidebar panel-card">
      <section class="sidebar-hero">
        <div class="sidebar-hero-eyebrow">Clipboard Inbox</div>
        <h2 class="sidebar-hero-title">收纳箱</h2>
      </section>

      <section class="sidebar-section">
        <div class="section-title">总览</div>
        <div class="sidebar-overview-grid">
          <div
            v-for="metric in overviewMetrics"
            :key="metric.label"
            class="sidebar-overview-card"
            :class="`is-${metric.tone}`"
          >
            <span>{{ metric.label }}</span>
            <strong>{{ metric.value }}</strong>
          </div>
        </div>
      </section>

      <section class="sidebar-section">
        <div class="section-title">分区</div>
        <button
          v-for="option in bucketOptions"
          :key="option.value"
          class="sidebar-filter"
          :class="{ 'is-active': filters.bucket === option.value }"
          @click="selectBucket(option.value)"
        >
          <div class="sidebar-filter-copy">
            <strong>{{ option.label }}</strong>
          </div>
          <span class="sidebar-filter-count">{{ bucketCount(option.value) }}</span>
        </button>
      </section>

      <section class="sidebar-section">
        <div class="section-title">类型</div>
        <button
          class="sidebar-filter"
          :class="{ 'is-active': filters.itemType === '' }"
          @click="selectType('')"
        >
          <div class="sidebar-filter-copy">
            <strong>全部类型</strong>
          </div>
          <span class="sidebar-filter-count">{{ totalTypeCount }}</span>
        </button>
        <button
          v-for="option in typeOptions"
          :key="option.value"
          class="sidebar-filter"
          :class="{ 'is-active': filters.itemType === option.value }"
          @click="selectType(option.value)"
        >
          <div class="sidebar-filter-copy">
            <strong>{{ option.label }}</strong>
          </div>
          <span class="sidebar-filter-count">{{ typeCount(option.value) }}</span>
        </button>
      </section>

      <section class="sidebar-section">
        <div class="section-title">筛选</div>
        <button
          class="sidebar-filter"
          :class="{ 'is-active': filters.starredOnly }"
          @click="toggleFlag('starredOnly')"
        >
          <div class="sidebar-filter-copy">
            <strong>仅星标</strong>
          </div>
          <span class="sidebar-filter-count">{{ facets.starred }}</span>
        </button>
        <button
          class="sidebar-filter"
          :class="{ 'is-active': filters.externalOnly }"
          @click="toggleFlag('externalOnly')"
        >
          <div class="sidebar-filter-copy">
            <strong>外部内容</strong>
          </div>
          <span class="sidebar-filter-count">{{ facets.external }}</span>
        </button>
        <button
          class="sidebar-filter"
          :class="{ 'is-active': filters.summaryOnly }"
          @click="toggleFlag('summaryOnly')"
        >
          <div class="sidebar-filter-copy">
            <strong>仅摘要</strong>
          </div>
          <span class="sidebar-filter-count">{{ facets.summaryOnly }}</span>
        </button>
      </section>

      <section class="sidebar-section">
        <div class="section-title section-title-row">
          <span>当前筛选</span>
          <button
            v-if="hasActiveFilters"
            class="section-link-btn"
            type="button"
            @click="resetFilters"
          >
            重置
          </button>
        </div>
        <div v-if="activeFilterChips.length > 0" class="filter-chip-list">
          <span v-for="chip in activeFilterChips" :key="chip.label" class="filter-chip">
            {{ chip.label }}
          </span>
        </div>
        <div v-else class="sidebar-empty-note">当前使用默认视图：历史流 + 全部类型</div>
      </section>

      <div class="sidebar-spacer" />

      <div class="sidebar-actions">
        <button class="sidebar-action-btn" @click="settingsDialogVisible = true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3" />
            <path
              d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65
              1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65
              0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65
              1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65
              0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65
              1.65 0 0 0 1.82.33h.01a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65
              0 0 0 1 1.51h.01a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65
              1.65 0 0 0-.33 1.82v.01a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65
              1.65 0 0 0-1.51 1z"
            />
          </svg>
          <span>收纳设置</span>
        </button>
      </div>
    </aside>

    <section class="inbox-list panel-card">
      <div class="list-toolbar">
        <el-input
          v-model.trim="filters.keyword"
          clearable
          placeholder="搜索标题、备注或正文"
          @keyup.enter="reloadList"
          @clear="reloadList"
        />
        <el-button @click="reloadList">搜索</el-button>
      </div>
      <div class="list-active-filters">
        <template v-if="activeFilterChips.length > 0">
          <span v-for="chip in activeFilterChips" :key="chip.label" class="filter-chip">
            {{ chip.label }}
          </span>
        </template>
        <span v-else class="filter-chip is-neutral">默认视图</span>
      </div>
      <div class="list-summary">
        <span>{{ listSummaryLabel }}</span>
        <span>{{ loadingList ? "正在刷新…" : selectedPositionLabel }}</span>
      </div>
      <div
        ref="listViewportRef"
        class="list-viewport"
        @scroll="onListScroll"
        @contextmenu="hideImageContextMenu"
      >
        <div :style="{ height: `${topSpacer}px` }" />
        <button
          v-for="item in virtualItems"
          :key="item.id"
          v-memo="[
            item.id,
            item.id === selectedId,
            item.title,
            item.preview,
            item.bucket,
            item.storageKind,
            item.starred,
            item.hasNote,
            item.lastSeenAt,
            item.seenCount,
          ]"
          class="list-row"
          :class="{ 'is-active': item.id === selectedId }"
          @click="selectItem(item.id)"
        >
          <div class="row-topline">
            <span class="row-topline-type">{{ itemTypeLabel(item.itemType) }}</span>
            <span class="row-topline-time">{{ formatDateTime(item.lastSeenAt) }}</span>
          </div>
          <div class="row-header">
            <strong>{{ item.title || "(未命名)" }}</strong>
            <div class="row-badges">
              <span v-if="item.bucket !== 'history'" class="badge promoted">
                {{ item.bucket === "archived" ? "已归档" : "已升格" }}
              </span>
              <span v-if="item.starred" class="badge star">星标</span>
              <span v-if="item.hasNote" class="badge note">有备注</span>
              <span class="badge">{{ itemTypeLabel(item.itemType) }}</span>
              <span class="badge" :class="storageBadgeClass(item)">
                {{ storageBadgeLabel(item) }}
              </span>
            </div>
          </div>
          <div class="row-preview">{{ item.preview || "暂无摘要" }}</div>
          <div class="row-footer">
            <div class="row-meta">
              <span>{{ bucketLabel(item.bucket) }}</span>
              <span>{{ formatByteSize(item.byteSize) }}</span>
              <span>{{ item.seenCount }} 次</span>
            </div>
            <div class="row-meta row-meta-secondary">
              <span>{{ storageKindLabel(item.storageKind) }}</span>
            </div>
          </div>
        </button>
        <div :style="{ height: `${bottomSpacer}px` }" />
      </div>
      <div class="list-footer">
        <el-button :disabled="loadingList || !hasMore" @click="loadMore">
          {{ hasMore ? "加载更多" : "没有更多了" }}
        </el-button>
      </div>
    </section>

    <section class="inbox-detail panel-card" @scroll="hideImageContextMenu">
      <template v-if="detail">
        <div class="detail-overview-card">
          <div class="detail-overview-topline">
            <div class="detail-overview-eyebrow">
              {{ bucketLabel(detail.bucket) }} · {{ itemTypeLabel(detail.itemType) }}
            </div>
            <button
              type="button"
              class="detail-overview-action"
              @click="detailEditExpanded = !detailEditExpanded"
            >
              {{ detailEditExpanded ? "收起编辑" : "编辑标题/备注" }}
            </button>
          </div>
          <h3>{{ detail.title || "(未命名)" }}</h3>
          <p class="detail-overview-preview">{{ detailPreviewText }}</p>
          <div class="detail-badge-row">
            <span v-if="metaDraft.starred" class="badge star">星标</span>
            <span class="badge">{{ itemTypeLabel(detail.itemType) }}</span>
            <span class="badge" :class="storageBadgeClass(detail)">
              {{ storageBadgeLabel(detail) }}
            </span>
            <span v-if="detail.note" class="badge note">已写备注</span>
            <span v-if="detail.bucket !== 'history'" class="badge promoted">
              {{ detail.bucket === "archived" ? "已归档" : "已收纳" }}
            </span>
          </div>
          <div class="detail-overview-metrics">
            <div class="detail-metric">
              <span>最近出现</span>
              <strong>{{ formatDateTime(detail.lastSeenAt) }}</strong>
            </div>
            <div class="detail-metric">
              <span>体积</span>
              <strong>{{ formatByteSize(detail.byteSize) }}</strong>
            </div>
            <div class="detail-metric">
              <span>出现次数</span>
              <strong>{{ detail.seenCount }} 次</strong>
            </div>
          </div>
          <div v-if="detail.canOpenPath && detail.openPath" class="detail-origin">
            <span>来源位置</span>
            <button
              type="button"
              class="detail-origin-link"
              @click="openPath(detail.openPath, true)"
            >
              {{ detail.openPath }}
            </button>
          </div>
        </div>

        <div class="detail-toolbar">
          <div class="detail-toolbar-main">
            <el-button type="primary" @click="transferToTodo">转任务清单</el-button>
            <el-button v-if="canTransferToVault" @click="transferToVault">存入密码库</el-button>
            <el-button :disabled="!copyableText" @click="copyDetailContent">复制内容</el-button>
            <el-button
              v-if="detail.canOpenPath && detail.openPath"
              @click="openPath(detail.openPath, true)"
            >
              打开位置
            </el-button>
          </div>
          <el-dropdown
            trigger="click"
            placement="bottom-end"
            class="detail-toolbar-more"
            @command="handleDetailActionCommand"
          >
            <el-button plain>更多操作</el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="toggle-star">
                  {{ metaDraft.starred ? "取消星标" : "设为星标" }}
                </el-dropdown-item>
                <el-dropdown-item command="promote" :disabled="!canPromoteCurrentItem">
                  {{ promoteButtonLabel }}
                </el-dropdown-item>
                <el-dropdown-item command="archive">
                  {{ archiveButtonLabel }}
                </el-dropdown-item>
                <el-dropdown-item command="note" disabled>转便签（后续支持）</el-dropdown-item>
                <el-dropdown-item command="delete" divided>删除</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>

        <div v-if="shouldShowDetailBody" class="detail-section detail-body-section">
          <div class="section-title">正文</div>
          <div v-if="detail.itemType === 'image' && detail.payloadDataUrl" class="image-box">
            <img
              :src="detail.payloadDataUrl"
              alt="clipboard image"
              class="detail-image"
              @click="openImagePreview"
              @contextmenu.stop.prevent="onImageContextMenu"
            />
            <div class="image-box-hint">点击查看大图，右键打开快捷操作</div>
          </div>
          <pre v-else-if="detailText" class="detail-text">{{ detailText }}</pre>
        </div>

        <div v-if="detailEditExpanded" class="detail-section">
          <div class="section-title-row">
            <div class="section-title">编辑</div>
            <button
              type="button"
              class="section-link-btn"
              @click="detailEditExpanded = false"
            >
              收起
            </button>
          </div>
          <div class="detail-edit-card">
            <el-form label-position="top">
              <el-form-item label="标题">
                <el-input v-model.trim="metaDraft.title" />
              </el-form-item>
              <el-form-item label="备注">
                <el-input v-model="metaDraft.note" type="textarea" :rows="4" />
              </el-form-item>
              <el-button type="primary" :loading="savingMeta" @click="saveMeta">保存</el-button>
            </el-form>
          </div>
        </div>

        <el-collapse
          v-if="detailMetaEntries.length > 0 || detail.fileRefs.length > 0"
          v-model="detailExpandedSections"
          class="detail-collapse"
        >
          <el-collapse-item v-if="detailMetaEntries.length > 0" name="meta">
            <template #title>
              <div class="detail-collapse-title">
                <span class="section-title">元数据</span>
                <span class="detail-collapse-count">{{ detailMetaEntries.length }} 项</span>
              </div>
            </template>
            <div class="detail-meta-list">
              <div v-for="entry in detailMetaEntries" :key="entry.label" class="detail-meta-item">
                <span>{{ entry.label }}</span>
                <strong>{{ entry.value }}</strong>
              </div>
            </div>
          </el-collapse-item>

          <el-collapse-item v-if="detail.fileRefs.length > 0" name="files">
            <template #title>
              <div class="detail-collapse-title">
                <span class="section-title">文件引用</span>
                <span class="detail-collapse-count">{{ detail.fileRefs.length }} 个</span>
              </div>
            </template>
            <div v-for="fileRef in detail.fileRefs" :key="fileRef.filePath" class="file-ref">
              <div class="file-ref-main">
                <strong>{{ fileRef.fileName }}</strong>
                <div class="file-ref-path">{{ fileRef.filePath }}</div>
                <div class="file-ref-submeta">
                  <span>{{ fileRef.fileSize ? formatByteSize(fileRef.fileSize) : "未知大小" }}</span>
                  <span v-if="fileRef.modifiedAt"
                    >修改于 {{ formatDateTime(fileRef.modifiedAt) }}</span
                  >
                </div>
              </div>
              <el-button size="small" @click="openPath(fileRef.filePath, true)">打开位置</el-button>
            </div>
          </el-collapse-item>
        </el-collapse>

      </template>

      <div v-else class="detail-placeholder">
        <div class="detail-placeholder-card">
          <div class="detail-placeholder-eyebrow">Ready to Sort</div>
          <h3>选择一条收纳记录</h3>
          <p>左侧切视图，中间扫摘要，右侧立即整理、补备注或转入其他工具。</p>
        </div>
      </div>
    </section>

    <el-dialog
      v-model="settingsDialogVisible"
      title="收纳设置"
      width="480px"
      :append-to-body="true"
    >
      <div class="setting-card">
        <div class="setting-row">
          <div class="setting-copy">
            <strong>后台采集</strong>
            <span>{{
              captureStatus.paused ? "已暂停，可手动恢复" : "持续监听新的剪贴板内容"
            }}</span>
          </div>
          <el-switch :model-value="captureStatus.captureEnabled" @change="onToggleCapture" />
        </div>
        <div class="setting-meta">
          <span>{{ captureStatus.paused ? "当前已暂停" : "当前运行中" }}</span>
          <span>保留 {{ captureStatus.historyRetentionDays }} 天</span>
        </div>
        <div class="capture-actions">
          <el-button size="small" @click="onTogglePause">
            {{ captureStatus.paused ? "立即恢复" : "暂停 5 分钟" }}
          </el-button>
          <el-button size="small" @click="runCleanup">清理</el-button>
        </div>
      </div>
    </el-dialog>

    <Teleport to="body">
      <div
        v-if="imagePreviewVisible"
        class="image-preview-overlay"
        @click="closeImagePreview"
        @contextmenu.self.prevent="hideImageContextMenu"
      >
        <div class="image-preview-panel" @click.stop>
          <div class="image-preview-header">
            <div>
              <strong>{{ detail?.title || "图片预览" }}</strong>
              <span>{{ imagePreviewSubtitle }}</span>
            </div>
            <button class="image-preview-close" type="button" @click="closeImagePreview">
              关闭
            </button>
          </div>
          <img
            v-if="currentImageDataUrl"
            :src="currentImageDataUrl"
            alt="preview image"
            class="image-preview-media"
            @click.stop
            @contextmenu.stop.prevent="onImageContextMenu"
          />
        </div>
      </div>
    </Teleport>

    <Teleport to="body">
      <div
        v-if="imageContextMenu.visible"
        ref="imageContextMenuRef"
        class="image-context-menu"
        :style="{ left: `${imageContextMenu.x}px`, top: `${imageContextMenu.y}px` }"
      >
        <button
          type="button"
          class="image-context-menu-item"
          :disabled="!canOperateCurrentImage"
          :class="{ 'is-disabled': !canOperateCurrentImage }"
          @click="copyCurrentImage"
        >
          复制图像
        </button>
        <button
          type="button"
          class="image-context-menu-item"
          :disabled="!canOperateCurrentImage"
          :class="{ 'is-disabled': !canOperateCurrentImage }"
          @click="openCurrentImage"
        >
          打开图像
        </button>
        <button
          type="button"
          class="image-context-menu-item"
          :disabled="!canOperateCurrentImage"
          :class="{ 'is-disabled': !canOperateCurrentImage }"
          @click="revealCurrentImage"
        >
          打开图像位置
        </button>
        <button
          type="button"
          class="image-context-menu-item"
          :disabled="!canOperateCurrentImage"
          :class="{ 'is-disabled': !canOperateCurrentImage }"
          @click="copyCurrentImagePath"
        >
          复制图像路径
        </button>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel, suppressClipboardCapture } from "../bridge/tauri";
import { useClipboardSuggestion } from "../composables/useClipboardSuggestion";
import { useTabs } from "../composables/useTabs";
import type {
  InboxCaptureStatus,
  InboxFacetCounts,
  InboxItemDetail,
  InboxItemSummary,
  InboxItemType,
  InboxListQuery,
  InboxListResult,
  InboxStorageKind,
} from "../types";

const PAGE_SIZE = 50;
const ROW_HEIGHT = 154;
const OVERSCAN = 8;

type BucketFilter = NonNullable<InboxListQuery["bucket"]> | "all";
type DetailCollapseSection = "meta" | "files";

const bucketOptions: Array<{ value: BucketFilter; label: string }> = [
  { value: "history", label: "历史流" },
  { value: "inbox", label: "收纳箱" },
  { value: "archived", label: "已归档" },
  { value: "all", label: "全部" },
];

const typeOptions: Array<{ value: InboxItemType; label: string }> = [
  { value: "text", label: "文本" },
  { value: "html", label: "HTML" },
  { value: "rtf", label: "RTF" },
  { value: "image", label: "图片" },
  { value: "file", label: "文件" },
  { value: "unknown", label: "未知" },
];

const { openTab } = useTabs();
const { setPendingToolInput } = useClipboardSuggestion();

const filters = reactive({
  bucket: "history" as BucketFilter,
  itemType: "" as InboxItemType | "",
  starredOnly: false,
  externalOnly: false,
  summaryOnly: false,
  keyword: "",
});

const captureStatus = ref<InboxCaptureStatus>({
  monitorRunning: true,
  consentAck: true,
  captureEnabled: true,
  captureWhenHidden: true,
  historyRetentionDays: 14,
  paused: false,
  pausedUntil: null,
});
const facets = ref<InboxFacetCounts>({
  buckets: { history: 0, inbox: 0, archived: 0 },
  types: {},
  starred: 0,
  external: 0,
  summaryOnly: 0,
});
const items = ref<InboxItemSummary[]>([]);
const detail = ref<InboxItemDetail | null>(null);
const selectedId = ref<number | null>(null);
const loadingList = ref(false);
const savingMeta = ref(false);
const hasMore = ref(false);
const nextOffset = ref(0);
const total = ref(0);
const listViewportRef = ref<HTMLElement | null>(null);
const viewportHeight = ref(600);
const scrollTop = ref(0);
const settingsDialogVisible = ref(false);
const imagePreviewVisible = ref(false);
const imageContextMenuRef = ref<HTMLElement | null>(null);
const detailEditExpanded = ref(false);
const detailExpandedSections = ref<DetailCollapseSection[]>([]);
let clipboardUnlisten: UnlistenFn | null = null;
let clipboardRefreshRunning = false;
let clipboardRefreshQueued = false;

const imageContextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
});

const metaDraft = reactive({
  title: "",
  note: "",
  starred: false,
});

const totalTypeCount = computed(() =>
  typeOptions.reduce((sum, item) => sum + typeCount(item.value), 0),
);
const overviewMetrics = computed(() => [
  { label: "当前结果", value: String(total.value), tone: "primary" },
  { label: "星标", value: String(facets.value.starred), tone: "accent" },
  { label: "外部引用", value: String(facets.value.external), tone: "warm" },
  { label: "仅摘要", value: String(facets.value.summaryOnly), tone: "muted" },
]);
const activeFilterChips = computed(() => {
  const chips: Array<{ label: string }> = [];
  if (filters.bucket !== "history") chips.push({ label: `分区：${bucketLabel(filters.bucket)}` });
  if (filters.itemType) chips.push({ label: `类型：${itemTypeLabel(filters.itemType)}` });
  if (filters.starredOnly) chips.push({ label: "仅星标" });
  if (filters.externalOnly) chips.push({ label: "外部内容" });
  if (filters.summaryOnly) chips.push({ label: "仅摘要" });
  if (filters.keyword) chips.push({ label: `搜索：${filters.keyword}` });
  return chips;
});
const hasActiveFilters = computed(() => activeFilterChips.value.length > 0);
const listSummaryLabel = computed(() => {
  if (total.value === 0) {
    return filters.keyword ? "当前搜索没有匹配结果" : "当前视图还没有记录";
  }
  return `已加载 ${items.value.length} / ${total.value} 条`;
});
const selectedPositionLabel = computed(() => {
  if (!items.value.length) return "等待新的记录进入";
  if (selectedId.value == null) return "已加载候选，待选择";
  const index = items.value.findIndex((item) => item.id === selectedId.value);
  return index >= 0 ? `已定位第 ${index + 1} 条` : "当前选择已不在列表中";
});
const visibleCount = computed(() =>
  Math.max(1, Math.ceil(viewportHeight.value / ROW_HEIGHT) + OVERSCAN * 2),
);
const startIndex = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN));
const endIndex = computed(() =>
  Math.min(items.value.length, startIndex.value + visibleCount.value),
);
const virtualItems = computed(() => items.value.slice(startIndex.value, endIndex.value));
const topSpacer = computed(() => startIndex.value * ROW_HEIGHT);
const bottomSpacer = computed(() =>
  Math.max(0, (items.value.length - endIndex.value) * ROW_HEIGHT),
);
const detailText = computed(() => {
  if (!detail.value) return "";
  return buildDisplayText(detail.value);
});
const copyableText = computed(() => (detail.value ? buildTransferText(detail.value) : ""));
const shouldShowDetailBody = computed(() => (detail.value ? hasVisibleDetailBody(detail.value) : false));
const detailPreviewText = computed(() => {
  if (!detail.value) return "";
  if (detail.value.preview) return detail.value.preview;
  if (detail.value.itemType === "unknown") {
    return "该条目仅保留格式标识和元数据，可作为后续整理线索。";
  }
  return detailText.value || "暂无摘要";
});
const currentImageDataUrl = computed(() =>
  detail.value?.itemType === "image" ? detail.value.payloadDataUrl || "" : "",
);
const currentImagePath = computed(() =>
  detail.value?.itemType === "image" && detail.value.canOpenPath ? detail.value.openPath || "" : "",
);
const canOperateCurrentImage = computed(() => !!currentImagePath.value);
const currentImageLabel = computed(() => {
  const meta = detail.value?.metaJson as Record<string, unknown> | null | undefined;
  return meta?.keptOriginal === false ? "当前图片" : "图像";
});
const imagePreviewSubtitle = computed(() => {
  if (!detail.value) return "";
  const meta = detail.value.metaJson as Record<string, unknown> | null;
  const segments: string[] = [];
  if (typeof meta?.width === "number" && typeof meta?.height === "number") {
    segments.push(`${meta.width} × ${meta.height}`);
  }
  if (detail.value.preview) {
    segments.push(detail.value.preview);
  }
  return segments.join(" · ");
});
const detailMetaEntries = computed(() => {
  if (!detail.value) return [];
  return buildDetailMetaEntries(detail.value);
});
const canTransferToVault = computed(() => {
  if (!detail.value) return false;
  return (
    ["text", "html", "rtf", "unknown"].includes(detail.value.itemType) &&
    !!buildTransferText(detail.value)
  );
});
const canPromoteCurrentItem = computed(() => {
  if (!detail.value) return false;
  return detail.value.bucket !== "inbox";
});
const promoteButtonLabel = computed(() => {
  if (!detail.value) return "转入收纳箱";
  if (detail.value.bucket === "history") return "转入收纳箱";
  if (detail.value.bucket === "archived") return "放回收纳箱";
  return "已在收纳箱";
});
const archiveButtonLabel = computed(() =>
  detail.value?.bucket === "archived" ? "恢复归档" : "归档",
);

function buildQuery(offset = 0): InboxListQuery {
  return {
    bucket: filters.bucket,
    itemType: filters.itemType,
    starredOnly: filters.starredOnly,
    externalOnly: filters.externalOnly,
    summaryOnly: filters.summaryOnly,
    keyword: filters.keyword,
    limit: PAGE_SIZE,
    offset,
  };
}

function itemTypeLabel(itemType: InboxItemType): string {
  return typeOptions.find((item) => item.value === itemType)?.label || itemType;
}

function storageKindLabel(kind: InboxStorageKind): string {
  if (kind === "inline") return "内联";
  if (kind === "external") return "外部文件";
  return "仅摘要";
}

function storageBadgeLabel(item: InboxItemSummary): string {
  if (item.storageKind === "inline") return "内联";
  if (item.storageKind === "external") return "外部存储";
  return item.metaJson?.excerpt ? "仅摘要" : "仅元数据";
}

function storageBadgeClass(item: InboxItemSummary): string {
  if (item.storageKind === "inline") return "inline";
  if (item.storageKind === "external") return "external";
  return item.metaJson?.excerpt ? "summary" : "meta-only";
}

function bucketLabel(bucket: BucketFilter): string {
  return bucketOptions.find((item) => item.value === bucket)?.label || String(bucket);
}

function bucketCount(bucket: BucketFilter): number {
  if (bucket === "all") {
    return Object.values(facets.value.buckets).reduce((sum, count) => sum + count, 0);
  }
  return facets.value.buckets[bucket as "history" | "inbox" | "archived"] || 0;
}

function typeCount(itemType: InboxItemType): number {
  return facets.value.types[itemType] || 0;
}

function formatByteSize(byteSize: number): string {
  if (!byteSize) return "0 B";
  if (byteSize < 1024) return `${byteSize} B`;
  if (byteSize < 1024 * 1024) return `${(byteSize / 1024).toFixed(1)} KB`;
  if (byteSize < 1024 * 1024 * 1024) return `${(byteSize / 1024 / 1024).toFixed(1)} MB`;
  return `${(byteSize / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function formatDateTime(value: string | null): string {
  if (!value) return "未知";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

async function loadCaptureStatus(): Promise<void> {
  captureStatus.value = (await invokeToolByChannel(
    "tool:inbox:capture-status",
    {},
  )) as InboxCaptureStatus;
}

async function loadList(
  reset = true,
  options: { preserveScroll?: boolean; refreshSelectedDetail?: boolean } = {},
): Promise<void> {
  loadingList.value = true;
  try {
    const offset = reset ? 0 : nextOffset.value;
    const channel = filters.keyword ? "tool:inbox:search" : "tool:inbox:list";
    const result = (await invokeToolByChannel(channel, buildQuery(offset))) as InboxListResult;
    items.value = reset ? result.items : [...items.value, ...result.items];
    facets.value = result.facets;
    hasMore.value = result.hasMore;
    nextOffset.value = result.nextOffset;
    total.value = result.total;
    if (reset && listViewportRef.value && !options.preserveScroll) {
      listViewportRef.value.scrollTop = 0;
      scrollTop.value = 0;
    }
    if (reset && options.preserveScroll) {
      await nextTick();
      scrollTop.value = listViewportRef.value?.scrollTop ?? scrollTop.value;
    }

    const exists =
      selectedId.value != null && items.value.some((item) => item.id === selectedId.value);
    if (!exists) {
      selectedId.value = items.value[0]?.id ?? null;
      if (selectedId.value != null) await loadDetail(selectedId.value);
      else clearDetailState();
    } else if (options.refreshSelectedDetail && selectedId.value != null) {
      await loadDetail(selectedId.value, { preserveUiState: true });
    }
  } catch (error) {
    ElMessage.error((error as Error).message || "收纳箱列表加载失败");
  } finally {
    loadingList.value = false;
  }
}

async function reloadList(): Promise<void> {
  await loadList(true);
}

async function loadMore(): Promise<void> {
  if (!hasMore.value || loadingList.value) return;
  await loadList(false);
}

async function loadDetail(id: number, options: { preserveUiState?: boolean } = {}): Promise<void> {
  closeImagePreview();
  hideImageContextMenu();
  try {
    const result = (await invokeToolByChannel("tool:inbox:get", { id })) as InboxItemDetail;
    applyLoadedDetail(result, options);
  } catch (error) {
    ElMessage.error((error as Error).message || "详情加载失败");
  }
}

async function selectItem(id: number): Promise<void> {
  selectedId.value = id;
  await loadDetail(id);
}

async function saveMeta(): Promise<void> {
  if (!detail.value) return;
  savingMeta.value = true;
  try {
    await invokeToolByChannel("tool:inbox:update-meta", {
      id: detail.value.id,
      title: metaDraft.title,
      note: metaDraft.note,
      starred: metaDraft.starred,
    });
    await Promise.all([loadDetail(detail.value.id, { preserveUiState: true }), loadList(true)]);
    ElMessage.success("已保存");
  } catch (error) {
    ElMessage.error((error as Error).message || "保存失败");
  } finally {
    savingMeta.value = false;
  }
}

async function toggleStar(): Promise<void> {
  metaDraft.starred = !metaDraft.starred;
  await saveMeta();
}

async function promoteItem(): Promise<void> {
  if (!detail.value || !canPromoteCurrentItem.value) return;
  try {
    await invokeToolByChannel("tool:inbox:promote", { id: detail.value.id });
    await Promise.all([loadDetail(detail.value.id, { preserveUiState: true }), loadList(true)]);
    ElMessage.success("已转入收纳箱");
  } catch (error) {
    ElMessage.error((error as Error).message || "转入失败");
  }
}

async function toggleArchive(): Promise<void> {
  if (!detail.value) return;
  try {
    await invokeToolByChannel("tool:inbox:archive", {
      id: detail.value.id,
      archived: detail.value.bucket !== "archived",
    });
    await Promise.all([loadDetail(detail.value.id, { preserveUiState: true }), loadList(true)]);
    ElMessage.success("状态已更新");
  } catch (error) {
    ElMessage.error((error as Error).message || "归档失败");
  }
}

async function deleteItem(): Promise<void> {
  if (!detail.value) return;
  try {
    await ElMessageBox.confirm("确定删除这条收纳记录吗？", "删除确认", {
      confirmButtonText: "删除",
      cancelButtonText: "取消",
      type: "warning",
    });
  } catch {
    return;
  }
  try {
    await invokeToolByChannel("tool:inbox:delete", { id: detail.value.id });
    selectedId.value = null;
    clearDetailState();
    closeImagePreview();
    hideImageContextMenu();
    await loadList(true);
    ElMessage.success("已删除");
  } catch (error) {
    ElMessage.error((error as Error).message || "删除失败");
  }
}

async function openPath(path: string, reveal = false): Promise<void> {
  try {
    await invokeToolByChannel("tool:inbox:open-path", { path, reveal });
  } catch (error) {
    ElMessage.error((error as Error).message || "打开路径失败");
  }
}

function buildTransferText(input: InboxItemDetail): string {
  const readableText = buildReadableText(input);
  if (readableText) return readableText;
  if (input.fileRefs.length > 0) return input.fileRefs.map((item) => item.filePath).join("\n");
  if (input.openPath) return input.openPath;
  return input.preview || input.title || "";
}

function buildDisplayText(input: InboxItemDetail): string {
  const readableText = buildReadableText(input);
  if (readableText) return readableText;
  if (["text", "html", "rtf"].includes(input.itemType)) {
    return input.preview || "";
  }
  return "";
}

function buildReadableText(input: InboxItemDetail): string {
  if (!["text", "html", "rtf", "unknown"].includes(input.itemType)) return "";
  const preferSearchText = input.itemType === "html" || input.itemType === "rtf";
  const primary = preferSearchText ? input.searchText : input.payloadText;
  const fallback = preferSearchText ? input.payloadText : input.searchText;
  return primary?.trim() || fallback?.trim() || "";
}

function buildDetailMetaEntries(input: InboxItemDetail): Array<{ label: string; value: string }> {
  if (!input.metaJson) return [];
  const meta = input.metaJson as Record<string, unknown>;
  const entries: Array<{ label: string; value: string }> = [];
  if (typeof meta.width === "number" && typeof meta.height === "number") {
    entries.push({ label: "尺寸", value: `${meta.width} × ${meta.height}` });
  }
  if (typeof meta.keptOriginal === "boolean") {
    entries.push({ label: "原图保留", value: meta.keptOriginal ? "是" : "否" });
  }
  if (typeof meta.count === "number") {
    entries.push({ label: "引用数量", value: String(meta.count) });
  }
  if (Array.isArray(meta.formats)) {
    entries.push({
      label: "格式标识",
      value: meta.formats.filter((item): item is string => typeof item === "string").join("、"),
    });
  }
  if (typeof meta.excerpt === "boolean" && meta.excerpt) {
    entries.push({ label: "正文状态", value: "仅保留摘要" });
  }
  return entries;
}

function hasVisibleDetailBody(input: InboxItemDetail): boolean {
  if (input.itemType === "image") return !!input.payloadDataUrl;
  return !!buildDisplayText(input);
}

function getDefaultDetailExpandedSections(input: InboxItemDetail): DetailCollapseSection[] {
  const sections: DetailCollapseSection[] = [];
  const hasBody = hasVisibleDetailBody(input);
  if (!hasBody && input.fileRefs.length > 0) {
    sections.push("files");
  } else if (
    !hasBody &&
    buildDetailMetaEntries(input).length > 0 &&
    (input.itemType === "unknown" ||
      input.storageKind === "metadata_only" ||
      input.metaJson?.excerpt === true)
  ) {
    sections.push("meta");
  }
  return sections;
}

function applyLoadedDetail(
  result: InboxItemDetail,
  options: { preserveUiState?: boolean } = {},
): void {
  detail.value = result;
  metaDraft.title = result.title || "";
  metaDraft.note = result.note || "";
  metaDraft.starred = result.starred;
  if (options.preserveUiState) {
    if (detailExpandedSections.value.length === 0) {
      detailExpandedSections.value = getDefaultDetailExpandedSections(result);
    }
    return;
  }
  detailEditExpanded.value = false;
  detailExpandedSections.value = getDefaultDetailExpandedSections(result);
}

function clearDetailState(): void {
  detail.value = null;
  metaDraft.title = "";
  metaDraft.note = "";
  metaDraft.starred = false;
  detailEditExpanded.value = false;
  detailExpandedSections.value = [];
}

function buildTodoDraft(input: InboxItemDetail): { title: string; description: string } {
  const text = buildTransferText(input);
  if (["text", "html", "rtf", "unknown"].includes(input.itemType) && text) {
    const lines = text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
    const [title = input.title || "收纳记录", ...rest] = lines;
    return { title, description: rest.join("\n") || input.note || "" };
  }
  const description = [input.preview, input.note, input.openPath ? `来源：${input.openPath}` : ""]
    .filter(Boolean)
    .join("\n\n");
  return { title: input.title || "收纳记录", description };
}

function transferToTodo(): void {
  if (!detail.value) return;
  setPendingToolInput({
    toolId: "todo",
    text: buildTransferText(detail.value),
    source: "inbox",
    label: detail.value.title,
    todoDraft: buildTodoDraft(detail.value),
    meta: {
      inboxItemId: detail.value.id,
      itemType: detail.value.itemType,
      openPath: detail.value.openPath,
    },
  });
  openTab("todo", "本地待办");
}

function transferToVault(): void {
  if (!detail.value || !canTransferToVault.value) return;
  setPendingToolInput({
    toolId: "vault",
    text: buildTransferText(detail.value),
    source: "inbox",
    label: detail.value.title,
    vaultDraft: {
      title: detail.value.title,
      fields: { notes: buildTransferText(detail.value) },
    },
    meta: {
      inboxItemId: detail.value.id,
      itemType: detail.value.itemType,
    },
  });
  openTab("vault", "密码库");
}

async function handleDetailActionCommand(command: string): Promise<void> {
  if (!detail.value) return;
  if (command === "toggle-star") {
    await toggleStar();
    return;
  }
  if (command === "promote") {
    await promoteItem();
    return;
  }
  if (command === "archive") {
    await toggleArchive();
    return;
  }
  if (command === "delete") {
    await deleteItem();
  }
}

async function copyDetailContent(): Promise<void> {
  if (!copyableText.value) return;
  try {
    await suppressClipboardCapture(copyableText.value);
    await navigator.clipboard.writeText(copyableText.value);
    ElMessage.success("内容已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}

function openImagePreview(): void {
  if (!currentImageDataUrl.value) return;
  hideImageContextMenu();
  imagePreviewVisible.value = true;
}

function closeImagePreview(): void {
  hideImageContextMenu();
  imagePreviewVisible.value = false;
}

function onImageContextMenu(event: MouseEvent): void {
  if (!currentImageDataUrl.value) return;
  const menuWidth = 180;
  const menuHeight = 192;
  imageContextMenu.visible = true;
  imageContextMenu.x = Math.max(8, Math.min(event.clientX, window.innerWidth - menuWidth));
  imageContextMenu.y = Math.max(8, Math.min(event.clientY, window.innerHeight - menuHeight));
}

function hideImageContextMenu(): void {
  imageContextMenu.visible = false;
}

function onDocumentClick(event: MouseEvent): void {
  if (!imageContextMenu.visible) return;
  const target = event.target as Node | null;
  if (target && imageContextMenuRef.value?.contains(target)) {
    return;
  }
  hideImageContextMenu();
}

function onDocumentKeydown(event: KeyboardEvent): void {
  if (event.key !== "Escape") return;
  hideImageContextMenu();
  closeImagePreview();
}

function onDocumentScroll(): void {
  hideImageContextMenu();
}

async function copyCurrentImage(): Promise<void> {
  if (!canOperateCurrentImage.value) return;
  hideImageContextMenu();
  try {
    await invokeToolByChannel("tool:inbox:copy-image", { path: currentImagePath.value });
    ElMessage.success(`${currentImageLabel.value}已复制到剪贴板`);
  } catch (error) {
    ElMessage.error((error as Error).message || "复制图像失败");
  }
}

async function openCurrentImage(): Promise<void> {
  if (!canOperateCurrentImage.value) return;
  hideImageContextMenu();
  await openPath(currentImagePath.value, false);
}

async function revealCurrentImage(): Promise<void> {
  if (!canOperateCurrentImage.value) return;
  hideImageContextMenu();
  await openPath(currentImagePath.value, true);
}

async function copyCurrentImagePath(): Promise<void> {
  if (!canOperateCurrentImage.value) return;
  hideImageContextMenu();
  try {
    await suppressClipboardCapture(currentImagePath.value);
    await navigator.clipboard.writeText(currentImagePath.value);
    ElMessage.success(`${currentImageLabel.value}路径已复制`);
  } catch (error) {
    ElMessage.error((error as Error).message || "复制图像路径失败");
  }
}

async function onToggleCapture(value: string | number | boolean): Promise<void> {
  try {
    await invokeToolByChannel("tool:settings:set", {
      key: "inbox_capture_enabled",
      value: value ? "true" : "false",
    });
    await loadCaptureStatus();
  } catch (error) {
    ElMessage.error((error as Error).message || "更新采集状态失败");
  }
}

async function onTogglePause(): Promise<void> {
  try {
    captureStatus.value = (await invokeToolByChannel("tool:inbox:capture-pause", {
      minutes: captureStatus.value.paused ? 0 : 5,
    })) as InboxCaptureStatus;
  } catch (error) {
    ElMessage.error((error as Error).message || "更新暂停状态失败");
  }
}

async function runCleanup(): Promise<void> {
  try {
    await invokeToolByChannel("tool:inbox:cleanup", {});
    await loadList(true);
    ElMessage.success("已执行清理");
  } catch (error) {
    ElMessage.error((error as Error).message || "清理失败");
  }
}

function selectBucket(bucket: BucketFilter): void {
  filters.bucket = bucket;
  void reloadList();
}

function selectType(itemType: InboxItemType | ""): void {
  filters.itemType = itemType;
  void reloadList();
}

function toggleFlag(key: "starredOnly" | "externalOnly" | "summaryOnly"): void {
  filters[key] = !filters[key];
  void reloadList();
}

function resetFilters(): void {
  filters.bucket = "history";
  filters.itemType = "";
  filters.starredOnly = false;
  filters.externalOnly = false;
  filters.summaryOnly = false;
  filters.keyword = "";
  void reloadList();
}

function onListScroll(event: Event): void {
  hideImageContextMenu();
  const target = event.target as HTMLElement;
  scrollTop.value = target.scrollTop;
  if (target.scrollTop + target.clientHeight >= target.scrollHeight - ROW_HEIGHT * 2) {
    void loadMore();
  }
}

function updateViewportHeight(): void {
  viewportHeight.value = listViewportRef.value?.clientHeight || 600;
}

function handleResize(): void {
  updateViewportHeight();
  hideImageContextMenu();
}

async function refreshFromClipboardChange(): Promise<void> {
  if (clipboardRefreshRunning) {
    clipboardRefreshQueued = true;
    return;
  }

  clipboardRefreshRunning = true;
  try {
    await Promise.all([
      loadCaptureStatus(),
      loadList(true, {
        preserveScroll: true,
        refreshSelectedDetail: true,
      }),
    ]);
  } finally {
    clipboardRefreshRunning = false;
    if (clipboardRefreshQueued) {
      clipboardRefreshQueued = false;
      void refreshFromClipboardChange();
    }
  }
}

onMounted(async () => {
  window.addEventListener("resize", handleResize);
  document.addEventListener("click", onDocumentClick);
  document.addEventListener("keydown", onDocumentKeydown);
  document.addEventListener("scroll", onDocumentScroll, true);
  try {
    clipboardUnlisten = await listen("clipboard-changed", () => {
      void refreshFromClipboardChange();
    });
  } catch {
    clipboardUnlisten = null;
  }
  await Promise.all([loadCaptureStatus(), loadList(true)]);
  updateViewportHeight();
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", handleResize);
  document.removeEventListener("click", onDocumentClick);
  document.removeEventListener("keydown", onDocumentKeydown);
  document.removeEventListener("scroll", onDocumentScroll, true);
  clipboardUnlisten?.();
  clipboardUnlisten = null;
});
</script>

<style scoped>
.inbox-panel {
  display: grid;
  grid-template-columns: 264px minmax(340px, 0.95fr) minmax(380px, 1.1fr);
  gap: 16px;
  height: 100%;
  min-height: 0;
}

.panel-card {
  min-height: 0;
  border: 1px solid var(--lc-border);
  border-radius: 20px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(246, 248, 251, 0.96));
  box-shadow: 0 12px 32px rgba(15, 23, 42, 0.06);
}

.inbox-sidebar,
.inbox-list,
.inbox-detail {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.inbox-sidebar {
  position: relative;
  gap: 16px;
  padding: 18px;
  overflow-y: auto;
}

.inbox-list,
.inbox-detail {
  padding: 18px;
}

.inbox-detail {
  gap: 16px;
  overflow-y: auto;
}

.sidebar-hero,
.sidebar-section,
.detail-section,
.setting-card,
.detail-edit-card,
.detail-text,
.detail-empty,
.image-box,
.detail-meta-item,
.file-ref,
.detail-placeholder-card {
  border: 1px solid var(--lc-border);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.84);
}

.sidebar-hero,
.setting-card,
.detail-edit-card,
.detail-text,
.detail-empty,
.image-box,
.detail-placeholder-card {
  padding: 14px;
}

.sidebar-section,
.detail-section {
  padding: 12px;
}

.sidebar-hero {
  background:
    linear-gradient(160deg, rgba(14, 165, 233, 0.12), rgba(255, 255, 255, 0.96) 56%),
    rgba(255, 255, 255, 0.92);
}

.sidebar-hero-eyebrow,
.detail-overview-eyebrow,
.detail-placeholder-eyebrow {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #0369a1;
}

.sidebar-hero-title,
.detail-overview-card h3,
.detail-placeholder-card h3 {
  margin: 8px 0 0;
  font-family: var(--lc-font-display);
  color: var(--lc-text);
}

.sidebar-hero-title {
  font-size: 28px;
}

.sidebar-empty-note,
.list-summary,
.row-preview,
.row-meta,
.detail-overview-preview,
.detail-origin span,
.detail-metric span,
.detail-meta-item span,
.file-ref-path,
.file-ref-submeta,
.image-box-hint,
.detail-placeholder-card p,
.setting-copy span,
.setting-meta,
.image-preview-header span {
  font-size: 12px;
  line-height: 1.6;
  color: var(--lc-text-secondary);
}

.detail-placeholder-card p {
  margin: 10px 0 0;
}

.setting-row,
.setting-meta,
.capture-actions,
.list-toolbar,
.list-summary,
.row-topline,
.row-header,
.row-badges,
.row-footer,
.row-meta,
.section-title-row,
.detail-badge-row,
.detail-overview-topline,
.detail-toolbar,
.detail-toolbar-main,
.detail-collapse-title,
.file-ref,
.image-preview-header {
  display: flex;
  align-items: center;
}

.setting-row,
.setting-meta,
.list-summary,
.row-topline,
.row-header,
.row-footer,
.row-meta,
.detail-overview-topline,
.detail-toolbar,
.file-ref,
.image-preview-header {
  justify-content: space-between;
}

.setting-row {
  align-items: flex-start;
  gap: 12px;
}

.capture-actions {
  gap: 10px;
}

.sidebar-overview-card strong,
.detail-metric strong {
  display: block;
  color: var(--lc-text);
}

.setting-card,
.setting-copy {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.sidebar-section,
.detail-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.section-title {
  font-size: 12px;
  font-weight: 700;
  color: var(--lc-text-secondary);
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.section-title-row {
  justify-content: space-between;
}

.section-link-btn,
.detail-origin-link,
.detail-overview-action {
  padding: 0;
  border: none;
  background: transparent;
  color: var(--lc-accent);
  cursor: pointer;
}

.section-link-btn,
.detail-overview-action {
  font-size: 12px;
}

.section-link-btn:hover,
.detail-origin-link:hover,
.detail-overview-action:hover {
  color: var(--lc-accent-light);
}

.sidebar-overview-grid,
.detail-overview-metrics,
.detail-meta-list {
  display: grid;
  gap: 10px;
}

.sidebar-overview-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.sidebar-overview-card {
  padding: 12px;
  border-radius: 14px;
  border: 1px solid var(--lc-border);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(241, 245, 249, 0.9));
}

.sidebar-overview-card span {
  display: block;
  font-size: 11px;
  color: var(--lc-text-secondary);
}

.sidebar-overview-card strong {
  margin-top: 8px;
  font-size: 22px;
  font-family: var(--lc-font-display);
}

.sidebar-overview-card.is-accent strong {
  color: #b45309;
}

.sidebar-overview-card.is-warm strong {
  color: #c2410c;
}

.sidebar-overview-card.is-muted strong {
  color: #475569;
}

.sidebar-filter,
.list-row,
.file-ref {
  width: 100%;
  border: 1px solid var(--lc-border);
  background: rgba(255, 255, 255, 0.84);
  color: var(--lc-text);
  transition:
    border-color 160ms var(--lc-ease),
    transform 160ms var(--lc-ease),
    box-shadow 160ms var(--lc-ease),
    background-color 160ms var(--lc-ease);
}

.sidebar-filter {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 11px 12px;
  border-radius: 14px;
  text-align: left;
  cursor: pointer;
}

.sidebar-filter:hover,
.list-row:hover,
.file-ref:hover {
  border-color: rgba(14, 165, 233, 0.28);
  box-shadow: 0 10px 28px rgba(14, 165, 233, 0.08);
  transform: translateY(-1px);
}

.sidebar-filter.is-active {
  border-color: rgba(14, 165, 233, 0.34);
  background: linear-gradient(180deg, rgba(14, 165, 233, 0.11), rgba(255, 255, 255, 0.95));
}

.sidebar-filter-copy {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.sidebar-filter-copy strong {
  font-size: 13px;
  color: var(--lc-text);
}

.sidebar-filter-count {
  display: inline-flex;
  min-width: 28px;
  justify-content: center;
  padding: 3px 8px;
  border-radius: 999px;
  background: rgba(14, 165, 233, 0.08);
  font-size: 12px;
  color: #0369a1;
}

.filter-chip-list,
.list-active-filters {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.filter-chip {
  display: inline-flex;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(14, 165, 233, 0.1);
  font-size: 11px;
  color: #0369a1;
}

.filter-chip.is-neutral {
  background: rgba(148, 163, 184, 0.12);
  color: #475569;
}

.sidebar-spacer {
  flex: 1;
}

.sidebar-actions {
  position: sticky;
  bottom: -18px;
  margin: 0 -18px -18px;
  padding: 12px 18px 18px;
  border-top: 1px solid var(--lc-border-subtle);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0), rgba(255, 255, 255, 0.96) 26%);
}

.sidebar-action-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 10px 12px;
  border: none;
  border-radius: 14px;
  background: rgba(14, 165, 233, 0.08);
  color: #075985;
  font-size: 13px;
  cursor: pointer;
}

.sidebar-action-btn:hover {
  background: rgba(14, 165, 233, 0.14);
}

.sidebar-action-btn svg {
  width: 14px;
  height: 14px;
}

.list-toolbar {
  gap: 10px;
}

.list-active-filters,
.list-summary {
  margin-top: 12px;
}

.list-summary {
  justify-content: space-between;
}

.list-viewport {
  flex: 1;
  min-height: 0;
  margin-top: 12px;
  overflow-y: auto;
  padding-right: 4px;
}

.list-row {
  display: grid;
  grid-template-rows: auto auto 1fr auto;
  gap: 10px;
  min-height: 142px;
  padding: 13px;
  margin-bottom: 12px;
  border-radius: 18px;
  text-align: left;
  cursor: pointer;
  box-sizing: border-box;
}

.list-row.is-active {
  border-color: rgba(14, 165, 233, 0.34);
  background: linear-gradient(180deg, rgba(14, 165, 233, 0.1), rgba(255, 255, 255, 0.98));
  box-shadow: 0 18px 36px rgba(14, 165, 233, 0.12);
}

.row-topline,
.row-header,
.row-footer,
.row-meta {
  justify-content: space-between;
  gap: 10px;
}

.row-topline {
  font-size: 11px;
}

.row-topline-type,
.row-topline-time {
  color: var(--lc-text-secondary);
}

.row-header {
  align-items: flex-start;
}

.row-header strong {
  display: block;
  flex: 1;
  min-width: 0;
  font-size: 16px;
  line-height: 1.35;
  color: var(--lc-text);
}

.row-badges,
.detail-badge-row {
  flex-wrap: wrap;
  gap: 6px;
}

.badge {
  display: inline-flex;
  padding: 3px 8px;
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.12);
  font-size: 11px;
  color: var(--lc-text-secondary);
}

.badge.star {
  background: rgba(245, 158, 11, 0.14);
  color: #b45309;
}

.badge.note {
  background: rgba(16, 185, 129, 0.12);
  color: #047857;
}

.badge.promoted {
  background: rgba(34, 197, 94, 0.14);
  color: #15803d;
}

.badge.inline {
  background: rgba(59, 130, 246, 0.12);
  color: #1d4ed8;
}

.badge.external {
  background: rgba(249, 115, 22, 0.14);
  color: #c2410c;
}

.badge.summary {
  background: rgba(14, 165, 233, 0.12);
  color: #0f766e;
}

.badge.meta-only {
  background: rgba(100, 116, 139, 0.14);
  color: #475569;
}

.row-preview {
  display: -webkit-box;
  margin: 0;
  font-size: 13px;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.row-meta {
  flex-wrap: wrap;
  font-size: 12px;
}

.row-meta-secondary {
  color: #0369a1;
}

.list-footer {
  padding-top: 12px;
  text-align: center;
}

.detail-overview-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 15px 16px;
  border: 1px solid rgba(14, 165, 233, 0.16);
  border-radius: 20px;
  background:
    linear-gradient(150deg, rgba(14, 165, 233, 0.11), rgba(255, 255, 255, 0.98) 58%),
    rgba(255, 255, 255, 0.92);
}

.detail-overview-preview,
.detail-text {
  margin: 10px 0 0;
}

.detail-overview-card h3 {
  margin: 0;
  font-size: 24px;
}

.detail-overview-topline {
  gap: 10px;
}

.detail-origin {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.detail-origin-link {
  display: block;
  width: 100%;
  font-size: 12px;
  text-align: left;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail-overview-metrics {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.detail-metric {
  padding: 10px 12px;
  border: 1px solid rgba(14, 165, 233, 0.12);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.78);
}

.detail-metric strong {
  margin-top: 6px;
  font-size: 14px;
}

.detail-overview-preview {
  display: -webkit-box;
  margin: 0;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.detail-toolbar {
  position: sticky;
  top: -6px;
  z-index: 4;
  align-items: flex-start;
  gap: 12px;
  padding: 12px;
  border: 1px solid rgba(14, 165, 233, 0.16);
  border-radius: 18px;
  background: rgba(248, 250, 252, 0.96);
  box-shadow: 0 12px 30px rgba(15, 23, 42, 0.08);
  backdrop-filter: blur(10px);
}

.detail-toolbar-main {
  flex: 1;
  flex-wrap: wrap;
  gap: 10px;
}

.detail-toolbar-main :deep(.el-button),
.detail-toolbar-more :deep(.el-button) {
  min-height: 40px;
  margin: 0;
}

.detail-toolbar-main :deep(.el-button) {
  min-width: 116px;
}

.detail-toolbar-more {
  flex-shrink: 0;
}

.detail-edit-card :deep(.el-form-item:last-child) {
  margin-bottom: 0;
}

.detail-text {
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--lc-font-mono);
}

.detail-empty,
.detail-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
}

.image-box {
  padding: 14px;
}

.detail-image {
  display: block;
  max-width: 100%;
  max-height: 360px;
  margin: 0 auto;
  border-radius: 12px;
  cursor: zoom-in;
  transition:
    transform 180ms var(--lc-ease),
    box-shadow 180ms var(--lc-ease);
}

.detail-image:hover {
  transform: translateY(-1px);
  box-shadow: 0 12px 30px rgba(15, 23, 42, 0.14);
}

.image-box-hint {
  margin-top: 10px;
  text-align: center;
}

.detail-meta-list {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.detail-collapse {
  display: flex;
  flex-direction: column;
  gap: 12px;
  border: none;
}

.detail-collapse-count {
  display: inline-flex;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(14, 165, 233, 0.08);
  font-size: 11px;
  color: #0369a1;
}

.detail-collapse-title {
  gap: 8px;
}

.detail-collapse :deep(.el-collapse) {
  border: none;
}

.detail-collapse :deep(.el-collapse-item) {
  overflow: hidden;
  border: 1px solid var(--lc-border);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.84);
}

.detail-collapse :deep(.el-collapse-item__header) {
  min-height: 52px;
  padding: 0 14px;
  border: none;
  background: transparent;
  line-height: 1.2;
}

.detail-collapse :deep(.el-collapse-item__wrap) {
  border: none;
  background: transparent;
}

.detail-collapse :deep(.el-collapse-item__content) {
  padding: 0 12px 12px;
}

.detail-collapse :deep(.el-collapse-item__arrow) {
  color: var(--lc-text-secondary);
}

.detail-meta-item,
.file-ref {
  padding: 12px;
}

.file-ref {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 8px;
  border-radius: 16px;
}

.file-ref-main {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
  text-align: left;
}

.file-ref-submeta {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.detail-placeholder {
  min-height: 260px;
}

.detail-placeholder-card {
  max-width: 360px;
  text-align: center;
}

.image-preview-overlay {
  position: fixed;
  inset: 0;
  z-index: 2200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 32px;
  background: rgba(15, 23, 42, 0.72);
  backdrop-filter: blur(6px);
}

.image-preview-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  width: min(92vw, 1200px);
  max-height: calc(100vh - 64px);
  padding: 18px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 20px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 24px 80px rgba(15, 23, 42, 0.28);
}

.image-preview-header {
  justify-content: space-between;
  gap: 12px;
}

.image-preview-header strong,
.image-preview-header span {
  display: block;
}

.image-preview-close {
  padding: 8px 12px;
  border: 1px solid var(--lc-border);
  border-radius: 999px;
  background: var(--lc-surface-0);
  color: var(--lc-text);
  cursor: pointer;
}

.image-preview-media {
  display: block;
  max-width: 100%;
  max-height: calc(85vh - 96px);
  margin: 0 auto;
  border-radius: 14px;
  object-fit: contain;
}

.image-context-menu {
  position: fixed;
  z-index: 2300;
  min-width: 144px;
  padding: 4px;
  border: 1px solid var(--lc-border);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 16px 40px rgba(15, 23, 42, 0.18);
  backdrop-filter: blur(8px);
}

.image-context-menu-item {
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--lc-text);
  text-align: left;
  cursor: pointer;
}

.image-context-menu-item:hover {
  background: rgba(14, 165, 233, 0.08);
}

.image-context-menu-item.is-disabled {
  color: var(--lc-text-secondary);
  cursor: not-allowed;
}

.image-context-menu-item.is-disabled:hover {
  background: transparent;
}

@media (max-width: 1240px) {
  .inbox-panel {
    grid-template-columns: 264px minmax(0, 1fr);
  }

  .inbox-detail {
    grid-column: 1 / -1;
  }
}

@media (max-width: 960px) {
  .inbox-panel {
    grid-template-columns: 1fr;
  }

  .inbox-sidebar,
  .inbox-detail {
    grid-column: auto;
  }

  .detail-overview-metrics,
  .detail-meta-list,
  .sidebar-overview-grid {
    grid-template-columns: 1fr 1fr;
  }
}

@media (max-width: 720px) {
  .inbox-sidebar,
  .inbox-list,
  .inbox-detail {
    padding: 14px;
  }

  .detail-overview-metrics,
  .detail-meta-list,
  .sidebar-overview-grid {
    grid-template-columns: 1fr;
  }

  .detail-toolbar {
    flex-direction: column;
    align-items: stretch;
  }

  .detail-toolbar-more {
    align-self: stretch;
  }

  .detail-toolbar-more :deep(.el-button) {
    width: 100%;
  }

  .image-preview-overlay {
    padding: 16px;
  }

  .image-preview-panel {
    width: 100%;
    padding: 14px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .sidebar-filter,
  .list-row,
  .file-ref,
  .detail-image {
    transition: none;
  }
}
</style>
