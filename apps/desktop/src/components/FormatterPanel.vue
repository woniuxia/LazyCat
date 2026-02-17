<template>
  <div class="panel-grid">
    <MonacoPane :model-value="input" :language="language" @update:model-value="emit('update:input', $event)" />
    <MonacoPane :model-value="output" :language="language" :read-only="true" @update:model-value="noop" />
    <div>
      <el-button type="primary" @click="emit('format')">执行格式化</el-button>
    </div>
    <el-input :model-value="detectedLabel" readonly />
  </div>
</template>

<script setup lang="ts">
import MonacoPane from "./MonacoPane.vue";

defineProps<{
  input: string;
  output: string;
  language: string;
  detectedLabel: string;
}>();

const emit = defineEmits<{
  (event: "update:input", value: string): void;
  (event: "format"): void;
}>();

function noop() {
  // MonacoPane in readOnly mode should not emit changes, keep handler for explicitness.
}
</script>
