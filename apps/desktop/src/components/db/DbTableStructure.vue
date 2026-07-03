<template>
  <div v-loading="loading" class="db-table-structure">
    <el-tabs v-model="activeTab" class="structure-tabs">
      <el-tab-pane label="字段" name="columns">
        <el-table :data="detail?.columns ?? []" size="small" border height="100%">
          <el-table-column type="index" width="46" />
          <el-table-column prop="name" label="字段名" min-width="140" show-overflow-tooltip>
            <template #default="{ row }">
              <span class="col-name">
                {{ row.name }}
                <el-tag v-if="row.primaryKey" type="warning" size="small" effect="plain">PK</el-tag>
              </span>
            </template>
          </el-table-column>
          <el-table-column prop="dataType" label="类型" min-width="130" show-overflow-tooltip />
          <el-table-column label="可空" width="60" align="center">
            <template #default="{ row }">{{ row.nullable ? "是" : "否" }}</template>
          </el-table-column>
          <el-table-column prop="defaultValue" label="默认值" min-width="110" show-overflow-tooltip>
            <template #default="{ row }">
              <span v-if="row.defaultValue === null" class="null-text">NULL</span>
              <span v-else>{{ row.defaultValue }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="comment" label="注释" min-width="160" show-overflow-tooltip />
        </el-table>
      </el-tab-pane>
      <el-tab-pane label="索引" name="indexes">
        <el-table :data="detail?.indexes ?? []" size="small" border height="100%">
          <el-table-column prop="name" label="索引名" min-width="150" show-overflow-tooltip />
          <el-table-column label="唯一" width="60" align="center">
            <template #default="{ row }">{{ row.unique ? "是" : "否" }}</template>
          </el-table-column>
          <el-table-column label="列 / 定义" min-width="260" show-overflow-tooltip>
            <template #default="{ row }">
              {{ row.columns.length > 0 ? row.columns.join(", ") : row.definition }}
            </template>
          </el-table-column>
        </el-table>
      </el-tab-pane>
      <el-tab-pane label="DDL" name="ddl">
        <div class="ddl-pane">
          <div class="ddl-toolbar">
            <el-button size="small" @click="copyDdl">复制 DDL</el-button>
          </div>
          <pre class="ddl-content">{{ detail?.ddl || "（无 DDL）" }}</pre>
        </div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../../bridge/tauri";
import type { DbTableDetail } from "../../types/db";

const props = defineProps<{
  connectionId: string;
  database: string;
  table: string;
}>();

const detail = ref<DbTableDetail | null>(null);
const loading = ref(false);
const activeTab = ref("columns");
let requestSeq = 0;

async function load(): Promise<void> {
  const seq = ++requestSeq;
  loading.value = true;
  try {
    const data = (await invokeToolByChannel("tool:db:schema-table-detail", {
      connectionId: props.connectionId,
      database: props.database,
      table: props.table,
    })) as DbTableDetail;
    if (seq === requestSeq) {
      detail.value = data;
    }
  } catch (error) {
    if (seq === requestSeq) {
      ElMessage.error((error as Error).message);
    }
  } finally {
    if (seq === requestSeq) {
      loading.value = false;
    }
  }
}

async function copyDdl(): Promise<void> {
  if (!detail.value?.ddl) return;
  await navigator.clipboard.writeText(detail.value.ddl);
  ElMessage.success("DDL 已复制");
}

onMounted(load);
watch(() => [props.connectionId, props.database, props.table], load);

defineExpose({ reload: load });
</script>

<style scoped>
.db-table-structure {
  height: 100%;
  display: flex;
  flex-direction: column;
}
.structure-tabs {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.structure-tabs :deep(.el-tabs__content) {
  flex: 1;
  min-height: 0;
}
.structure-tabs :deep(.el-tab-pane) {
  height: 100%;
}
.col-name {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.null-text {
  color: var(--el-text-color-placeholder);
  font-style: italic;
}
.ddl-pane {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.ddl-toolbar {
  flex-shrink: 0;
}
.ddl-content {
  flex: 1;
  margin: 0;
  padding: 12px;
  overflow: auto;
  background: var(--el-fill-color-light);
  border-radius: 8px;
  font-family: var(--lc-font-mono, "Consolas", monospace);
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
