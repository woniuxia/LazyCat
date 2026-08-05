<template>
  <div class="workbench-panel json-workbench-panel">
    <el-tabs v-model="activeTab" class="workbench-tabs" aria-label="JSON 工作台功能">
      <el-tab-pane label="处理与转换" name="process">
        <JsonProcessPanel ref="processPanelRef" />
      </el-tab-pane>
      <el-tab-pane label="JSON Schema" name="schema" lazy>
        <JsonSchemaPanel />
      </el-tab-pane>
      <el-tab-pane label="数组过滤" name="array-filter" lazy>
        <JsonArrayFilterPanel />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useClipboardSuggestion } from "../composables/useClipboardSuggestion";
import JsonArrayFilterPanel from "./JsonArrayFilterPanel.vue";
import JsonProcessPanel from "./JsonProcessPanel.vue";
import JsonSchemaPanel from "./JsonSchemaPanel.vue";
import { workbenchTabState, type JsonWorkbenchTab } from "./workbenchTabState";

interface JsonProcessPanelApi {
  applyExternalInput(text: string): void;
}

const activeTab = ref<JsonWorkbenchTab>(workbenchTabState.json);
const processPanelRef = ref<JsonProcessPanelApi | null>(null);

watch(
  activeTab,
  (tab) => {
    workbenchTabState.json = tab;
  },
  { flush: "sync" },
);

const { watchPendingInput } = useClipboardSuggestion();
watchPendingInput("json-workbench", async (text) => {
  activeTab.value = "process";
  await nextTick();
  processPanelRef.value?.applyExternalInput(text);
});
</script>

<style scoped>
.workbench-panel,
.workbench-tabs {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.workbench-tabs :deep(.el-tabs__header) {
  flex-shrink: 0;
  margin-bottom: 12px;
}

.workbench-tabs :deep(.el-tabs__content),
.workbench-tabs :deep(.el-tab-pane) {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.workbench-tabs :deep(.el-tabs__content) {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.workbench-tabs :deep(.el-tab-pane) {
  display: flex;
  flex-direction: column;
}
</style>
