<template>
  <el-dialog v-model="visible" title="快捷键一览" width="480px" :append-to-body="true">
    <el-table :data="shortcuts" border>
      <el-table-column prop="key" label="快捷键" width="180" />
      <el-table-column prop="desc" label="功能" />
    </el-table>
    <template #footer>
      <el-button @click="visible = false">关闭</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref } from "vue";

const visible = ref(false);

const shortcuts = [
  { key: "Ctrl + /", desc: "打开快捷键帮助" },
  { key: "自定义快捷键", desc: "显示/隐藏主窗口（在设置中配置）" },
];

function show() {
  visible.value = true;
}

defineExpose({ show });
</script>
