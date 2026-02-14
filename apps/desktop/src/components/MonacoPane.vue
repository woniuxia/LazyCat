<template>
  <div ref="container" class="monaco-pane"></div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import loader from "@monaco-editor/loader/lib/es/loader/index.js";
import type * as monaco from "monaco-editor";

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

onMounted(async () => {
  const monacoInstance = await loader.init();
  editor = monacoInstance.editor.create(container.value as HTMLElement, {
    value: props.modelValue,
    language: props.language,
    theme: "vs",
    readOnly: props.readOnly,
    automaticLayout: true,
    minimap: { enabled: false },
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
    loader.init().then((monacoInstance) => {
      monacoInstance.editor.setModelLanguage(model, language ?? "plaintext");
    });
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
  height: 360px;
  border: 1px solid #dce3ef;
  border-radius: 10px;
  overflow: hidden;
}
</style>
