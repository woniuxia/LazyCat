<template>
  <div
    ref="containerRef"
    class="rte-viewer"
    @click="onClick"
    @contextmenu="onContextMenu"
    @dblclick="onImageDblClick"
  >
    <div v-if="parsedDoc" class="rte-prose" v-html="html" />
    <div v-else-if="fallbackText" class="rte-prose rte-legacy">{{ fallbackText }}</div>
    <div v-else class="rte-empty">{{ emptyText }}</div>
    <RteFileRefMenu
      :visible="menu.visible"
      :x="menu.x"
      :y="menu.y"
      :kind="menu.kind"
      :can-delete="false"
      @close="menu.visible = false"
      @action="onMenuAction"
    />
    <RteImagePreview v-if="previewSrc" :src="previewSrc" @close="closePreview" />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import type { JSONContent } from '@tiptap/vue-3';
import { convertFileSrc } from '@tauri-apps/api/core';
import { ElMessage } from 'element-plus';

import { rewriteLocalSrc, tryParseDoc, walkFileRefPaths } from '../rich/legacy';
import { renderRichDescription } from '../rich/render';
import { ensureDataDir } from '../rich/data-dir';
import type { FileRefKind } from '../rich/data-dir';
import { invokeToolByChannel } from '../bridge/tauri';
import RteFileRefMenu from './RteFileRefMenu.vue';
import RteImagePreview from './RteImagePreview.vue';

const props = withDefaults(
  defineProps<{
    value: string;
    emptyText?: string;
  }>(),
  { emptyText: '' }
);

const containerRef = ref<HTMLElement | null>(null);
const previewSrc = ref('');

const parsedDoc = computed(() => tryParseDoc(props.value));
const fallbackText = computed(() => {
  // 无法解析 JSON 且原值非空：走 legacy 纯文本渲染
  const t = props.value?.trim?.() ?? '';
  return parsedDoc.value ? '' : t;
});

const dataDir = ref('');
onMounted(async () => {
  dataDir.value = await ensureDataDir();
});

const rewrittenDoc = computed<JSONContent | null>(() => {
  if (!parsedDoc.value) return null;
  return rewriteLocalSrc(parsedDoc.value, (src) => {
    const root = dataDir.value;
    if (!root) return src;
    // attrs.src 约定 'attachments/<hash>.<ext>'
    const abs = joinPath(root, src);
    try {
      return convertFileSrc(abs);
    } catch {
      return src;
    }
  });
});

const html = computed(() => {
  if (!rewrittenDoc.value) return '';
  try {
    return renderRichDescription(rewrittenDoc.value);
  } catch {
    return '';
  }
});

// ── FileRef 失效检测：挂载 & 内容变化时批量 stat 原路径类节点 ─

const missingPaths = ref<Set<string>>(new Set());

async function detectMissingPaths(): Promise<void> {
  const doc = parsedDoc.value;
  if (!doc) {
    if (missingPaths.value.size > 0) missingPaths.value = new Set();
    return;
  }
  const paths = walkFileRefPaths(doc);
  if (paths.length === 0) {
    if (missingPaths.value.size > 0) missingPaths.value = new Set();
    return;
  }
  try {
    const res = (await invokeToolByChannel('tool:system:check-paths-exist', {
      paths,
    })) as { missing?: string[] };
    const list = Array.isArray(res?.missing) ? res.missing : [];
    missingPaths.value = new Set(list);
  } catch {
    missingPaths.value = new Set();
  }
  await nextTick();
  applyMissingClass();
}

function applyMissingClass(): void {
  const root = containerRef.value;
  if (!root) return;
  const nodes = root.querySelectorAll<HTMLElement>('.rte-file-ref[data-kind="path"]');
  nodes.forEach((el) => {
    const src = el.getAttribute('data-src') ?? '';
    if (missingPaths.value.has(src)) {
      el.classList.add('is-missing');
      el.setAttribute('title', '文件不存在');
    } else {
      el.classList.remove('is-missing');
      el.removeAttribute('title');
    }
  });
}

onMounted(() => {
  void detectMissingPaths();
});

watch(
  () => props.value,
  () => {
    void detectMissingPaths();
  }
);

watch(
  () => html.value,
  async () => {
    await nextTick();
    applyMissingClass();
  }
);

function joinPath(dir: string, sub: string): string {
  const d = dir.replace(/[\/\\]+$/, '');
  const s = sub.replace(/^[\/\\]+/, '');
  // convertFileSrc 对斜杠不敏感，统一正斜杠可读性更好
  return `${d}/${s}`.replace(/\\/g, '/');
}

function joinLocalPath(dir: string, sub: string): string {
  const d = dir.replace(/[\/\\]+$/, '');
  const s = sub.replace(/^[\/\\]+/, '');
  return `${d}/${s}`;
}

// ── 左键 / 右键 / 菜单 ─────────────────────────────

interface MenuTarget {
  src: string;
  name: string;
  kind: FileRefKind;
}
const menu = reactive({
  visible: false,
  x: 0,
  y: 0,
  kind: 'attachment' as FileRefKind,
  target: null as MenuTarget | null,
});

function onClick(e: MouseEvent): void {
  const target = e.target as Element | null;
  const fileEl = target?.closest?.('.rte-file-ref') as HTMLElement | null;
  if (fileEl) {
    e.preventDefault();
    const kind = (fileEl.getAttribute('data-kind') === 'path' ? 'path' : 'attachment') as FileRefKind;
    const src = fileEl.getAttribute('data-src') ?? '';
    if (!src) return;
    void openFileRef(kind, src);
    return;
  }
  const a = target?.closest?.('a[href]') as HTMLAnchorElement | null;
  if (!a) return;
  e.preventDefault();
  const href = (a.getAttribute('href') ?? '').trim();
  if (!/^(https?:|mailto:)/i.test(href)) return;
  invokeToolByChannel('tool:system:open-external', { url: href }).catch(() => {
    /* 忽略：后端会二次校验协议并返回错误，这里不弹窗打扰 */
  });
}

function onImageDblClick(e: MouseEvent): void {
  const target = e.target as Element | null;
  const img = target?.closest?.('img') as HTMLImageElement | null;
  if (!img) return;
  e.preventDefault();
  previewSrc.value = img.src;
}

function closePreview(): void {
  previewSrc.value = '';
}

function onContextMenu(e: MouseEvent): void {
  const target = e.target as Element | null;
  const el = target?.closest?.('.rte-file-ref') as HTMLElement | null;
  if (!el) return;
  e.preventDefault();
  const kind = (el.getAttribute('data-kind') === 'path' ? 'path' : 'attachment') as FileRefKind;
  const src = el.getAttribute('data-src') ?? '';
  const name = el.getAttribute('data-name') ?? '';
  menu.target = { src, name, kind };
  menu.kind = kind;
  menu.x = e.clientX;
  menu.y = e.clientY;
  menu.visible = true;
}

async function onMenuAction(action: 'open' | 'reveal' | 'copy-path' | 'delete'): Promise<void> {
  if (action === 'delete') return; // Viewer 不支持删除
  const target = menu.target;
  if (!target) return;
  const abs = await resolveAbsPath(target.kind, target.src);
  if (!abs) {
    ElMessage.error('附件路径解析失败');
    return;
  }
  if (action === 'open') {
    void openAbsPath(abs);
  } else if (action === 'reveal') {
    try {
      await invokeToolByChannel('tool:system:reveal-in-folder', { path: abs });
    } catch (err) {
      ElMessage.error((err as Error).message || '无法定位文件');
    }
  } else if (action === 'copy-path') {
    try {
      await navigator.clipboard.writeText(abs);
      ElMessage.success('路径已复制');
    } catch {
      ElMessage.error('复制失败');
    }
  }
}

async function resolveAbsPath(kind: FileRefKind, src: string): Promise<string> {
  if (!src) return '';
  if (kind === 'path') return src;
  const dir = await ensureDataDir();
  if (!dir) return '';
  return joinLocalPath(dir, src);
}

async function openFileRef(kind: FileRefKind, src: string): Promise<void> {
  const abs = await resolveAbsPath(kind, src);
  if (!abs) {
    ElMessage.error('附件路径解析失败');
    return;
  }
  await openAbsPath(abs);
}

async function openAbsPath(absPath: string): Promise<void> {
  try {
    await invokeToolByChannel('tool:system:open-local-path', { path: absPath });
  } catch (err) {
    const msg = (err as Error).message || '';
    if (msg.includes('file not found')) {
      ElMessage.error('文件不存在');
    } else {
      ElMessage.error(msg || '无法打开文件');
    }
  }
}

onUnmounted(() => {
  /* 无额外清理，convertFileSrc 产出的 URL 由 WebView 管理 */
});
</script>

<style scoped>
.rte-viewer {
  font-size: 14px;
  line-height: 1.65;
  color: var(--el-text-color-primary);
  word-break: break-word;
}
.rte-viewer :deep(p) {
  margin: 4px 0;
}
.rte-viewer :deep(h1),
.rte-viewer :deep(h2),
.rte-viewer :deep(h3) {
  margin: 10px 0 6px;
  color: var(--el-text-color-primary);
  font-weight: 600;
}
.rte-viewer :deep(h1) { font-size: 18px; }
.rte-viewer :deep(h2) { font-size: 16px; }
.rte-viewer :deep(h3) { font-size: 14.5px; }
.rte-viewer :deep(ul),
.rte-viewer :deep(ol) {
  padding-left: 22px;
  margin: 4px 0;
}
.rte-viewer :deep(blockquote) {
  border-left: 3px solid var(--el-border-color);
  color: var(--el-text-color-regular);
  padding: 2px 10px;
  margin: 6px 0;
}
.rte-viewer :deep(pre) {
  background: var(--el-fill-color);
  padding: 8px 10px;
  border-radius: 4px;
  overflow-x: auto;
  font-family: var(--el-font-family-monospace, Menlo, Consolas, monospace);
  font-size: 13px;
}
.rte-viewer :deep(code) {
  background: var(--el-fill-color);
  padding: 0 4px;
  border-radius: 3px;
  font-size: 13px;
}
.rte-viewer :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: 4px;
  margin: 4px 0;
  cursor: zoom-in;
}
.rte-viewer :deep(a) {
  color: var(--el-color-primary);
  text-decoration: underline;
}
.rte-viewer :deep(hr) {
  border: none;
  border-top: 1px solid var(--el-border-color);
  margin: 10px 0;
}
.rte-legacy {
  white-space: pre-wrap;
}
.rte-empty {
  color: var(--el-text-color-placeholder);
  font-size: 13px;
}
</style>
