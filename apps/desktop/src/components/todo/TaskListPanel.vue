<template>
  <div class="task-list-panel">
    <TodoPanel>
      <template #view-switch>
        <TaskViewSwitch active-view="todo" :due-count="dueCount" @change="switchView" />
      </template>
    </TodoPanel>
  </div>
</template>

<script setup lang="ts">
import { useTabs } from "../../composables/useTabs";
import { useFollowUpDueCount } from "../../composables/useFollowUpDueCount";
import TodoPanel from "./TodoPanel.vue";
import TaskViewSwitch from "./TaskViewSwitch.vue";

const { openTab } = useTabs();
const { dueCount } = useFollowUpDueCount();
function switchView(view: "todo" | "follow-up") {
  if (view === "follow-up") openTab("follow-up", "关注事项");
}
</script>

<style scoped>
.task-list-panel { position: relative; height: 100%; min-height: 0; background: #fff; }
.task-list-panel > * { height: 100%; }
</style>