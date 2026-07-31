<template>
  <section class="diff-layout" aria-label="文本对比工作台">
    <header class="diff-toolbar">
      <div class="diff-toolbar-group">
        <el-radio-group v-model="renderSideBySide" size="small" aria-label="对比布局">
          <el-radio-button :value="true">并排</el-radio-button>
          <el-radio-button :value="false">内联</el-radio-button>
        </el-radio-group>
        <el-select
          v-model="languageMode"
          size="small"
          class="language-select"
          aria-label="文本语言"
        >
          <el-option
            v-for="option in languageOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-checkbox v-model="ignoreTrimWhitespace" size="small">忽略首尾空白</el-checkbox>
      </div>

      <div class="diff-toolbar-group diff-toolbar-actions">
        <el-button size="small" @click="swapContent">交换</el-button>
        <el-button size="small" :disabled="!modifiedContent" @click="copyResult"
          >复制右侧</el-button
        >
        <el-button size="small" :loading="saving" :disabled="!modifiedContent" @click="saveResult"
          >导出右侧</el-button
        >
        <el-button size="small" :disabled="!originalContent && !modifiedContent" @click="clearAll"
          >清空</el-button
        >
      </div>
    </header>

    <div class="diff-filebar">
      <div class="diff-file" :title="originalPath || '尚未关联文件'">
        <span class="diff-file-side">原始</span>
        <span class="diff-file-name">{{ originalFileName }}</span>
        <el-button
          size="small"
          link
          :loading="openingSide === 'original'"
          @click="openFile('original')"
          >打开文件</el-button
        >
      </div>
      <button
        class="diff-swap-button"
        type="button"
        title="交换两侧内容"
        aria-label="交换两侧内容"
        @click="swapContent"
      >
        ⇄
      </button>
      <div class="diff-file" :title="modifiedPath || '尚未关联文件'">
        <span class="diff-file-side is-modified">修改后</span>
        <span class="diff-file-name">{{ modifiedFileName }}</span>
        <el-button
          size="small"
          link
          :loading="openingSide === 'modified'"
          @click="openFile('modified')"
          >打开文件</el-button
        >
      </div>
    </div>

    <div class="diff-summary" role="status" aria-live="polite">
      <span>{{ effectiveLanguageLabel }}</span>
      <span class="summary-divider" aria-hidden="true"></span>
      <span>{{ summary.hunks }} 处差异</span>
      <span class="summary-added">+{{ summary.addedLines }}</span>
      <span class="summary-removed">−{{ summary.removedLines }}</span>
      <span>~{{ summary.changedLines }}</span>
      <div class="diff-navigation">
        <el-button size="small" :disabled="summary.hunks === 0" @click="navigateHunk(-1)"
          >上一处</el-button
        >
        <span>{{ summary.hunks ? activeHunk + 1 : 0 }} / {{ summary.hunks }}</span>
        <el-button size="small" :disabled="summary.hunks === 0" @click="navigateHunk(1)"
          >下一处</el-button
        >
      </div>
    </div>

    <div ref="diffContainer" class="diff-editor-container"></div>
  </section>
</template>

<script lang="ts">
const diffState = {
  original: "",
  modified: "",
  originalPath: "",
  modifiedPath: "",
  languageMode: "auto",
  renderSideBySide: true,
  ignoreTrimWhitespace: false,
};
</script>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import monaco from "../utils/monaco-setup";
import {
  detectMonacoLanguage,
  fileNameFromPath,
  summarizeDiff,
  type DiffSummary,
} from "../utils/textWorkbench";

type Side = "original" | "modified";
type ReadTextResponse = { content: string; path: string };

const languageOptions = [
  { label: "自动识别", value: "auto" },
  { label: "纯文本", value: "plaintext" },
  { label: "JSON", value: "json" },
  { label: "JavaScript", value: "javascript" },
  { label: "TypeScript", value: "typescript" },
  { label: "HTML / Vue", value: "html" },
  { label: "CSS", value: "css" },
  { label: "Markdown", value: "markdown" },
  { label: "SQL", value: "sql" },
  { label: "Java", value: "java" },
  { label: "Python", value: "python" },
  { label: "Rust", value: "rust" },
  { label: "YAML", value: "yaml" },
  { label: "XML", value: "xml" },
  { label: "Shell", value: "shell" },
  { label: "PowerShell", value: "powershell" },
];

const diffContainer = ref<HTMLElement | null>(null);
const renderSideBySide = ref(diffState.renderSideBySide);
const languageMode = ref(diffState.languageMode);
const ignoreTrimWhitespace = ref(diffState.ignoreTrimWhitespace);
const originalPath = ref(diffState.originalPath);
const modifiedPath = ref(diffState.modifiedPath);
const originalContent = ref(diffState.original);
const modifiedContent = ref(diffState.modified);
const openingSide = ref<Side | "">("");
const saving = ref(false);
const summary = ref<DiffSummary>(summarizeDiff(null));
const activeHunk = ref(-1);

let diffEditor: monaco.editor.IStandaloneDiffEditor | null = null;
let originalModel: monaco.editor.ITextModel | null = null;
let modifiedModel: monaco.editor.ITextModel | null = null;
let originalChangeDisposable: monaco.IDisposable | null = null;
let modifiedChangeDisposable: monaco.IDisposable | null = null;
let diffUpdateDisposable: monaco.IDisposable | null = null;

const originalFileName = computed(() =>
  originalPath.value ? fileNameFromPath(originalPath.value) : "未命名",
);
const modifiedFileName = computed(() =>
  modifiedPath.value ? fileNameFromPath(modifiedPath.value) : "未命名",
);
const effectiveLanguage = computed(() => {
  if (languageMode.value !== "auto") return languageMode.value;
  return detectMonacoLanguage(modifiedPath.value || originalPath.value);
});
const effectiveLanguageLabel = computed(() => {
  const option = languageOptions.find((item) => item.value === effectiveLanguage.value);
  return option?.label ?? effectiveLanguage.value;
});

onMounted(() => {
  diffEditor = monaco.editor.createDiffEditor(diffContainer.value as HTMLElement, {
    theme: "vs",
    automaticLayout: true,
    renderSideBySide: renderSideBySide.value,
    ignoreTrimWhitespace: ignoreTrimWhitespace.value,
    diffAlgorithm: "advanced",
    minimap: { enabled: false },
    readOnly: false,
    originalEditable: true,
    scrollbar: { alwaysConsumeMouseWheel: false },
  });

  originalModel = monaco.editor.createModel(originalContent.value, effectiveLanguage.value);
  modifiedModel = monaco.editor.createModel(modifiedContent.value, effectiveLanguage.value);
  diffEditor.setModel({ original: originalModel, modified: modifiedModel });
  originalChangeDisposable = originalModel.onDidChangeContent(() => {
    originalContent.value = originalModel?.getValue() ?? "";
  });
  modifiedChangeDisposable = modifiedModel.onDidChangeContent(() => {
    modifiedContent.value = modifiedModel?.getValue() ?? "";
  });
  diffUpdateDisposable = diffEditor.onDidUpdateDiff(updateSummary);
  updateSummary();
});

watch([renderSideBySide, ignoreTrimWhitespace], ([sideBySide, ignoreWhitespace]) => {
  diffEditor?.updateOptions({
    renderSideBySide: sideBySide,
    ignoreTrimWhitespace: ignoreWhitespace,
  });
});

watch(effectiveLanguage, (language) => {
  if (originalModel) monaco.editor.setModelLanguage(originalModel, language);
  if (modifiedModel) monaco.editor.setModelLanguage(modifiedModel, language);
});

function updateSummary() {
  const changes = diffEditor?.getLineChanges() ?? null;
  summary.value = summarizeDiff(changes);
  if (summary.value.hunks === 0) activeHunk.value = -1;
  else if (activeHunk.value >= summary.value.hunks) activeHunk.value = summary.value.hunks - 1;
}

async function openFile(side: Side) {
  const currentContent = side === "original" ? originalContent.value : modifiedContent.value;
  if (currentContent) {
    try {
      await ElMessageBox.confirm(
        `打开文件会替换${side === "original" ? "左" : "右"}侧当前内容。`,
        "替换当前内容？",
        {
          confirmButtonText: "继续",
          cancelButtonText: "取消",
          type: "warning",
        },
      );
    } catch {
      return;
    }
  }

  openingSide.value = side;
  try {
    const selected = await open({ multiple: false });
    if (typeof selected !== "string") return;
    const result = (await invokeToolByChannel("tool:file:read-text", {
      path: selected,
    })) as ReadTextResponse;
    if (side === "original") {
      originalPath.value = result.path;
      originalModel?.setValue(result.content);
    } else {
      modifiedPath.value = result.path;
      modifiedModel?.setValue(result.content);
    }
    ElMessage.success(`已载入 ${fileNameFromPath(result.path)}`);
  } catch (error) {
    ElMessage.error((error as Error).message || "打开文件失败");
  } finally {
    openingSide.value = "";
  }
}

function swapContent() {
  if (!originalModel || !modifiedModel) return;
  const originalValue = originalModel.getValue();
  const originalFilePath = originalPath.value;
  originalModel.setValue(modifiedModel.getValue());
  modifiedModel.setValue(originalValue);
  originalPath.value = modifiedPath.value;
  modifiedPath.value = originalFilePath;
}

async function clearAll() {
  try {
    await ElMessageBox.confirm("将清空两侧内容和文件关联。", "清空对比？", {
      confirmButtonText: "清空",
      cancelButtonText: "取消",
      type: "warning",
    });
  } catch {
    return;
  }
  originalModel?.setValue("");
  modifiedModel?.setValue("");
  originalPath.value = "";
  modifiedPath.value = "";
}

function navigateHunk(direction: -1 | 1) {
  const changes = diffEditor?.getLineChanges();
  if (!changes?.length || !diffEditor) return;
  activeHunk.value =
    activeHunk.value < 0
      ? direction === 1
        ? 0
        : changes.length - 1
      : (activeHunk.value + direction + changes.length) % changes.length;
  const change = changes[activeHunk.value];
  const modifiedLine =
    change.modifiedEndLineNumber === 0
      ? Math.max(1, change.modifiedStartLineNumber)
      : change.modifiedStartLineNumber;
  const originalLine =
    change.originalEndLineNumber === 0
      ? Math.max(1, change.originalStartLineNumber)
      : change.originalStartLineNumber;
  diffEditor.getModifiedEditor().revealLineInCenter(modifiedLine);
  diffEditor.getModifiedEditor().setPosition({ lineNumber: modifiedLine, column: 1 });
  diffEditor.getOriginalEditor().revealLineInCenter(originalLine);
  diffEditor.getModifiedEditor().focus();
}

async function copyResult() {
  try {
    await navigator.clipboard.writeText(modifiedContent.value);
    ElMessage.success("已复制右侧结果");
  } catch {
    ElMessage.error("复制失败，请检查剪贴板权限");
  }
}

async function saveResult() {
  saving.value = true;
  try {
    const sourceName = modifiedPath.value ? fileNameFromPath(modifiedPath.value) : "result.txt";
    const path = await save({ defaultPath: `diff-${sourceName}` });
    if (!path) return;
    await invokeToolByChannel("tool:file:write-text", { path, content: modifiedContent.value });
    ElMessage.success(`已导出 ${fileNameFromPath(path)}`);
  } catch (error) {
    ElMessage.error((error as Error).message || "导出失败");
  } finally {
    saving.value = false;
  }
}

onBeforeUnmount(() => {
  diffState.original = originalModel?.getValue() ?? originalContent.value;
  diffState.modified = modifiedModel?.getValue() ?? modifiedContent.value;
  diffState.originalPath = originalPath.value;
  diffState.modifiedPath = modifiedPath.value;
  diffState.languageMode = languageMode.value;
  diffState.renderSideBySide = renderSideBySide.value;
  diffState.ignoreTrimWhitespace = ignoreTrimWhitespace.value;
  originalChangeDisposable?.dispose();
  modifiedChangeDisposable?.dispose();
  diffUpdateDisposable?.dispose();
  diffEditor?.dispose();
  originalModel?.dispose();
  modifiedModel?.dispose();
  diffEditor = null;
  originalModel = null;
  modifiedModel = null;
});
</script>

<style scoped>
.diff-layout {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  gap: 9px;
}

.diff-toolbar,
.diff-toolbar-group,
.diff-filebar,
.diff-file,
.diff-summary,
.diff-navigation {
  display: flex;
  align-items: center;
}

.diff-toolbar {
  justify-content: space-between;
  gap: 12px;
}

.diff-toolbar-group {
  flex-wrap: wrap;
  gap: 8px;
}

.diff-toolbar-actions {
  justify-content: flex-end;
}

.diff-toolbar-actions :deep(.el-button + .el-button) {
  margin-left: 0;
}

.language-select {
  width: 132px;
}

.diff-filebar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 28px minmax(0, 1fr);
  gap: 8px;
}

.diff-file {
  min-width: 0;
  min-height: 34px;
  padding: 0 8px 0 10px;
  border: 1px solid var(--lc-border);
  border-radius: 7px;
  background: var(--el-fill-color-lighter);
  gap: 8px;
}

.diff-file-side {
  flex: none;
  color: var(--el-color-danger-dark-2);
  font-size: 11px;
  font-weight: 600;
}

.diff-file-side.is-modified {
  color: var(--el-color-success-dark-2);
}

.diff-file-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--el-text-color-primary);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.diff-swap-button {
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid var(--lc-border);
  border-radius: 6px;
  background: var(--el-bg-color);
  color: var(--el-text-color-secondary);
  cursor: pointer;
  font-size: 16px;
  transition:
    border-color 0.2s,
    color 0.2s,
    background-color 0.2s;
}

.diff-swap-button:hover,
.diff-swap-button:focus-visible {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
  outline: none;
}

.diff-summary {
  min-height: 28px;
  padding: 0 4px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  gap: 10px;
}

.summary-divider {
  width: 1px;
  height: 14px;
  background: var(--lc-border);
}

.summary-added {
  color: var(--el-color-success-dark-2);
  font-weight: 600;
}

.summary-removed {
  color: var(--el-color-danger-dark-2);
  font-weight: 600;
}

.diff-navigation {
  margin-left: auto;
  gap: 8px;
}

.diff-navigation :deep(.el-button + .el-button) {
  margin-left: 0;
}

.diff-editor-container {
  flex: 1;
  min-height: 220px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md, 10px);
  overflow: hidden;
}

@media (max-width: 900px) {
  .diff-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .diff-toolbar-actions {
    justify-content: flex-start;
  }
}

@media (max-width: 640px) {
  .diff-filebar {
    grid-template-columns: 1fr;
  }

  .diff-swap-button {
    justify-self: center;
    transform: rotate(90deg);
  }

  .diff-summary {
    flex-wrap: wrap;
  }

  .diff-navigation {
    margin-left: 0;
  }
}
</style>
