<template>
  <section class="api-mock-routes">
    <div class="api-mock-toolbar">
      <div>
        <h2>路由</h2>
        <span v-if="enabledCount !== null">{{ enabledCount }}/{{ routes.length }} 启用</span>
      </div>
      <el-button :icon="Plus" size="small" :disabled="!hasProject" @click="emit('create')"
        >路由</el-button
      >
    </div>

    <div v-if="!hasProject" class="api-mock-empty">选择或新建一个 Mock 项目。</div>
    <template v-else>
      <div v-if="routes.length === 0" class="api-mock-empty">
        暂无路由。添加第一条路由后即可启动服务。
      </div>
      <div ref="listRef" class="route-list">
        <div
          v-for="route in routes"
          :key="route.id"
          class="api-mock-route"
          :class="{ active: route.id === selectedId, disabled: !route.enabled }"
          role="button"
          tabindex="0"
          @click="emit('select', route.id)"
          @keydown.enter="emit('select', route.id)"
        >
          <span
            class="method"
            :style="{ color: route.enabled ? getMockMethodColor(route.method) : '#94a3b8' }"
            >{{ route.method }}</span
          >
          <span
            class="route-path"
            :title="
              route.name && route.name !== route.pathPattern
                ? `${route.name}\n${route.pathPattern}`
                : route.pathPattern
            "
            >{{ route.pathPattern }}</span
          >
          <span class="route-controls" @click.stop>
            <el-button
              class="route-copy"
              :icon="CopyDocument"
              size="small"
              text
              title="复制为新路由"
              @click="emit('duplicate', route)"
            />
            <el-switch
              size="small"
              :model-value="route.enabled"
              @change="(value: string | number | boolean) => emit('toggle', route, Boolean(value))"
            />
          </span>
          <span class="route-meta">
            {{ route.statusCode }} · {{ route.responseKind === "file" ? "文件" : "文本" }}
            <span v-if="route.delayMs > 0"> · 延迟 {{ route.delayMs }}ms</span>
            <span v-if="!route.enabled"> · 已禁用</span>
          </span>
        </div>
      </div>
      <div v-if="routes.length > 1" class="route-order-note">
        同等级路由按列表顺序优先匹配，可拖拽调整
      </div>
    </template>
  </section>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { CopyDocument, Plus } from "@element-plus/icons-vue";
import Sortable from "sortablejs";
import type { ApiMockRouteSummary } from "../../types/api-mock";
import { getMockMethodColor } from "../../utils/apiMock";

const props = defineProps<{
  routes: ApiMockRouteSummary[];
  selectedId: number | null;
  hasProject: boolean;
  enabledCount: number | null;
}>();

const emit = defineEmits<{
  (event: "select", id: number): void;
  (event: "create"): void;
  (event: "toggle", route: ApiMockRouteSummary, enabled: boolean): void;
  (event: "duplicate", route: ApiMockRouteSummary): void;
  (event: "reorder", ids: number[]): void;
}>();

const listRef = ref<HTMLElement | null>(null);
let sortable: Sortable | null = null;

onMounted(() => {
  if (!listRef.value) return;
  sortable = Sortable.create(listRef.value, {
    animation: 150,
    draggable: ".api-mock-route",
    filter: ".route-controls",
    preventOnFilter: false,
    ghostClass: "api-mock-sortable-ghost",
    forceFallback: true,
    onEnd: (event) => {
      const { oldIndex, newIndex } = event;
      if (oldIndex == null || newIndex == null || oldIndex === newIndex) return;
      const ids = props.routes.map((route) => route.id);
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
</script>

<style scoped>
.api-mock-routes {
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

.route-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.api-mock-route {
  display: grid;
  grid-template-columns: 58px minmax(0, 1fr) auto;
  gap: 2px 8px;
  align-items: center;
  width: 100%;
  padding: 6px 10px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #f8fafc;
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    background-color 0.15s ease;
}

.api-mock-route:hover {
  border-color: #93c5fd;
  background: #f0f7ff;
}

.api-mock-route.active {
  border-color: #2563eb;
  background: #eff6ff;
}

.api-mock-route.disabled {
  color: #94a3b8;
}

.api-mock-route.api-mock-sortable-ghost {
  opacity: 0.4;
  border-style: dashed;
}

.method {
  font-size: 12px;
  font-weight: 700;
}

.route-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.route-controls {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.route-copy {
  opacity: 0;
  transition: opacity 0.15s ease;
}

.api-mock-route:hover .route-copy,
.api-mock-route:focus-within .route-copy {
  opacity: 1;
}

.route-meta {
  grid-column: 2 / 4;
  font-size: 12px;
  color: #64748b;
}

.route-order-note {
  padding: 2px 2px 0;
  font-size: 12px;
  color: #94a3b8;
}

.api-mock-empty {
  padding: 20px 10px;
  font-size: 13px;
  line-height: 1.6;
  color: #64748b;
}
</style>
