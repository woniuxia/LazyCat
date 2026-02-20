<template>
  <div class="diff-layout">
    <div class="diff-toolbar">
      <el-radio-group v-model="renderSideBySide" size="small">
        <el-radio-button :value="true">并排对比</el-radio-button>
        <el-radio-button :value="false">内联对比</el-radio-button>
      </el-radio-group>
      <el-button size="small" @click="clearAll">清空</el-button>
    </div>
    <div ref="diffContainer" class="diff-editor-container"></div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import loader from "@monaco-editor/loader/lib/es/loader/index.js";
import type * as monaco from "monaco-editor";

const diffContainer = ref<HTMLElement | null>(null);
const renderSideBySide = ref(true);
let diffEditor: monaco.editor.IStandaloneDiffEditor | null = null;
let monacoModule: typeof monaco | null = null;
let themeObserver: MutationObserver | null = null;

function currentMonacoTheme(): string {
  return document.documentElement.dataset.theme === "light" ? "vs" : "vs-dark";
}

onMounted(async () => {
  monacoModule = await loader.init();
  diffEditor = monacoModule.editor.createDiffEditor(diffContainer.value as HTMLElement, {
    theme: currentMonacoTheme(),
    automaticLayout: true,
    renderSideBySide: renderSideBySide.value,
    minimap: { enabled: false },
    readOnly: false,
    originalEditable: true,
  });

  const originalModel = monacoModule.editor.createModel("", "plaintext");
  const modifiedModel = monacoModule.editor.createModel("", "plaintext");
  diffEditor.setModel({ original: originalModel, modified: modifiedModel });

  themeObserver = new MutationObserver(() => {
    if (monacoModule) monacoModule.editor.setTheme(currentMonacoTheme());
  });
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });
});

watch(renderSideBySide, (val) => {
  diffEditor?.updateOptions({ renderSideBySide: val });
});

function clearAll() {
  if (!diffEditor) return;
  const model = diffEditor.getModel();
  model?.original.setValue("");
  model?.modified.setValue("");
}

onBeforeUnmount(() => {
  if (themeObserver) {
    themeObserver.disconnect();
    themeObserver = null;
  }
  if (diffEditor) {
    const model = diffEditor.getModel();
    model?.original.dispose();
    model?.modified.dispose();
    diffEditor.dispose();
    diffEditor = null;
  }
});
</script>

<style scoped>
.diff-layout {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  gap: 12px;
}

.diff-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.diff-editor-container {
  flex: 1;
  min-height: 200px;
  border: 1px solid var(--lc-border);
  border-radius: 10px;
  overflow: hidden;
}
</style>
