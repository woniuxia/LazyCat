<template>
  <div class="quick-inputs">
    <div class="quick-inputs__header">
      <span>快速输入</span>
      <el-button link type="primary" :icon="Plus" @click="emit('add')">添加</el-button>
    </div>
    <div class="quick-inputs__list">
      <div v-for="item in items" :key="item.id" class="quick-inputs__item">
        <el-button
          class="quick-inputs__use"
          size="small"
          :title="item.text"
          @click="emit('use', item)"
        >
          {{ item.text }}
        </el-button>
        <el-button text circle :icon="Edit" title="编辑快速输入" @click="emit('edit', item)" />
        <el-button text circle :icon="Delete" title="删除快速输入" @click="emit('delete', item)" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Delete, Edit, Plus } from "@element-plus/icons-vue";
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
.quick-inputs__item {
  display: inline-flex;
  align-items: center;
  min-width: 0;
}
.quick-inputs__use {
  min-width: 0;
  max-width: min(320px, calc(100vw - 150px));
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.quick-inputs__item > .el-button + .el-button {
  margin-left: 1px;
}
.quick-inputs__item > .el-button.is-circle {
  width: 24px;
  height: 24px;
}
</style>
