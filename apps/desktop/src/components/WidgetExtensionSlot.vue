<template>
  <section class="extension-slot" :style="inlineStyle">
    <div class="ext-inner">
      <span class="ext-text">{{ displayText }}</span>
      <button
        v-for="btn in buttons"
        :key="btn.key"
        class="ext-btn"
        :style="btnStyle"
        :title="btn.title"
        @click="$emit('action', btn.payload)"
      >{{ btn.label }}</button>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  echo?: string | null;
}>();

defineEmits<{
  (e: "action", payload: { kind: string; [key: string]: unknown }): void;
}>();

const displayText = computed(() => {
  if (props.echo && props.echo.trim().length > 0) return props.echo;
  return "快捷操作";
});

const buttons = [
  { key: "pm", label: "PM", title: "项目管理", payload: { kind: "open-tool", toolId: "pm" } },
  { key: "todo", label: "待办", title: "新建待办", payload: { kind: "open-todo-create" } },
  { key: "inbox", label: "Inbox", title: "收集箱", payload: { kind: "open-tool", toolId: "inbox" } },
];

const inlineStyle = computed(() => {
  // 硬编码颜色确保挂件扩展区始终可见，不依赖 CSS 变量
  return "background: rgba(255,255,255,0.12); border: 1px solid rgba(255,255,255,0.2);";
});

const btnStyle = "background: rgba(255,255,255,0.15); border: 1px solid rgba(255,255,255,0.25); color: #fff;";
</script>

<style scoped>
.extension-slot {
  min-height: 52px;
  border-radius: 12px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
}

.ext-inner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  width: 100%;
}

.ext-text {
  flex: 1;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.75);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ext-btn {
  padding: 5px 12px;
  border-radius: 6px;
  font-size: 12px;
  cursor: pointer;
  font-family: inherit;
  white-space: nowrap;
  transition: background-color 0.15s ease;
  flex-shrink: 0;
}

.ext-btn:hover {
  background: rgba(255, 255, 255, 0.3) !important;
}
</style>