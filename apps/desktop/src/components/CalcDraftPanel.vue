<template>
  <div class="calc-draft">
    <div class="calc-draft-list">
      <el-empty v-if="!history.length" description="暂无计算记录，输入公式后按回车" />
      <div
        v-for="item in history"
        :key="item.id"
        class="calc-row calc-row-history"
        tabindex="0"
        @click="emit('historyClick', item)"
        @keyup.enter="emit('historyClick', item)"
      >
        <div class="calc-expression">{{ item.expression }}</div>
        <div class="calc-result-wrap">
          <div class="calc-result">= {{ item.resultDisplay }}</div>
          <div class="calc-time">{{ formatHistoryTime(item.createdAt) }}</div>
        </div>
      </div>
    </div>
    <div class="calc-row calc-row-active">
      <el-input
        class="calc-input"
        :model-value="currentInput"
        type="textarea"
        :rows="3"
        placeholder="输入计算公式，例如 23.7%*5789+4587，按回车计算并复制结果"
        @update:model-value="onInput"
        @keydown.enter.exact.prevent="emit('enter')"
      />
      <div class="calc-result calc-result-pending">= {{ currentPreview || "" }}</div>
    </div>
    <div class="calc-draft-actions">
      <el-space>
        <el-button type="primary" @click="emit('enter')">计算并复制</el-button>
        <el-button @click="emit('clearHistory')">清空历史</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
interface CalcDraftItem {
  id: number;
  expression: string;
  resultRaw: string;
  resultDisplay: string;
  createdAt: number;
}

const props = defineProps<{
  history: CalcDraftItem[];
  currentInput: string;
  currentPreview: string;
  formatHistoryTime: (timestamp: number) => string;
}>();

const emit = defineEmits<{
  (event: "update:currentInput", value: string): void;
  (event: "enter"): void;
  (event: "clearHistory"): void;
  (event: "historyClick", item: CalcDraftItem): void;
}>();

function onInput(value: string | number) {
  emit("update:currentInput", String(value ?? ""));
}
</script>
