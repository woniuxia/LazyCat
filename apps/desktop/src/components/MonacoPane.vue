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
  }>(),
  {
    language: "plaintext",
    readOnly: false
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
