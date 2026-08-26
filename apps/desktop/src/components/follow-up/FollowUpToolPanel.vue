<template>
  <div class="follow-up-tool-panel">
    <FollowUpPanel
      ref="followUpRef"
      @create-todo="createTodoFromFollowUp"
      @due-count-change="dueCount = $event"
    >
      <template #view-switch>
        <TaskViewSwitch active-view="follow-up" :due-count="dueCount" @change="switchView" />
      </template>
    </FollowUpPanel>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { useNavigationHandoff } from "../../composables/useNavigationHandoff";
import { useFollowUpDueCount } from "../../composables/useFollowUpDueCount";
import { useTabs } from "../../composables/useTabs";
import { useTodoNavigation } from "../../composables/useTodoNavigation";
import type { FollowUpItem } from "../../types/follow-up";
import { buildFollowUpTodoInput } from "../../utils/followUp";
import FollowUpPanel from "./FollowUpPanel.vue";
import TaskViewSwitch from "../todo/TaskViewSwitch.vue";

const { openTab } = useTabs();
const { dueCount } = useFollowUpDueCount();
const followUpRef = ref<InstanceType<typeof FollowUpPanel> | null>(null);
const { pendingFollowUpFocus, consumeFollowUpFocus } = useTodoNavigation();
const navigationHandoff = useNavigationHandoff();

async function applyFocus() {
  const request = consumeFollowUpFocus();
  if (!request) return;
  await nextTick();
  await followUpRef.value?.focus(request.itemId, request.dueOnly);
}
function switchView(view: "todo" | "follow-up") {
  if (view === "todo") openTab("todo", "任务清单");
}
function createTodoFromFollowUp(item: FollowUpItem) {
  navigationHandoff.setPendingToolInput(buildFollowUpTodoInput(item));
  openTab("todo", "任务清单");
}
watch(pendingFollowUpFocus, () => void applyFocus());
onMounted(() => void applyFocus());
</script>

<style scoped>
.follow-up-tool-panel { position: relative; height: 100%; min-height: 0; background: #fff; }
.follow-up-tool-panel > * { height: 100%; }
</style>