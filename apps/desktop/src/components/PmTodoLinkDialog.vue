<template>
  <el-dialog
    :model-value="visible"
    title="绑定已有任务"
    width="560px"
    :append-to-body="appendToBody"
    @update:model-value="$emit('update:visible', $event)"
    @open="$emit('open')"
  >
    <div class="pm-todo-candidate-header">
      <span class="pm-todo-candidate-hint">可绑定当前项目或未归项目的任务</span>
      <el-input
        :model-value="keyword"
        size="small"
        placeholder="搜索任务标题"
        clearable
        style="width: 200px;"
        @update:model-value="$emit('update:keyword', $event)"
        @input="$emit('search')"
      />
    </div>
    <div v-if="loading" class="pm-todo-loading">搜索中...</div>
    <template v-else-if="candidates.length === 0">
      <div class="pm-todo-empty">
        <template v-if="emptyReason === 'blocked_only'">当前项目下所有任务已被其他工作项关联</template>
        <template v-else-if="emptyReason === 'no_match'">未找到匹配的任务</template>
        <template v-else>暂无可绑定的任务</template>
      </div>
    </template>
    <div v-else class="pm-todo-candidate-list">
      <el-checkbox-group :model-value="selectedIds" @update:model-value="$emit('update:selectedIds', $event)">
        <div v-for="c in candidates" :key="c.id" class="pm-todo-candidate-item">
          <el-checkbox :value="c.id">
            <span>{{ c.title }}</span>
            <el-tag v-if="c.isUnassignedProject" size="small" type="warning" effect="plain" style="margin-left: 4px;">未归项目</el-tag>
            <el-tag size="small" :type="c.status === 'completed' ? 'success' : 'info'" effect="plain" style="margin-left: 4px;">
              {{ c.status === 'completed' ? '已完成' : c.status === 'in_progress' ? '进行中' : '待办' }}
            </el-tag>
            <span style="margin-left: 4px; color: #909399; font-size: 12px;">{{ c.priority }}</span>
          </el-checkbox>
        </div>
      </el-checkbox-group>
    </div>
    <template #footer>
      <el-button @click="$emit('update:visible', false)">取消</el-button>
      <el-button type="primary" :disabled="selectedIds.length === 0" @click="$emit('submit')">
        {{ confirmText }} ({{ selectedIds.length }})
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
interface CandidateItem {
  id: number;
  title: string;
  status: string;
  priority: string;
  isUnassignedProject?: boolean;
}

interface Props {
  visible: boolean;
  appendToBody?: boolean;
  confirmText?: string;
  keyword: string;
  loading: boolean;
  candidates: CandidateItem[];
  selectedIds: number[];
  emptyReason?: string;
}

withDefaults(defineProps<Props>(), {
  appendToBody: false,
  confirmText: "绑定",
  emptyReason: "",
});

defineEmits<{
  "update:visible": [value: boolean];
  "update:keyword": [value: string];
  "update:selectedIds": [value: number[]];
  open: [];
  search: [];
  submit: [];
}>();
</script>
