<template>
  <div class="manual-panel">
    <div v-if="!activeUrl" class="manual-placeholder">
      <el-empty description="暂无文档内容" />
    </div>
    <iframe
      v-else
      :key="activeUrl"
      :src="activeUrl"
      class="manual-frame"
      sandbox="allow-scripts allow-same-origin allow-popups allow-forms"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";

interface Manual {
  id: string;
  name: string;
  url: string;
}

const props = defineProps<{
  manualId: string;
}>();

const manuals = ref<Manual[]>([]);
const activeUrl = ref("");

/** manualId 格式为 "manual-vue3" -> 后端 id 为 "vue3" */
function backendId(id: string) {
  return id.replace(/^manual-/, "");
}

function updateUrl() {
  const bid = backendId(props.manualId);
  const found = manuals.value.find((m) => m.id === bid);
  activeUrl.value = found?.url ?? "";
}

async function loadManuals() {
  try {
    const data = await invokeToolByChannel("tool:manuals:list", {});
    manuals.value = Array.isArray(data) ? (data as Manual[]) : [];
  } catch {
    manuals.value = [];
  }
  updateUrl();
}

watch(() => props.manualId, updateUrl);

onMounted(loadManuals);
</script>

<style scoped>
.manual-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.manual-frame {
  width: 100%;
  height: 100%;
  border: none;
  flex: 1;
}
.manual-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}
</style>
