<template>
  <div class="workbench-panel data-convert-panel">
    <el-tabs v-model="activeTab" class="workbench-tabs" aria-label="数据格式转换功能">
      <el-tab-pane label="CSV → JSON" name="csv">
        <CsvJsonPanel />
      </el-tab-pane>
      <el-tab-pane label="JavaBean / JSON / JS" name="java-bean" lazy>
        <JavaBeanJsPanel />
      </el-tab-pane>
      <el-tab-pane label="配置文件互转" name="config" lazy>
        <ConfigConvertPanel />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import ConfigConvertPanel from "./ConfigConvertPanel.vue";
import CsvJsonPanel from "./CsvJsonPanel.vue";
import JavaBeanJsPanel from "./JavaBeanJsPanel.vue";
import { workbenchTabState, type DataConvertTab } from "./workbenchTabState";

const activeTab = ref<DataConvertTab>(workbenchTabState.dataConvert);

watch(
  activeTab,
  (tab) => {
    workbenchTabState.dataConvert = tab;
  },
  { flush: "sync" },
);
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
