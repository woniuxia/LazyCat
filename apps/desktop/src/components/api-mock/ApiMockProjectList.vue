<template>
  <aside class="api-mock-projects">
    <div class="api-mock-toolbar">
      <div>
        <h2>API Mock</h2>
        <span>{{ projects.length }} 个项目</span>
      </div>
      <el-button :icon="Plus" size="small" @click="emit('create')">项目</el-button>
    </div>

    <div v-if="projects.length === 0" class="api-mock-empty">
      暂无 Mock 项目。先新建项目，再添加路由并启动本地服务。
    </div>

    <div ref="listRef" class="project-list">
      <button
        v-for="project in projects"
        :key="project.id"
        class="api-mock-project"
        :class="{ active: project.id === selectedId }"
        type="button"
        @click="emit('select', project.id)"
      >
        <span class="project-main">
          <strong>{{ project.name }}</strong>
          <small>{{ project.host }}:{{ project.port }}</small>
        </span>
        <el-tag size="small" :type="runtimeTagType(project)">
          {{ runtimeLabel(project) }}
        </el-tag>
      </button>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { Plus } from "@element-plus/icons-vue";
import Sortable from "sortablejs";
import type { ApiMockProjectSummary } from "../../types/api-mock";
import { deriveMockProjectRuntimeState } from "../../utils/apiMock";

const props = defineProps<{
  projects: ApiMockProjectSummary[];
  selectedId: number | null;
}>();

const emit = defineEmits<{
  (event: "select", id: number): void;
  (event: "create"): void;
  (event: "reorder", ids: number[]): void;
}>();

const listRef = ref<HTMLElement | null>(null);
let sortable: Sortable | null = null;

onMounted(() => {
  if (!listRef.value) return;
  sortable = Sortable.create(listRef.value, {
    animation: 150,
    draggable: ".api-mock-project",
    ghostClass: "api-mock-sortable-ghost",
    forceFallback: true,
    onEnd: (event) => {
      const { oldIndex, newIndex } = event;
      if (oldIndex == null || newIndex == null || oldIndex === newIndex) return;
      const ids = props.projects.map((project) => project.id);
      const [moved] = ids.splice(oldIndex, 1);
      if (moved === undefined) return;
      ids.splice(newIndex, 0, moved);
      emit("reorder", ids);
    },
  });
});

onBeforeUnmount(() => {
  sortable?.destroy();
  sortable = null;
});

function runtimeLabel(project: ApiMockProjectSummary): string {
  const state = deriveMockProjectRuntimeState(project);
  if (state === "running") return "运行中";
  if (state === "restart-required") return "需重启";
  if (state === "error") return "异常";
  return "已停止";
}

function runtimeTagType(project: ApiMockProjectSummary): "success" | "warning" | "danger" | "info" {
  const state = deriveMockProjectRuntimeState(project);
  if (state === "running") return "success";
  if (state === "restart-required") return "warning";
  if (state === "error") return "danger";
  return "info";
}
</script>

<style scoped>
.api-mock-projects {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  padding: 12px;
  border: 1px solid #dbe4ef;
  border-radius: 8px;
  background: #ffffff;
  overflow: auto;
}

.api-mock-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.api-mock-toolbar h2 {
  margin: 0;
  font-size: 15px;
  line-height: 1.4;
}

.api-mock-toolbar span {
  font-size: 12px;
  color: #64748b;
}

.project-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.api-mock-project {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
  padding: 10px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #f8fafc;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.api-mock-project.active {
  border-color: #2563eb;
  background: #eff6ff;
}

.api-mock-project.api-mock-sortable-ghost {
  opacity: 0.4;
  border-style: dashed;
}

.project-main {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
}

.project-main strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-main small {
  font-size: 12px;
  color: #64748b;
}

.api-mock-empty {
  padding: 20px 10px;
  font-size: 13px;
  line-height: 1.6;
  color: #64748b;
}
</style>
