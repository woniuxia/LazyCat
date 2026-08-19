<template>
  <div class="task-workspace-container">
    <div class="task-workspace-layout" :class="{ 'is-detail-open': detailOpen }">
      <aside class="task-workspace-sidebar" :class="`${namespace}-sidebar`">
        <div class="task-workspace-switch"><slot name="switch" /></div>
        <div class="task-workspace-sidebar-content"><slot name="sidebar" /></div>
      </aside>

      <section class="task-workspace-list-pane" :class="`${namespace}-list-pane`">
        <header class="task-workspace-toolbar"><slot name="toolbar" /></header>
        <div class="task-workspace-list-content"><slot name="list" /></div>
      </section>

      <section class="task-workspace-detail-pane" :class="`${namespace}-detail-pane`">
        <div class="task-workspace-detail-backbar">
          <el-button :icon="Back" title="返回列表" @click="emit('closeDetail')">返回列表</el-button>
        </div>
        <div class="task-workspace-detail-content"><slot name="detail" /></div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Back } from "@element-plus/icons-vue";

defineProps<{
  namespace: "todo" | "follow-up";
  detailOpen: boolean;
}>();

const emit = defineEmits<{
  closeDetail: [];
}>();
</script>

<style scoped>
.task-workspace-container {
  container: task-workspace / inline-size;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
}

.task-workspace-layout {
  display: grid;
  grid-template-columns: 260px minmax(360px, 1.2fr) minmax(300px, 1fr);
  grid-template-areas: "sidebar list detail";
  gap: 16px;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
}

.task-workspace-sidebar,
.task-workspace-list-pane,
.task-workspace-detail-pane,
.task-workspace-list-content,
.task-workspace-detail-content {
  min-width: 0;
  min-height: 0;
}

.task-workspace-sidebar {
  grid-area: sidebar;
  display: flex;
  flex-direction: column;
  gap: 20px;
  padding-right: 4px;
}

.task-workspace-switch {
  flex: 0 0 auto;
}

.task-workspace-sidebar-content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.task-workspace-list-pane {
  grid-area: list;
  display: flex;
  flex-direction: column;
}

.task-workspace-toolbar {
  display: flex;
  align-items: stretch;
  height: 80px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--lc-border-subtle);
  margin-bottom: 12px;
  flex: 0 0 auto;
}

.task-workspace-list-content {
  display: flex;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
}

.task-workspace-detail-pane {
  grid-area: detail;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
}

.task-workspace-detail-backbar {
  display: none;
}

.task-workspace-detail-content {
  display: flex;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
}

@container task-workspace (max-width: 1280px) {
  .task-workspace-layout {
    grid-template-columns: 240px minmax(320px, 1.2fr) minmax(320px, 1fr);
    gap: 14px;
  }
}

@container task-workspace (max-width: 1024px) {
  .task-workspace-layout {
    grid-template-columns: 220px minmax(300px, 1fr) 300px;
    gap: 12px;
  }
}

@container task-workspace (max-width: 900px) {
  .task-workspace-layout {
    grid-template-columns: 220px minmax(0, 1fr);
    grid-template-areas: "sidebar list";
  }

  .task-workspace-detail-pane {
    grid-area: list;
    z-index: 4;
    display: none;
  }

  .task-workspace-layout.is-detail-open .task-workspace-detail-pane {
    display: flex;
  }

  .task-workspace-detail-backbar {
    display: flex;
    align-items: center;
    min-height: 48px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--lc-border);
    flex: 0 0 auto;
  }
}

@container task-workspace (max-width: 640px) {
  .task-workspace-layout {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    grid-template-areas:
      "sidebar"
      "list";
  }

  .task-workspace-sidebar {
    gap: 0;
    padding-right: 0;
  }

  .task-workspace-sidebar-content {
    display: none;
  }

  .task-workspace-toolbar {
    height: 80px;
  }
}
</style>
