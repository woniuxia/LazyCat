<template>
  <div class="inbox-panel">
    <aside class="inbox-sidebar panel-card">
      <section class="sidebar-section">
        <div class="section-title">分区</div>
        <button
          v-for="option in bucketOptions"
          :key="option.value"
          class="sidebar-filter"
          :class="{ 'is-active': filters.bucket === option.value }"
          @click="selectBucket(option.value)"
        >
          <span>{{ option.label }}</span>
          <span>{{ bucketCount(option.value) }}</span>
        </button>
      </section>

      <section class="sidebar-section">
        <div class="section-title">类型</div>
        <button
          class="sidebar-filter"
          :class="{ 'is-active': filters.itemType === '' }"
          @click="selectType('')"
        >
          <span>全部类型</span>
          <span>{{ totalTypeCount }}</span>
        </button>
        <button
          v-for="option in typeOptions"
          :key="option.value"
          class="sidebar-filter"
          :class="{ 'is-active': filters.itemType === option.value }"
          @click="selectType(option.value)"
        >
          <span>{{ option.label }}</span>
          <span>{{ typeCount(option.value) }}</span>
        </button>
      </section>

      <section class="sidebar-section">
        <div class="section-title">筛选</div>
        <button
          class="sidebar-filter"
          :class="{ 'is-active': filters.starredOnly }"
          @click="toggleFlag('starredOnly')"
        >
          <span>仅星标</span>
          <span>{{ facets.starred }}</span>
        </button>
        <button
          class="sidebar-filter"
          :class="{ 'is-active': filters.externalOnly }"
          @click="toggleFlag('externalOnly')"
        >
          <span>外部内容</span>
          <span>{{ facets.external }}</span>
        </button>
        <button
          class="sidebar-filter"
          :class="{ 'is-active': filters.summaryOnly }"
          @click="toggleFlag('summaryOnly')"
        >
          <span>仅摘要</span>
          <span>{{ facets.summaryOnly }}</span>
        </button>
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
      <div class="list-summary">
        <span>{{ currentBucketLabel }} · {{ total }} 条</span>
        <span v-if="loadingList">加载中...</span>
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
          class="list-row"
          :class="{ 'is-active': item.id === selectedId }"
          @click="selectItem(item.id)"
        >
          <div class="row-header">
            <strong>{{ item.title || "(未命名)" }}</strong>
            <div class="row-badges">
              <span v-if="item.bucket !== 'history'" class="badge promoted">
                {{ item.bucket === "archived" ? "已归档" : "已升格" }}
              </span>
              <span v-if="item.starred" class="badge star">星标</span>
              <span class="badge">{{ itemTypeLabel(item.itemType) }}</span>
              <span class="badge" :class="storageBadgeClass(item)">
                {{ storageBadgeLabel(item) }}
              </span>
            </div>
          </div>
          <div class="row-preview">{{ item.preview || "暂无摘要" }}</div>
          <div class="row-meta">
            <span>{{ formatDateTime(item.lastSeenAt) }}</span>
            <span>{{ formatByteSize(item.byteSize) }}</span>
            <span>{{ item.seenCount }} 次</span>
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
        <div class="detail-header">
          <div>
            <h3>{{ detail.title || "(未命名)" }}</h3>
            <div class="detail-subline">
              {{ itemTypeLabel(detail.itemType) }} · {{ bucketLabel(detail.bucket) }} ·
              {{ formatDateTime(detail.lastSeenAt) }}
            </div>
          </div>
          <div class="detail-header-actions">
            <el-button size="small" @click="toggleStar">
              {{ metaDraft.starred ? "取消星标" : "设为星标" }}
            </el-button>
            <el-button size="small" @click="promoteItem">转入收纳箱</el-button>
          </div>
        </div>

        <div class="detail-actions">
          <el-button type="primary" @click="transferToTodo">转任务清单</el-button>
          <el-button :disabled="!canTransferToVault" @click="transferToVault">存入密码库</el-button>
          <el-button :disabled="!copyableText" @click="copyDetailContent">复制内容</el-button>
          <el-button disabled>转便签（后续支持）</el-button>
          <el-button @click="toggleArchive">
            {{ detail.bucket === "archived" ? "恢复" : "归档" }}
          </el-button>
          <el-button
            v-if="detail.canOpenPath && detail.openPath"
            @click="openPath(detail.openPath, true)"
          >
            打开位置
          </el-button>
          <el-button type="danger" @click="deleteItem">删除</el-button>
        </div>

        <div class="detail-section">
          <div class="section-title">编辑</div>
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

        <div class="detail-section">
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
          <div v-else-if="detail.itemType === 'unknown'" class="detail-empty">
            该格式未持久化原始内容，仅保留格式标识和基础元数据。
          </div>
          <pre v-else-if="detailText" class="detail-text">{{ detailText }}</pre>
          <div v-else class="detail-empty">该条目没有可直接展示的正文。</div>
        </div>

        <div v-if="detailMetaEntries.length > 0" class="detail-section">
          <div class="section-title">元数据</div>
          <div class="detail-meta-list">
            <div v-for="entry in detailMetaEntries" :key="entry.label" class="detail-meta-item">
              <span>{{ entry.label }}</span>
              <strong>{{ entry.value }}</strong>
            </div>
          </div>
        </div>

        <div v-if="detail.fileRefs.length > 0" class="detail-section">
          <div class="section-title">文件引用</div>
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
        </div>

        <div class="detail-grid">
          <div class="detail-metric">
            <span>首次记录</span>
            <strong>{{ formatDateTime(detail.capturedAt) }}</strong>
          </div>
          <div class="detail-metric">
            <span>最近出现</span>
            <strong>{{ formatDateTime(detail.lastSeenAt) }}</strong>
          </div>
          <div class="detail-metric">
            <span>存储方式</span>
            <strong>{{ storageKindLabel(detail.storageKind) }}</strong>
          </div>
          <div class="detail-metric">
            <span>体积</span>
            <strong>{{ formatByteSize(detail.byteSize) }}</strong>
          </div>
        </div>
      </template>

      <div v-else class="detail-placeholder">
        <h3>选择一条收纳记录</h3>
        <p>左侧筛选，中间浏览，右侧查看详情并转入待办或密码库。</p>
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
const ROW_HEIGHT = 106;
const OVERSCAN = 8;

type BucketFilter = InboxListQuery["bucket"] | "all";

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

const currentBucketLabel = computed(
  () => bucketOptions.find((item) => item.value === filters.bucket)?.label || "全部",
);
const totalTypeCount = computed(() =>
  typeOptions.reduce((sum, item) => sum + typeCount(item.value), 0),
);
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
  return buildReadableText(detail.value) || detail.value.preview || "";
});
const copyableText = computed(() => (detail.value ? buildTransferText(detail.value) : ""));
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
  if (!detail.value || !detail.value.metaJson) return [];
  const meta = detail.value.metaJson as Record<string, unknown>;
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
});
const canTransferToVault = computed(() => {
  if (!detail.value) return false;
  return (
    ["text", "html", "rtf", "unknown"].includes(detail.value.itemType) &&
    !!buildTransferText(detail.value)
  );
});

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
      else detail.value = null;
    } else if (options.refreshSelectedDetail && selectedId.value != null) {
      await loadDetail(selectedId.value);
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

async function loadDetail(id: number): Promise<void> {
  closeImagePreview();
  hideImageContextMenu();
  try {
    const result = (await invokeToolByChannel("tool:inbox:get", { id })) as InboxItemDetail;
    detail.value = result;
    metaDraft.title = result.title || "";
    metaDraft.note = result.note || "";
    metaDraft.starred = result.starred;
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
    await Promise.all([loadDetail(detail.value.id), loadList(true)]);
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
  if (!detail.value) return;
  try {
    await invokeToolByChannel("tool:inbox:promote", { id: detail.value.id });
    await Promise.all([loadDetail(detail.value.id), loadList(true)]);
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
    await Promise.all([loadDetail(detail.value.id), loadList(true)]);
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
    detail.value = null;
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

function buildReadableText(input: InboxItemDetail): string {
  if (!["text", "html", "rtf", "unknown"].includes(input.itemType)) return "";
  const preferSearchText = input.itemType === "html" || input.itemType === "rtf";
  const primary = preferSearchText ? input.searchText : input.payloadText;
  const fallback = preferSearchText ? input.payloadText : input.searchText;
  return primary?.trim() || fallback?.trim() || "";
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

async function copyDetailContent(): Promise<void> {
  if (!copyableText.value) return;
  try {
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
  grid-template-columns: 248px minmax(320px, 0.9fr) minmax(360px, 1fr);
  gap: 16px;
  height: 100%;
  min-height: 0;
}

.panel-card {
  min-height: 0;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-lg);
  background: var(--lc-surface-0);
  box-shadow: var(--lc-shadow-sm);
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
  padding: 16px;
  overflow-y: auto;
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

.setting-card,
.detail-metric,
.detail-text,
.detail-empty,
.image-box {
  border: 1px solid var(--lc-border);
  border-radius: 14px;
  background: var(--lc-surface-1);
}

.setting-card,
.detail-metric {
  padding: 12px;
}

.setting-row,
.setting-meta,
.capture-actions,
.list-toolbar,
.list-summary,
.row-header,
.row-meta,
.detail-header,
.detail-header-actions,
.detail-actions {
  display: flex;
  align-items: center;
}

.setting-row,
.setting-meta,
.list-summary,
.detail-header {
  justify-content: space-between;
}

.capture-actions,
.detail-header-actions,
.detail-actions,
.row-badges {
  gap: 8px;
}

.setting-copy span,
.setting-meta,
.detail-subline,
.row-preview,
.row-meta,
.detail-placeholder p {
  font-size: 12px;
  color: var(--lc-text-secondary);
}

.setting-card,
.setting-copy {
  display: flex;
  flex-direction: column;
}

.setting-card,
.setting-copy,
.capture-actions {
  gap: 10px;
}

.setting-row {
  gap: 12px;
  align-items: flex-start;
}

.setting-copy {
  flex: 1;
  min-width: 0;
}

.setting-copy strong {
  font-size: 13px;
}

.setting-copy span {
  line-height: 1.5;
}

.setting-meta {
  font-size: 12px;
}

.sidebar-spacer {
  flex: 1;
}

.sidebar-actions {
  position: sticky;
  bottom: -16px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0 -16px -16px;
  padding: 12px 16px 16px;
  border-top: 1px solid var(--lc-border-subtle);
  background: linear-gradient(
    180deg,
    rgba(255, 255, 255, 0) 0%,
    rgba(255, 255, 255, 0.92) 18%,
    var(--lc-surface-0) 42%
  );
  z-index: 2;
}

.sidebar-action-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border: none;
  border-radius: var(--lc-radius-sm);
  background: transparent;
  color: var(--lc-text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: all 150ms var(--lc-ease);
}

.sidebar-action-btn:hover {
  background: var(--lc-surface-1);
  color: var(--lc-text);
}

.sidebar-action-btn svg {
  width: 14px;
  height: 14px;
}

.sidebar-filter,
.list-row,
.file-ref {
  width: 100%;
  border: 1px solid var(--lc-border);
  border-radius: 12px;
  background: var(--lc-surface-0);
  color: var(--lc-text-primary);
  cursor: pointer;
}

.sidebar-filter,
.file-ref {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
}

.sidebar-filter.is-active {
  border-color: rgba(14, 116, 144, 0.32);
  background: rgba(14, 165, 233, 0.08);
}

.inbox-list,
.inbox-detail {
  padding: 16px;
}

.list-toolbar {
  gap: 10px;
  margin-bottom: 10px;
}

.list-viewport {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}

.list-row {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 96px;
  padding: 12px;
  margin-bottom: 10px;
  text-align: left;
}

.list-row.is-active {
  border-color: rgba(14, 116, 144, 0.32);
  box-shadow: 0 10px 24px rgba(14, 116, 144, 0.1);
}

.badge {
  display: inline-flex;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.12);
  font-size: 11px;
  color: var(--lc-text-secondary);
}

.badge.star {
  background: rgba(245, 158, 11, 0.14);
  color: #b45309;
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

.list-footer {
  padding-top: 10px;
  text-align: center;
}

.inbox-detail {
  overflow-y: auto;
}

.detail-header {
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--lc-border);
}

.detail-header h3 {
  margin: 0;
  font-size: 20px;
}

.detail-actions {
  flex-wrap: wrap;
  margin-top: 16px;
}

.detail-text {
  padding: 12px;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--lc-font-mono);
  line-height: 1.6;
}

.image-box {
  padding: 12px;
}

.detail-image {
  display: block;
  max-width: 100%;
  max-height: 360px;
  margin: 0 auto;
  border-radius: 10px;
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
  font-size: 12px;
  color: var(--lc-text-secondary);
}

.file-ref {
  margin-bottom: 8px;
}

.file-ref-main {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
  text-align: left;
}

.file-ref-path,
.file-ref-submeta {
  font-size: 12px;
  color: var(--lc-text-secondary);
}

.file-ref-submeta {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.detail-meta-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.detail-meta-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 12px;
  border: 1px solid var(--lc-border);
  border-radius: 14px;
  background: var(--lc-surface-1);
}

.detail-meta-item span {
  font-size: 12px;
  color: var(--lc-text-secondary);
}

.detail-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-top: 18px;
}

.detail-metric {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.detail-placeholder {
  margin: auto;
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
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.image-preview-header strong,
.image-preview-header span {
  display: block;
}

.image-preview-header span {
  margin-top: 4px;
  font-size: 12px;
  color: var(--lc-text-secondary);
}

.image-preview-close {
  padding: 8px 12px;
  border: 1px solid var(--lc-border);
  border-radius: 999px;
  background: var(--lc-surface-0);
  color: var(--lc-text-primary);
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
  color: var(--lc-text-primary);
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

@media (max-width: 1200px) {
  .inbox-panel {
    grid-template-columns: 1fr;
    grid-template-rows: auto minmax(320px, 1fr) minmax(320px, 1fr);
  }

  .inbox-sidebar {
    max-height: 320px;
  }

  .image-preview-overlay {
    padding: 16px;
  }

  .image-preview-panel {
    width: 100%;
    padding: 14px;
  }
}
</style>
