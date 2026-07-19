<template>
  <div ref="container" class="monaco-pane"></div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import monaco from "../utils/monaco-setup";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    language?: string;
    readOnly?: boolean;
    ariaLabel?: string;
  }>(),
  {
    language: "plaintext",
    readOnly: false,
    ariaLabel: "代码编辑器"
  }
);

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
}>();

const container = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let suppressEmit = false;

onMounted(() => {
  editor = monaco.editor.create(container.value as HTMLElement, {
    value: props.modelValue,
    language: props.language,
    theme: "vs",
    readOnly: props.readOnly,
    ariaLabel: props.ariaLabel,
    automaticLayout: true,
    minimap: { enabled: false },
    // 编辑器滚动到边界后放行滚轮事件，避免吞掉外层容器的滚动
    scrollbar: { alwaysConsumeMouseWheel: false },
    guides: {
      indentation: true,
      bracketPairs: true
    }
  });

  editor.onDidChangeModelContent(() => {
    if (suppressEmit || !editor) return;
    emit("update:modelValue", editor.getValue());
  });
});

async function formatDocument() {
  await editor?.getAction("editor.action.formatDocument")?.run();
}

function focusLine(line: number, column = 1) {
  if (!editor) return;
  const model = editor.getModel();
  if (!model) return;
  const safeLine = Math.min(Math.max(1, line), model.getLineCount());
  const safeColumn = Math.min(Math.max(1, column), model.getLineMaxColumn(safeLine));
  editor.setPosition({ lineNumber: safeLine, column: safeColumn });
  editor.revealLineInCenter(safeLine);
  editor.focus();
}

function focusText(text: string) {
  if (!editor || !text) return false;
  const model = editor.getModel();
  if (!model) return false;
  const search = JSON.stringify(text);
  const match = model.findMatches(search, false, true, null, false, 1)[0];
  if (!match) return false;
  editor.setSelection(match.range);
  editor.revealRangeInCenter(match.range);
  editor.focus();
  return true;
}

defineExpose({ formatDocument, focusLine, focusText });

watch(
  () => props.modelValue,
  (value) => {
    if (!editor) return;
    if (value === editor.getValue()) return;
    suppressEmit = true;
    editor.setValue(value);
    suppressEmit = false;
  }
);

watch(
  () => props.language,
  (language) => {
    if (!editor) return;
    const model = editor.getModel();
    if (!model) return;
    monaco.editor.setModelLanguage(model, language ?? "plaintext");
  }
);

onBeforeUnmount(() => {
  if (editor) {
    editor.dispose();
    editor = null;
  }
});
</script>

<style scoped>
.monaco-pane {
  width: 100%;
  height: 100%;
  min-height: 200px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md, 10px);
  overflow: hidden;
}
</style>
