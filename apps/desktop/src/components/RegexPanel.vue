<template>
  <div class="panel-grid">
    <el-input v-model="pattern" placeholder="正则表达式" />
    <el-input v-model="flags" placeholder="flags，例如 gi" />
    <el-input
      class="panel-grid-full"
      v-model="input"
      type="textarea"
      :rows="8"
      placeholder="待匹配文本"
    />
    <el-input class="panel-grid-full" :model-value="output" type="textarea" :rows="8" readonly placeholder="匹配结果" />
    <el-select v-model="kind" placeholder="常用模板">
      <el-option label="邮箱" value="email" />
      <el-option label="IPv4" value="ipv4" />
      <el-option label="URL" value="url" />
      <el-option label="中国手机号" value="phone-cn" />
    </el-select>
    <div>
      <el-space>
        <el-button type="primary" @click="runRegexTest">执行匹配</el-button>
        <el-button @click="applyTemplate">填充模板</el-button>
        <el-button @click="loadTemplates">查看模板库</el-button>
      </el-space>
    </div>
    <el-input
      class="panel-grid-full"
      :model-value="templatesDisplay"
      type="textarea"
      :rows="6"
      readonly
      placeholder="模板库"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const pattern = ref("");
const flags = ref("g");
const input = ref("");
const output = ref("");
const kind = ref<"email" | "ipv4" | "url" | "phone-cn">("email");
const templates = ref<unknown[]>([]);

const templatesDisplay = computed(() => JSON.stringify(templates.value, null, 2));

async function runRegexTest() {
  try {
    const data = await invokeToolByChannel("tool:regex:test", {
      pattern: pattern.value,
      flags: flags.value,
      input: input.value,
    });
    output.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function applyTemplate() {
  try {
    pattern.value = String(await invokeToolByChannel("tool:regex:generate", { kind: kind.value }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function loadTemplates() {
  try {
    const data = await invokeToolByChannel("tool:regex:templates", {});
    templates.value = Array.isArray(data) ? data : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

// Auto-test on input change
let timer: ReturnType<typeof setTimeout> | null = null;
watch([pattern, flags, input], () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    if (!pattern.value.trim() && !input.value.trim()) {
      output.value = "";
      return;
    }
    runRegexTest();
  }, 300);
});

onMounted(() => loadTemplates());
</script>
