<template>
  <div class="manual-panel">
    <div class="manual-tabs">
      <el-tabs v-model="activeId" @tab-change="onTabChange">
        <el-tab-pane
          v-for="m in manuals"
          :key="m.id"
          :label="m.name"
          :name="m.id"
        />
      </el-tabs>
    </div>
    <div class="manual-frame-wrap">
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";

interface Manual {
  id: string;
  name: string;
  url: string;
}

const manuals = ref<Manual[]>([]);
const activeId = ref("");

const activeUrl = computed(
  () => manuals.value.find((m) => m.id === activeId.value)?.url ?? ""
);

function onTabChange(id: string) {
  activeId.value = id;
}

onMounted(async () => {
  try {
    const data = await invokeToolByChannel("tool:manuals:list", {});
    manuals.value = Array.isArray(data) ? (data as Manual[]) : [];
    if (manuals.value.length > 0) {
      activeId.value = manuals.value[0].id;
    }
  } catch {
    // 静默失败，显示空状态
  }
});
</script>

<style scoped>
.manual-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.manual-tabs {
  flex-shrink: 0;
  padding: 0 12px;
}
.manual-frame-wrap {
  flex: 1;
  overflow: hidden;
  border: 1px solid var(--el-border-color);
  border-radius: 4px;
  margin: 0 12px 12px;
}
.manual-frame {
  width: 100%;
  height: 100%;
  border: none;
}
.manual-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}
</style>
