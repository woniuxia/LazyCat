<template>
  <div class="http-status-panel">
    <el-input
      v-model="searchQuery"
      placeholder="输入状态码或描述搜索"
      clearable
      prefix-icon="Search"
      style="max-width: 400px; margin-bottom: 4px"
    />

    <div v-if="searchQuery && flatResults.length === 0" class="empty-hint">
      未找到匹配的状态码
    </div>

    <template v-if="searchQuery">
      <el-table :data="flatResults" stripe size="small" :show-header="true" style="width: 100%">
        <el-table-column label="状态码" width="80" align="center">
          <template #default="{ row }">
            <span class="code-cell" :class="codeClass(row.code)">{{ row.code }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="name" label="名称" width="220" />
        <el-table-column prop="desc" label="说明" width="120" />
        <el-table-column prop="usage" label="用途" min-width="180" />
        <el-table-column label="常见原因" min-width="260">
          <template #default="{ row }">
            <div v-if="row.causes" class="causes-cell">
              <span v-for="(c, i) in row.causes.split('; ')" :key="i" class="cause-tag">{{ c }}</span>
            </div>
            <span v-else class="no-cause">-</span>
          </template>
        </el-table-column>
      </el-table>
    </template>

    <template v-else>
      <div v-for="group in groups" :key="group.category" class="group-section">
        <div class="group-header" :class="'header-' + group.category">{{ group.category }} {{ group.name }}</div>
        <el-table :data="group.codes" stripe size="small" :show-header="group === groups[0]" style="width: 100%">
          <el-table-column label="状态码" width="80" align="center">
            <template #default="{ row }">
              <span class="code-cell" :class="codeClass(row.code)">{{ row.code }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="name" label="名称" width="220" />
          <el-table-column prop="desc" label="说明" width="120" />
          <el-table-column prop="usage" label="用途" min-width="180" />
          <el-table-column label="常见原因" min-width="260">
            <template #default="{ row }">
              <div v-if="row.causes" class="causes-cell">
                <span v-for="(c, i) in row.causes.split('; ')" :key="i" class="cause-tag">{{ c }}</span>
              </div>
              <span v-else class="no-cause">-</span>
            </template>
          </el-table-column>
        </el-table>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

interface StatusCode {
  code: number;
  name: string;
  desc: string;
  usage: string;
  causes: string;
}
interface StatusGroup {
  category: string;
  name: string;
  codes: StatusCode[];
}

const searchQuery = ref("");
const groups = ref<StatusGroup[]>([]);
const flatResults = ref<StatusCode[]>([]);

function codeClass(code: number) {
  const prefix = Math.floor(code / 100);
  return `code-${prefix}xx`;
}

async function loadAll() {
  try {
    const data = (await invokeToolByChannel("tool:network:http-status-list", {})) as {
      groups: StatusGroup[];
    };
    groups.value = data.groups;
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function search(query: string) {
  if (!query.trim()) {
    flatResults.value = [];
    return;
  }
  try {
    const data = (await invokeToolByChannel("tool:network:http-status-lookup", {
      query,
    })) as { results: StatusCode[] };
    flatResults.value = data.results;
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

let timer: ReturnType<typeof setTimeout> | null = null;
watch(searchQuery, (val) => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => search(val), 300);
});

onMounted(() => loadAll());
</script>

<style scoped>
.http-status-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.group-section {
  margin-bottom: 4px;
}
.group-header {
  font-size: 13px;
  font-weight: 600;
  padding: 6px 4px 4px;
}
.header-1xx { color: #909399; }
.header-2xx { color: #67C23A; }
.header-3xx { color: #E6A23C; }
.header-4xx { color: #F56C6C; }
.header-5xx { color: #F56C6C; }
.code-cell {
  font-weight: 700;
  font-family: monospace;
  font-size: 13px;
}
.code-1xx { color: #909399; }
.code-2xx { color: #67C23A; }
.code-3xx { color: #E6A23C; }
.code-4xx { color: #F56C6C; }
.code-5xx { color: #F56C6C; }
.causes-cell {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.cause-tag {
  display: inline-block;
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 3px;
  background: var(--el-fill-color);
  color: var(--el-text-color-regular);
  line-height: 1.6;
}
.no-cause {
  color: var(--el-text-color-placeholder);
}
.empty-hint {
  color: var(--el-text-color-secondary);
  text-align: center;
  padding: 24px;
}
</style>
