<template>
  <div class="diff-layout">
    <div class="diff-toolbar">
      <el-radio-group v-model="renderSideBySide" size="small">
        <el-radio-button :value="true">并排对比</el-radio-button>
        <el-radio-button :value="false">内联对比</el-radio-button>
      </el-radio-group>
      <el-button size="small" @click="swapContent">交换</el-button>
      <el-button size="small" @click="clearAll">清空</el-button>
    </div>
    <div ref="diffContainer" class="diff-editor-container"></div>
  </div>
</template>

<script lang="ts">
// 独立于组件实例的模块级状态，组件销毁重建时内容得以保留
const diffState = { original: "", modified: "" };
</script>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import monaco from "../utils/monaco-setup";

const diffContainer = ref<HTMLElement | null>(null);
const renderSideBySide = ref(true);
let diffEditor: monaco.editor.IStandaloneDiffEditor | null = null;

onMounted(() => {
  diffEditor = monaco.editor.createDiffEditor(diffContainer.value as HTMLElement, {
    theme: "vs",
    automaticLayout: true,
    renderSideBySide: renderSideBySide.value,
    minimap: { enabled: false },
    readOnly: false,
    originalEditable: true,
  });

  const originalModel = monaco.editor.createModel(diffState.original, "plaintext");
  const modifiedModel = monaco.editor.createModel(diffState.modified, "plaintext");
  diffEditor.setModel({ original: originalModel, modified: modifiedModel });
});

watch(renderSideBySide, (val) => {
  diffEditor?.updateOptions({ renderSideBySide: val });
});

function swapContent() {
  if (!diffEditor) return;
  const model = diffEditor.getModel();
  if (!model) return;
  const origVal = model.original.getValue();
  const modVal = model.modified.getValue();
  model.original.setValue(modVal);
  model.modified.setValue(origVal);
}

function clearAll() {
  if (!diffEditor) return;
  const model = diffEditor.getModel();
  model?.original.setValue("");
  model?.modified.setValue("");
}

onBeforeUnmount(() => {
  if (diffEditor) {
    const model = diffEditor.getModel();
    diffState.original = model?.original.getValue() ?? "";
    diffState.modified = model?.modified.getValue() ?? "";
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
