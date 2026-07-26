<template>
  <div class="reference-card">
    <header class="card-toolbar">
      <span class="drag-grip" data-tauri-drag-region aria-hidden="true">•••</span>
      <span class="card-label" data-tauri-drag-region>置顶参考</span>
      <span class="toolbar-spacer" data-tauri-drag-region />
      <select v-model="language" class="language-select" aria-label="代码语言">
        <option v-for="option in MONACO_LANGUAGE_OPTIONS" :key="option" :value="option">
          {{ option }}
        </option>
      </select>
      <button
        type="button"
        class="toolbar-button"
        :class="{ active: wordWrap }"
        :aria-pressed="wordWrap"
        @click="wordWrap = !wordWrap"
      >
        自动换行
      </button>
      <button type="button" class="toolbar-button" @click="copyAll">复制全部</button>
      <button
        type="button"
        class="toolbar-button close-button"
        aria-label="关闭参考卡"
        @click="closeCard"
      >
        ×
      </button>
    </header>
    <div v-if="errorMessage" class="card-error" role="alert">{{ errorMessage }}</div>
    <MonacoPane
      ref="editorRef"
      v-model="content"
      class="card-editor"
      :language="language"
      :word-wrap="wordWrap"
      aria-label="置顶参考卡编辑器"
      @error="handleEditorError"
    />
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { APP_EVENTS } from "../bridge/events";
import { referenceCardReady, suppressClipboardCapture } from "../bridge/tauri";
import type { ReferenceCardInitPayload } from "../types/reference-card";
import { detectClipboardMonacoLanguage, MONACO_LANGUAGE_OPTIONS } from "../utils/monacoLanguages";
import MonacoPane from "./MonacoPane.vue";

interface MonacoPaneApi {
  focusEditor(): void;
}

const content = ref("");
const language = ref("plaintext");
const wordWrap = ref(true);
const errorMessage = ref("");
const editorRef = ref<MonacoPaneApi | null>(null);
let unlistenInit: UnlistenFn | null = null;

onMounted(async () => {
  window.addEventListener("keydown", onWindowKeydown, true);
  try {
    unlistenInit = await listen<ReferenceCardInitPayload>(
      APP_EVENTS.REFERENCE_CARD_INIT,
      async ({ payload }) => {
        content.value = payload.content;
        language.value = detectClipboardMonacoLanguage(payload.content);
        await nextTick();
        editorRef.value?.focusEditor();
      },
    );
    await referenceCardReady();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onWindowKeydown, true);
  unlistenInit?.();
});

async function copyAll() {
  try {
    await suppressClipboardCapture(content.value);
    await navigator.clipboard.writeText(content.value);
    errorMessage.value = "";
  } catch (error) {
    errorMessage.value = "复制失败：" + (error instanceof Error ? error.message : String(error));
  }
}

function onWindowKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  event.preventDefault();
  event.stopPropagation();
  void closeCard();
}

async function closeCard() {
  try {
    await getCurrentWindow().close();
  } catch (error) {
    errorMessage.value = "关闭失败：" + (error instanceof Error ? error.message : String(error));
  }
}

function handleEditorError(message: string) {
  errorMessage.value = message;
  console.error("[reference-card] " + message);
}
</script>

<style scoped>
.reference-card {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-sizing: border-box;
  color: #1f2937;
  background: #ffffff;
  border: 1px solid #d8dee8;
}

.card-toolbar {
  height: 38px;
  flex: 0 0 38px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  box-sizing: border-box;
  background: #f7f8fa;
  border-bottom: 1px solid #e4e7ed;
  user-select: none;
}

.drag-grip,
.card-label,
.toolbar-spacer {
  align-self: stretch;
  display: flex;
  align-items: center;
  cursor: move;
}

.drag-grip {
  color: #6b7280;
  letter-spacing: 1px;
}

.card-label {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
}

.toolbar-spacer {
  flex: 1 1 auto;
  min-width: 8px;
}

.language-select,
.toolbar-button {
  height: 26px;
  box-sizing: border-box;
  border: 1px solid #cfd5df;
  border-radius: 5px;
  color: #303133;
  background: #ffffff;
  font: inherit;
}

.language-select {
  max-width: 118px;
  padding: 0 6px;
}

.toolbar-button {
  padding: 0 8px;
  cursor: pointer;
}

.toolbar-button:hover,
.toolbar-button.active {
  color: #1d4ed8;
  border-color: #8aacf0;
  background: #eff6ff;
}

.language-select:focus-visible,
.toolbar-button:focus-visible {
  outline: 2px solid #2563eb;
  outline-offset: 1px;
}

.close-button {
  width: 28px;
  padding: 0;
  font-size: 18px;
}

.close-button:hover {
  color: #b42318;
  border-color: #e49a9a;
  background: #fff1f1;
}

.card-error {
  flex: 0 0 auto;
  padding: 5px 10px;
  color: #9f1c14;
  background: #fff1f0;
  border-bottom: 1px solid #ffccc7;
  font-size: 12px;
}

.card-editor {
  flex: 1 1 auto;
  min-height: 0;
  border: 0;
  border-radius: 0;
}

@media (max-width: 440px) {
  .card-label {
    display: none;
  }

  .language-select {
    max-width: 90px;
  }

  .toolbar-button {
    padding-inline: 6px;
  }
}
</style>
