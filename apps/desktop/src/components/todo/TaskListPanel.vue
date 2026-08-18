<template>
  <div class="task-list-panel">
    <header class="task-view-switch">
      <el-segmented v-model="activeView" :options="viewOptions" />
    </header>
    <div class="task-view-body">
      <TodoPanel v-show="activeView === 'todo'" />
      <FollowUpPanel
        ref="followUpRef"
        v-show="activeView === 'follow-up'"
        @create-todo="createTodoFromFollowUp"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { useNavigationHandoff } from "../../composables/useNavigationHandoff";
import { useTodoNavigation } from "../../composables/useTodoNavigation";
import type { FollowUpItem } from "../../types/follow-up";
import { buildFollowUpTodoInput } from "../../utils/followUp";
import FollowUpPanel from "../follow-up/FollowUpPanel.vue";
import TodoPanel from "./TodoPanel.vue";

const activeView = ref<"todo" | "follow-up">("todo");
const viewOptions = [
  { label: "我的任务", value: "todo" },
  { label: "关注事项", value: "follow-up" },
];
const followUpRef = ref<InstanceType<typeof FollowUpPanel> | null>(null);
const { pendingFollowUpFocus, consumeFollowUpFocus } = useTodoNavigation();
const navigationHandoff = useNavigationHandoff();

async function applyFollowUpFocus() {
  const request = consumeFollowUpFocus();
  if (!request) return;
  activeView.value = "follow-up";
  await nextTick();
  await followUpRef.value?.focus(request.itemId, request.dueOnly);
}
function createTodoFromFollowUp(item: FollowUpItem) {
  navigationHandoff.setPendingToolInput(buildFollowUpTodoInput(item));
  activeView.value = "todo";
}
watch(pendingFollowUpFocus, () => void applyFollowUpFocus());
onMounted(() => void applyFollowUpFocus());
</script>

<style scoped>
.task-list-panel {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: #fff;
}
.task-view-switch {
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-bottom: 1px solid #e5e8ec;
  background: #fff;
  flex: 0 0 auto;
}
.task-view-body {
  position: relative;
  min-height: 0;
  flex: 1;
}
.task-view-body > * {
  height: 100%;
}
</style>
