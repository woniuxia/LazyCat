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
    wordWrap?: boolean;
  }>(),
  {
    language: "plaintext",
    readOnly: false,
    ariaLabel: "代码编辑器",
    wordWrap: false,
  },
);

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
  (event: "error", message: string): void;
}>();

const container = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let suppressEmit = false;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

onMounted(() => {
  try {
    editor = monaco.editor.create(container.value as HTMLElement, {
      value: props.modelValue,
      language: props.language,
      theme: "vs",
      readOnly: props.readOnly,
      ariaLabel: props.ariaLabel,
      automaticLayout: true,
      minimap: { enabled: false },
      wordWrap: props.wordWrap ? "on" : "off",
      // 编辑器滚动到边界后放行滚轮事件，避免吞掉外层容器的滚动
      scrollbar: { alwaysConsumeMouseWheel: false },
      guides: {
        indentation: true,
        bracketPairs: true,
      },
    });

    editor.onDidChangeModelContent(() => {
      if (suppressEmit || !editor) return;
      emit("update:modelValue", editor.getValue());
    });
  } catch (error) {
    emit("error", "Monaco 初始化失败：" + errorMessage(error));
  }
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

function focusEditor() {
  editor?.focus();
}

defineExpose({ formatDocument, focusLine, focusText, focusEditor });

watch(
  () => props.modelValue,
  (value) => {
    if (!editor) return;
    if (value === editor.getValue()) return;
    suppressEmit = true;
    editor.setValue(value);
    suppressEmit = false;
  },
);

watch(
  () => props.language,
  (language) => {
    if (!editor) return;
    const model = editor.getModel();
    if (!model) return;
    try {
      monaco.editor.setModelLanguage(model, language ?? "plaintext");
    } catch (error) {
      emit("error", "切换 Monaco 语言失败：" + errorMessage(error));
    }
  },
);

watch(
  () => props.wordWrap,
  (enabled) => editor?.updateOptions({ wordWrap: enabled ? "on" : "off" }),
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
