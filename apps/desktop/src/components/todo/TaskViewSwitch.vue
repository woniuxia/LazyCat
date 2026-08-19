<template>
  <nav class="task-view-switch" aria-label="任务清单视图">
    <button
      type="button"
      :class="{ active: activeView === 'todo' }"
      :aria-pressed="activeView === 'todo'"
      @click="emit('change', 'todo')"
    >
      <span class="task-view-switch-label">我的任务</span>
    </button>
    <button
      type="button"
      :class="{ active: activeView === 'follow-up' }"
      :aria-pressed="activeView === 'follow-up'"
      @click="emit('change', 'follow-up')"
    >
      <span class="task-view-switch-label">关注事项</span>
      <span
        class="due-count"
        :class="{ 'is-empty': dueCount === 0 }"
        :aria-label="dueCount > 0 ? `${dueCount} 项待复查` : undefined"
        :aria-hidden="dueCount === 0"
      >
        {{ dueCount > 99 ? "99+" : dueCount || "0" }}
      </span>
    </button>
  </nav>
</template>

<script setup lang="ts">
defineProps<{
  activeView: "todo" | "follow-up";
  dueCount: number;
}>();

const emit = defineEmits<{
  change: [view: "todo" | "follow-up"];
}>();
</script>

<style scoped>
.task-view-switch {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 3px;
  padding: 3px;
  border: 1px solid var(--lc-border-subtle);
  border-radius: 8px;
  background: var(--lc-surface-2);
}
.task-view-switch button {
  position: relative;
  min-width: 0;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 0 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--lc-text-secondary);
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
  transition:
    color var(--lc-duration) var(--lc-ease),
    background-color var(--lc-duration) var(--lc-ease),
    box-shadow var(--lc-duration) var(--lc-ease);
}
.task-view-switch-label {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 1;
}
.task-view-switch button:hover {
  color: var(--lc-text);
}
.task-view-switch button.active {
  color: var(--lc-text);
  background: var(--lc-surface-0);
  box-shadow: var(--lc-shadow-sm);
}
.task-view-switch button:focus-visible {
  outline: 2px solid var(--lc-accent);
  outline-offset: 1px;
}
.due-count {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  width: 24px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  background: var(--lc-danger);
  color: #fff;
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  line-height: 1;
}
.due-count.is-empty {
  visibility: hidden;
}
</style>
