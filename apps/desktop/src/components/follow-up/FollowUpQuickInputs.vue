<template>
  <div class="quick-inputs">
    <div class="quick-inputs__header">
      <span>快速输入</span>
      <el-button link type="primary" :icon="Plus" @click="emit('add')">添加</el-button>
    </div>
    <div class="quick-inputs__list">
      <el-dropdown
        v-for="item in items"
        :key="item.id"
        split-button
        size="small"
        :title="item.text"
        @click="emit('use', item)"
      >
        {{ item.text }}
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item @click="emit('edit', item)">编辑</el-dropdown-item>
            <el-dropdown-item class="danger-item" @click="emit('delete', item)">
              删除
            </el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Plus } from "@element-plus/icons-vue";
import type { FollowUpQuickInput } from "../../utils/followUpQuickInputs";

defineProps<{ items: FollowUpQuickInput[] }>();
const emit = defineEmits<{
  add: [];
  use: [item: FollowUpQuickInput];
  edit: [item: FollowUpQuickInput];
  delete: [item: FollowUpQuickInput];
}>();
</script>

<style scoped>
.quick-inputs {
  margin: -6px 0 18px;
}
.quick-inputs__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 7px;
  color: var(--lc-text-muted);
  font-size: 12px;
}
.quick-inputs__list {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
}
.quick-inputs__list :deep(.el-dropdown),
.quick-inputs__list :deep(.el-button-group) {
  max-width: 100%;
}
.quick-inputs__list :deep(.el-button-group > .el-button:first-child) {
  min-width: 0;
  max-width: min(320px, calc(100vw - 120px));
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
:deep(.danger-item) {
  color: var(--lc-danger);
}
</style>
