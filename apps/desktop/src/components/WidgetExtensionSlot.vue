<template>
  <section class="extension-slot">
    <div class="ext-inner">
      <button
        v-for="btn in fixedButtons"
        :key="btn.key"
        :class="['ext-btn', 'ext-btn--fixed', `ext-btn--${btn.key}`]"
        :title="btn.title"
        @click="$emit('action', btn.payload)"
      >{{ btn.label }}</button>
      <span v-if="dynamicButtons.length > 0" class="ext-sep" />
      <button
        v-for="btn in dynamicButtons"
        :key="btn.key"
        class="ext-btn ext-btn--hot"
        :title="btn.title"
        @click="$emit('action', btn.payload)"
      >{{ btn.label }}</button>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";

interface ExtensionButton {
  key: string;
  label: string;
  title: string;
  payload: { kind: string; [key: string]: unknown };
}

const props = defineProps<{
  hotTools?: { id: string; label: string }[];
  fixedToolIds?: string[];
}>();

defineEmits<{
  (e: "action", payload: { kind: string; [key: string]: unknown }): void;
}>();

const fixedIds = computed(() => new Set(props.fixedToolIds ?? ["pm", "todo", "inbox"]));

const LABEL_MAP: Record<string, string> = {
  pm: "PM",
  todo: "待办",
  inbox: "Inbox",
};

const TITLE_MAP: Record<string, string> = {
  pm: "项目管理",
  todo: "打开待办",
  inbox: "收集箱",
};

const fixedButtons = computed<ExtensionButton[]>(() => {
  return (props.fixedToolIds ?? ["pm", "todo", "inbox"]).map((id) => ({
    key: id,
    label: LABEL_MAP[id] ?? id,
    title: TITLE_MAP[id] ?? id,
    payload: { kind: "open-tool", toolId: id },
  }));
});

const dynamicButtons = computed(() => {
  return (props.hotTools ?? [])
    .filter((t) => !fixedIds.value.has(t.id))
    .map((t) => ({
      key: `hot-${t.id}`,
      label: t.label,
      title: t.label,
      payload: { kind: "open-tool", toolId: t.id },
    }));
});
</script>

<style scoped>
.extension-slot {
  min-height: 52px;
  border-radius: 12px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  background: var(--wc-block-bg);
  border: 1px solid var(--wc-block-border);
}

.ext-inner {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  width: 100%;
}

.ext-btn {
  padding: 5px 12px;
  border-radius: 14px;
  font-size: 12px;
  cursor: pointer;
  font-family: inherit;
  white-space: nowrap;
  transition: background-color 0.15s ease, transform 0.1s ease, opacity 0.15s ease;
  background: var(--wc-block-bg);
  border: 1px solid var(--wc-block-border);
  color: var(--wc-text);
}

.ext-btn:hover {
  background: var(--wc-block-border);
}

.ext-btn:active {
  transform: scale(0.96);
}

.ext-btn--fixed {
  font-weight: 500;
}

/* 固定按钮各自着色 */
.ext-btn--pm {
  color: var(--wc-accent);
  background: rgba(99, 102, 241, 0.08);
  border-color: rgba(99, 102, 241, 0.12);
}

.ext-btn--pm:hover {
  background: rgba(99, 102, 241, 0.14);
}

.ext-btn--todo {
  color: var(--wc-accent-purple);
  background: rgba(168, 85, 247, 0.08);
  border-color: rgba(168, 85, 247, 0.12);
}

.ext-btn--todo:hover {
  background: rgba(168, 85, 247, 0.14);
}

.ext-btn--inbox {
  color: var(--wc-accent-teal);
  background: rgba(20, 184, 166, 0.08);
  border-color: rgba(20, 184, 166, 0.12);
}

.ext-btn--inbox:hover {
  background: rgba(20, 184, 166, 0.14);
}

.ext-btn--hot {
  opacity: 0.7;
  font-weight: 400;
}

.ext-btn--hot:hover {
  opacity: 1;
  color: var(--wc-accent);
}

.ext-sep {
  width: 1px;
  height: 16px;
  background: var(--wc-divider);
  flex-shrink: 0;
  margin: 0 2px;
}
</style>
