<template>
  <div ref="containerRef" class="rte-viewer" @click="onClick">
    <div v-if="parsedDoc" class="rte-prose" v-html="html" />
    <div v-else-if="fallbackText" class="rte-prose rte-legacy">{{ fallbackText }}</div>
    <div v-else class="rte-empty">{{ emptyText }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import type { JSONContent } from '@tiptap/vue-3';
import { renderToHTMLString } from '@tiptap/static-renderer/pm/html-string';
import { convertFileSrc } from '@tauri-apps/api/core';

import { buildExtensions } from '../rich/extensions';
import { rewriteLocalSrc, tryParseDoc } from '../rich/legacy';
import { invokeToolByChannel } from '../bridge/tauri';

const props = withDefaults(
  defineProps<{
    value: string;
    emptyText?: string;
  }>(),
  { emptyText: '' }
);

const containerRef = ref<HTMLElement | null>(null);

const parsedDoc = computed(() => tryParseDoc(props.value));
const fallbackText = computed(() => {
  // 无法解析 JSON 且原值非空：走 legacy 纯文本渲染
  const t = props.value?.trim?.() ?? '';
  return parsedDoc.value ? '' : t;
});

/**
 * dataDir 缓存：Viewer 挂载后一次性取，之后所有 Viewer 共享这份静态值。
 * 通过 module-level 闭包避免重复 IPC；首次失败会 retry。
 */
let sharedDataDir = '';
let sharedDataDirPromise: Promise<string> | null = null;

async function fetchDataDir(): Promise<string> {
  if (sharedDataDir) return sharedDataDir;
  if (!sharedDataDirPromise) {
    sharedDataDirPromise = invokeToolByChannel('tool:system:get-paths', {})
      .then((res) => {
        const v = (res as { dataDir?: string })?.dataDir ?? '';
        sharedDataDir = v;
        return v;
      })
      .catch(() => '') as Promise<string>;
  }
  return sharedDataDirPromise;
}

const dataDir = ref('');
onMounted(async () => {
  dataDir.value = await fetchDataDir();
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
    return renderToHTMLString({
      extensions: buildExtensions(),
      content: rewrittenDoc.value,
    });
  } catch {
    return '';
  }
});

function joinPath(dir: string, sub: string): string {
  const d = dir.replace(/[\/\\]+$/, '');
  const s = sub.replace(/^[\/\\]+/, '');
  // convertFileSrc 对斜杠不敏感，统一正斜杠可读性更好
  return `${d}/${s}`.replace(/\\/g, '/');
}

function onClick(e: MouseEvent): void {
  const target = e.target as Element | null;
  const a = target?.closest?.('a[href]') as HTMLAnchorElement | null;
  if (!a) return;
  e.preventDefault();
  const href = (a.getAttribute('href') ?? '').trim();
  if (!/^(https?:|mailto:)/i.test(href)) return;
  invokeToolByChannel('tool:system:open-external', { url: href }).catch(() => {
    /* 忽略：后端会二次校验协议并返回错误，这里不弹窗打扰 */
  });
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
