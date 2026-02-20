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
let monacoModule: typeof monaco | null = null;
let suppressEmit = false;
let themeObserver: MutationObserver | null = null;

function currentMonacoTheme(): string {
  return document.documentElement.dataset.theme === "light" ? "vs" : "vs-dark";
}

onMounted(async () => {
  monacoModule = await loader.init();
  editor = monacoModule.editor.create(container.value as HTMLElement, {
    value: props.modelValue,
    language: props.language,
    theme: currentMonacoTheme(),
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

  // Watch for data-theme changes on <html> and switch Monaco theme
  themeObserver = new MutationObserver(() => {
    if (monacoModule) {
      monacoModule.editor.setTheme(currentMonacoTheme());
    }
  });
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
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
    if (!editor || !monacoModule) return;
    const model = editor.getModel();
    if (!model) return;
    monacoModule.editor.setModelLanguage(model, language ?? "plaintext");
  }
);

onBeforeUnmount(() => {
  if (themeObserver) {
    themeObserver.disconnect();
    themeObserver = null;
  }
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
  border: 1px solid var(--lc-border);
  border-radius: 10px;
  overflow: hidden;
}
</style>
