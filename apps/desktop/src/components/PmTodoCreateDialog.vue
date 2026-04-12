<template>
  <el-dialog
    :model-value="visible"
    title="新建执行任务"
    width="480px"
    :append-to-body="appendToBody"
    @update:model-value="$emit('update:visible', $event)"
    @close="resetForm"
  >
    <el-form label-width="60px" size="default" @submit.prevent="handleSubmit">
      <el-form-item label="标题">
        <el-input v-model="form.title" placeholder="任务标题" @keyup.enter="handleSubmit" />
      </el-form-item>
      <el-form-item label="优先级">
        <el-select v-model="form.priority">
          <el-option label="P0 紧急" value="P0" />
          <el-option label="P1 高" value="P1" />
          <el-option label="P2 中" value="P2" />
          <el-option label="P3 低" value="P3" />
        </el-select>
      </el-form-item>
      <el-form-item label="描述">
        <el-input v-model="form.description" type="textarea" :rows="3" placeholder="可选描述" />
      </el-form-item>
      <el-form-item label="日期">
        <el-date-picker v-model="form.eventAt" type="date" placeholder="可选日期" value-format="YYYY-MM-DD" clearable style="width: 100%" />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="$emit('update:visible', false)">取消</el-button>
      <el-button type="primary" @click="handleSubmit">{{ confirmText }}</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { reactive, watch } from "vue";

interface Props {
  visible: boolean;
  appendToBody?: boolean;
  confirmText?: string;
}

const props = withDefaults(defineProps<Props>(), {
  appendToBody: false,
  confirmText: "创建",
});

const emit = defineEmits<{
  "update:visible": [value: boolean];
  submit: [form: { title: string; priority: string; description: string; eventAt: string | null }];
}>();

const form = reactive({
  title: "",
  priority: "P2",
  description: "",
  eventAt: null as string | null,
});

function resetForm() {
  form.title = "";
  form.priority = "P2";
  form.description = "";
  form.eventAt = null;
}

function handleSubmit() {
  if (!form.title.trim()) return;
  emit("submit", { ...form, title: form.title.trim() });
}

// Reset when dialog closes
watch(() => props.visible, (val) => {
  if (!val) resetForm();
});
</script>
