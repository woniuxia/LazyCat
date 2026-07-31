<template>
  <section class="md-panel" aria-label="Markdown 工作台">
    <header class="md-toolbar">
      <div class="md-file-meta" :title="currentPath || '尚未关联文件'">
        <span class="md-file-name">{{ currentFileName }}</span>
        <span v-if="isDirty" class="md-dirty" aria-label="有未保存修改">未保存</span>
      </div>

      <div class="md-toolbar-actions">
        <el-button size="small" :loading="opening" @click="openFile">打开</el-button>
        <el-button
          size="small"
          :loading="saving"
          :disabled="!isDirty && Boolean(currentPath)"
          @click="saveFile(false)"
        >
          保存
        </el-button>
        <el-button size="small" :loading="saving" @click="saveFile(true)">另存为</el-button>
        <span class="md-toolbar-divider" aria-hidden="true"></span>
        <el-button size="small" :disabled="!source" @click="copyHtml">复制 HTML</el-button>
        <el-button size="small" :disabled="!source" @click="clearSource">清空</el-button>
      </div>
    </header>

    <div class="md-layout">
      <section class="md-workspace" aria-label="Markdown 编辑器">
        <div class="md-pane-title">
          <span>Markdown</span>
          <span>{{ lineCount }} 行 · {{ source.length }} 字符</span>
        </div>
        <div class="md-editor">
          <MonacoPane v-model="source" language="markdown" />
        </div>
      </section>

      <section class="md-workspace" aria-label="Markdown 预览">
        <div class="md-pane-title">
          <span>预览</span>
          <span>GFM · 安全渲染</span>
        </div>
        <article v-if="source.trim()" class="md-preview" v-html="renderedHtml"></article>
        <div v-else class="md-preview-empty">在左侧输入 Markdown，预览会实时更新</div>
      </section>
    </div>
  </section>
</template>

<script lang="ts">
const initialSource = `# Markdown 工作台

在左侧编辑，右侧实时预览。支持 **GFM** 表格、任务列表和代码高亮。

- [x] 安全渲染
- [ ] 打开一个 Markdown 文件

| 能力 | 状态 |
| --- | --- |
| 表格 | 可用 |
| 代码高亮 | 可用 |

\`\`\`ts
const message: string = "hello";
console.log(message);
\`\`\`
`;

const markdownState = {
  source: initialSource,
  currentPath: "",
  lastSavedSource: initialSource,
};
</script>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { renderMarkdown } from "../utils/renderMarkdown";
import { fileNameFromPath } from "../utils/textWorkbench";
import MonacoPane from "./MonacoPane.vue";

type ReadTextResponse = { content: string; path: string };

const source = ref(markdownState.source);
const currentPath = ref(markdownState.currentPath);
const lastSavedSource = ref(markdownState.lastSavedSource);
const opening = ref(false);
const saving = ref(false);

const renderedHtml = computed(() => renderMarkdown(source.value));
const lineCount = computed(() => (source.value ? source.value.split(/\r?\n/).length : 0));
const isDirty = computed(() => source.value !== lastSavedSource.value);
const currentFileName = computed(() =>
  currentPath.value ? fileNameFromPath(currentPath.value) : "未命名.md",
);

async function confirmDiscard(): Promise<boolean> {
  if (!isDirty.value) return true;
  try {
    await ElMessageBox.confirm("当前内容尚未保存，继续会丢失这些修改。", "放弃未保存修改？", {
      confirmButtonText: "继续",
      cancelButtonText: "取消",
      type: "warning",
    });
    return true;
  } catch {
    return false;
  }
}

async function openFile() {
  if (!(await confirmDiscard())) return;
  opening.value = true;
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Markdown", extensions: ["md", "markdown", "mdx", "txt"] }],
    });
    if (typeof selected !== "string") return;
    const result = (await invokeToolByChannel("tool:file:read-text", {
      path: selected,
    })) as ReadTextResponse;
    source.value = result.content;
    currentPath.value = result.path;
    lastSavedSource.value = result.content;
    ElMessage.success(`已打开 ${fileNameFromPath(result.path)}`);
  } catch (error) {
    ElMessage.error((error as Error).message || "打开文件失败");
  } finally {
    opening.value = false;
  }
}

async function saveFile(saveAs: boolean) {
  saving.value = true;
  try {
    let path = saveAs ? "" : currentPath.value;
    if (!path) {
      const selected = await save({
        defaultPath: currentFileName.value,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!selected) return;
      path = selected;
    }
    await invokeToolByChannel("tool:file:write-text", { path, content: source.value });
    currentPath.value = path;
    lastSavedSource.value = source.value;
    ElMessage.success(`已保存 ${fileNameFromPath(path)}`);
  } catch (error) {
    ElMessage.error((error as Error).message || "保存文件失败");
  } finally {
    saving.value = false;
  }
}

async function copyHtml() {
  try {
    await navigator.clipboard.writeText(renderedHtml.value);
    ElMessage.success("已复制渲染后的 HTML");
  } catch {
    ElMessage.error("复制失败，请检查剪贴板权限");
  }
}

async function clearSource() {
  if (!(await confirmDiscard())) return;
  source.value = "";
  currentPath.value = "";
  lastSavedSource.value = "";
}

function handleShortcut(event: KeyboardEvent) {
  if (!(event.ctrlKey || event.metaKey)) return;
  if (event.key.toLowerCase() === "o") {
    event.preventDefault();
    void openFile();
    return;
  }
  if (event.key.toLowerCase() === "s") {
    event.preventDefault();
    void saveFile(event.shiftKey);
  }
}

onMounted(() => window.addEventListener("keydown", handleShortcut));

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleShortcut);
  markdownState.source = source.value;
  markdownState.currentPath = currentPath.value;
  markdownState.lastSavedSource = lastSavedSource.value;
});
</script>

<style scoped>
.md-panel {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  gap: 10px;
}

.md-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 32px;
}

.md-file-meta,
.md-toolbar-actions,
.md-pane-title {
  display: flex;
  align-items: center;
}

.md-file-meta {
  min-width: 0;
  gap: 8px;
}

.md-file-name {
  overflow: hidden;
  color: var(--el-text-color-primary);
  font-size: 13px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.md-dirty {
  flex: none;
  padding: 2px 7px;
  border: 1px solid var(--el-color-warning-light-5);
  border-radius: 999px;
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning-dark-2);
  font-size: 11px;
}

.md-toolbar-actions {
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.md-toolbar-actions :deep(.el-button + .el-button) {
  margin-left: 0;
}

.md-toolbar-divider {
  width: 1px;
  height: 18px;
  margin: 0 2px;
  background: var(--lc-border);
}

.md-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  flex: 1;
  min-height: 0;
  gap: 12px;
}

.md-workspace {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md, 10px);
  background: var(--el-bg-color);
}

.md-pane-title {
  justify-content: space-between;
  flex: none;
  min-height: 34px;
  padding: 0 12px;
  border-bottom: 1px solid var(--lc-border);
  background: var(--el-fill-color-lighter);
  color: var(--el-text-color-secondary);
  font-size: 11px;
  letter-spacing: 0.02em;
}

.md-pane-title span:first-child {
  color: var(--el-text-color-primary);
  font-size: 12px;
  font-weight: 600;
}

.md-editor {
  flex: 1;
  min-height: 0;
}

.md-editor :deep(.monaco-pane) {
  border: 0;
  border-radius: 0;
}

.md-preview,
.md-preview-empty {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.md-preview {
  padding: 20px 24px 40px;
  color: var(--el-text-color-primary);
  font-size: 14px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}

.md-preview-empty {
  display: grid;
  place-items: center;
  padding: 24px;
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  text-align: center;
}

.md-preview :deep(h1),
.md-preview :deep(h2) {
  padding-bottom: 0.3em;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.md-preview :deep(h1) {
  margin: 0 0 0.75em;
  font-size: 1.8em;
}

.md-preview :deep(h2) {
  margin: 1.2em 0 0.55em;
  font-size: 1.4em;
}

.md-preview :deep(h3) {
  margin: 1em 0 0.5em;
  font-size: 1.15em;
}

.md-preview :deep(p),
.md-preview :deep(ul),
.md-preview :deep(ol),
.md-preview :deep(blockquote),
.md-preview :deep(pre),
.md-preview :deep(table) {
  margin: 0 0 1em;
}

.md-preview :deep(pre) {
  padding: 14px 16px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 7px;
  background: #f6f8fa;
  overflow: auto;
}

.md-preview :deep(code) {
  font-family: "Cascadia Code", "JetBrains Mono", Consolas, monospace;
  font-size: 0.9em;
}

.md-preview :deep(:not(pre) > code) {
  padding: 2px 5px;
  border-radius: 4px;
  background: var(--el-fill-color);
}

.md-preview :deep(ul),
.md-preview :deep(ol) {
  padding-left: 1.6em;
}

.md-preview :deep(.task-list-item) {
  list-style: none;
}

.md-preview :deep(.task-list-item input) {
  margin: 0 0.45em 0 -1.35em;
  accent-color: var(--el-color-primary);
}

.md-preview :deep(blockquote) {
  padding: 0.4em 1em;
  border-left: 3px solid var(--el-color-primary);
  color: var(--el-text-color-secondary);
}

.md-preview :deep(table) {
  display: block;
  width: max-content;
  max-width: 100%;
  border-collapse: collapse;
  overflow-x: auto;
}

.md-preview :deep(th),
.md-preview :deep(td) {
  padding: 7px 10px;
  border: 1px solid var(--el-border-color);
  text-align: left;
}

.md-preview :deep(th) {
  background: var(--el-fill-color-lighter);
  font-weight: 600;
}

.md-preview :deep(img) {
  max-width: 100%;
  height: auto;
}

.md-preview :deep(a) {
  color: var(--el-color-primary);
}

@media (max-width: 900px) {
  .md-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .md-toolbar-actions {
    justify-content: flex-start;
  }

  .md-layout {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(280px, 1fr) minmax(280px, 1fr);
    overflow-y: auto;
  }
}
</style>
