<template>
  <div class="task-list-panel">
    <TodoPanel v-show="activeView === 'todo'">
      <template #view-switch>
        <TaskViewSwitch
          :active-view="activeView"
          :due-count="followUpDueCount"
          @change="activeView = $event"
        />
      </template>
    </TodoPanel>
    <FollowUpPanel
      v-show="activeView === 'follow-up'"
      ref="followUpRef"
      @create-todo="createTodoFromFollowUp"
      @due-count-change="followUpDueCount = $event"
    >
      <template #view-switch>
        <TaskViewSwitch
          :active-view="activeView"
          :due-count="followUpDueCount"
          @change="activeView = $event"
        />
      </template>
    </FollowUpPanel>
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
import TaskViewSwitch from "./TaskViewSwitch.vue";

const activeView = ref<"todo" | "follow-up">("todo");
const followUpDueCount = ref(0);
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
  position: relative;
  height: 100%;
  min-height: 0;
  background: #fff;
}
.task-list-panel > * {
  height: 100%;
}
</style>
