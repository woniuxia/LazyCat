<template>
  <div class="md-panel">
    <div class="md-layout">
      <div class="md-editor">
        <MonacoPane v-model="source" language="markdown" />
      </div>
      <div class="md-preview" v-html="renderedHtml"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import MarkdownIt from "markdown-it";
import DOMPurify from "dompurify";
import MonacoPane from "./MonacoPane.vue";

const source = ref("# Markdown 预览\n\n在左侧编辑，右侧实时预览。\n\n- 列表项 1\n- 列表项 2\n\n```js\nconsole.log('hello');\n```\n");

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
});

const renderedHtml = computed(() => {
  const raw = md.render(source.value);
  return DOMPurify.sanitize(raw);
});
</script>

<style scoped>
.md-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.md-layout {
  display: flex;
  gap: 16px;
  flex: 1;
  min-height: 0;
}

.md-editor {
  flex: 1;
  min-width: 0;
}

.md-preview {
  flex: 1;
  min-width: 0;
  padding: 16px;
  border: 1px solid var(--lc-border);
  border-radius: 10px;
  overflow-y: auto;
  line-height: 1.6;
  font-size: 14px;
}

.md-preview :deep(h1) {
  font-size: 1.6em;
  margin: 0.5em 0;
  border-bottom: 1px solid var(--el-border-color);
  padding-bottom: 0.3em;
}

.md-preview :deep(h2) {
  font-size: 1.3em;
  margin: 0.5em 0;
}

.md-preview :deep(pre) {
  background: var(--el-fill-color);
  padding: 12px;
  border-radius: 6px;
  overflow-x: auto;
}

.md-preview :deep(code) {
  font-family: monospace;
  font-size: 0.9em;
}

.md-preview :deep(p code) {
  background: var(--el-fill-color);
  padding: 2px 6px;
  border-radius: 3px;
}

.md-preview :deep(ul),
.md-preview :deep(ol) {
  padding-left: 1.5em;
}

.md-preview :deep(blockquote) {
  border-left: 3px solid var(--el-color-primary);
  margin: 0.5em 0;
  padding: 0.5em 1em;
  color: var(--el-text-color-secondary);
}

.md-preview :deep(table) {
  border-collapse: collapse;
  width: 100%;
}

.md-preview :deep(th),
.md-preview :deep(td) {
  border: 1px solid var(--el-border-color);
  padding: 8px;
}
</style>
