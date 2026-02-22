<template>
  <div class="naming-case-panel">
    <el-input
      v-model="input"
      type="textarea"
      :rows="4"
      placeholder="输入标识符，每行一个（如 hello_world、helloWorld、PascalCase）"
    />
    <div class="results-grid">
      <div v-for="item in styles" :key="item.key" class="result-card">
        <div class="result-label">{{ item.label }}</div>
        <el-input :model-value="results[item.key] || ''" readonly>
          <template #append>
            <el-button @click="copy(results[item.key])">复制</el-button>
          </template>
        </el-input>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";
import { ElMessage } from "element-plus";

const input = ref("");

const styles = [
  { key: "camelCase", label: "camelCase" },
  { key: "pascalCase", label: "PascalCase" },
  { key: "snakeCase", label: "snake_case" },
  { key: "screamingSnake", label: "SCREAMING_SNAKE" },
  { key: "kebabCase", label: "kebab-case" },
  { key: "dotCase", label: "dot.case" },
] as const;

const results = ref<Record<string, string>>({
  camelCase: "",
  pascalCase: "",
  snakeCase: "",
  screamingSnake: "",
  kebabCase: "",
  dotCase: "",
});

let timer: ReturnType<typeof setTimeout> | null = null;

async function convert() {
  if (!input.value.trim()) {
    results.value = { camelCase: "", pascalCase: "", snakeCase: "", screamingSnake: "", kebabCase: "", dotCase: "" };
    return;
  }
  try {
    const res = (await invokeToolByChannel("tool:text:naming-convert", {
      input: input.value,
    })) as Record<string, string>;
    results.value = res;
  } catch (e: any) {
    ElMessage.error(e.message || "转换失败");
  }
}

watch(input, () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(convert, 300);
});

function copy(text: string) {
  if (!text) return;
  navigator.clipboard.writeText(text).then(() => {
    ElMessage.success("已复制");
  });
}
</script>

<style scoped>
.naming-case-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
}
.results-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
}
.result-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.result-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-family: monospace;
}
</style>
