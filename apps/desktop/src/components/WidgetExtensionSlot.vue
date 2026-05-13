<template>
  <section class="extension-slot">
    <div class="ext-inner">
      <button
        v-for="btn in allButtons"
        :key="btn.key"
        class="ext-btn"
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
  todo: "新建待办",
  inbox: "收集箱",
};

const fixedButtons = computed<ExtensionButton[]>(() => {
  return (props.fixedToolIds ?? ["pm", "todo", "inbox"]).map((id) => ({
    key: id,
    label: LABEL_MAP[id] ?? id,
    title: TITLE_MAP[id] ?? id,
    payload:
      id === "todo"
        ? { kind: "open-todo-create" }
        : { kind: "open-tool", toolId: id },
  }));
});

const allButtons = computed(() => {
  const dynamic = (props.hotTools ?? [])
    .filter((t) => !fixedIds.value.has(t.id))
    .map((t) => ({
      key: `hot-${t.id}`,
      label: t.label,
      title: t.label,
      payload: { kind: "open-tool", toolId: t.id },
    }));
  return [...fixedButtons.value, ...dynamic];
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
  border-radius: 6px;
  font-size: 12px;
  cursor: pointer;
  font-family: inherit;
  white-space: nowrap;
  transition: background-color 0.15s ease;
  background: var(--wc-block-bg);
  border: 1px solid var(--wc-block-border);
  color: var(--wc-text);
}

.ext-btn:hover {
  background: var(--wc-block-border);
}
</style>
