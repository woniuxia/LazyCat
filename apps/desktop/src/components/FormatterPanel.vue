<template>
  <div class="formatter-layout">
    <div class="formatter-toolbar">
      <el-button type="primary" @click="formatCode">执行格式化</el-button>
      <el-input :model-value="formatDetectedLabel" readonly style="max-width: 300px" />
    </div>
    <div class="formatter-editors">
      <MonacoPane :model-value="formatInput" :language="monacoLanguage" @update:model-value="formatInput = $event" />
      <MonacoPane :model-value="formatOutput" :language="monacoLanguage" :read-only="true" @update:model-value="noop" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import MonacoPane from "./MonacoPane.vue";
import { formatHtml, formatJava, formatJson, formatSqlCode, formatXml } from "@lazycat/formatters";

type FormatKind = "json" | "xml" | "html" | "java" | "sql" | "plaintext";

const formatInput = ref("");
const formatOutput = ref("");
const formatDetected = ref<FormatKind>("plaintext");

const monacoLanguage = computed(() => {
  const map: Record<string, string> = { json: "json", xml: "xml", html: "html", java: "java", sql: "sql" };
  return map[formatDetected.value] ?? "plaintext";
});

const formatDetectedLabel = computed(() => {
  const map: Record<FormatKind, string> = {
    json: "自动识别类型: JSON",
    xml: "自动识别类型: XML",
    html: "自动识别类型: HTML",
    java: "自动识别类型: Java",
    sql: "自动识别类型: SQL",
    plaintext: "自动识别类型: 未识别",
  };
  return map[formatDetected.value];
});

function detectFormatKind(input: string): FormatKind {
  const source = input.trim();
  if (!source) return "plaintext";
  try {
    if (JSON.parse(source) !== undefined) return "json";
  } catch { /* not json */ }
  if (source.startsWith("<") && source.endsWith(">")) {
    const lower = source.toLowerCase();
    if (
      lower.includes("<!doctype html") ||
      /<html[\s>]/i.test(source) ||
      /<(head|body|div|span|script|style|main|section|article|nav|footer|header)[\s>]/i.test(source)
    ) return "html";
    return "xml";
  }
  if (
    /\b(select|insert|update|delete|create|alter|drop|truncate|with)\b/i.test(source) &&
    /\b(from|into|table|where|values|set|join)\b/i.test(source)
  ) return "sql";
  if (
    /\b(class|interface|enum|record)\b/.test(source) &&
    /\b(public|private|protected|static|void|package|import)\b/.test(source)
  ) return "java";
  return "plaintext";
}

async function formatByKind(input: string, kind: Exclude<FormatKind, "plaintext">): Promise<string> {
  switch (kind) {
    case "json": return formatJson(input);
    case "xml": return formatXml(input);
    case "html": return formatHtml(input);
    case "java": return formatJava(input);
    case "sql": return formatSqlCode(input);
  }
}

async function formatCode() {
  try {
    const source = formatInput.value;
    if (!source.trim()) {
      formatOutput.value = "";
      formatDetected.value = "plaintext";
      return;
    }
    const detected = detectFormatKind(source);
    formatDetected.value = detected;
    if (detected === "plaintext") {
      throw new Error("无法识别代码类型，目前支持 JSON/XML/HTML/Java/SQL");
    }
    formatOutput.value = await formatByKind(source, detected);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function noop() {}

// Auto-format on input change
let timer: ReturnType<typeof setTimeout> | null = null;
watch(formatInput, () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => formatCode(), 300);
});
</script>

<style scoped>
.formatter-layout {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  gap: 12px;
}

.formatter-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.formatter-editors {
  flex: 1;
  display: flex;
  gap: 14px;
  min-height: 200px;
}

.formatter-editors > * {
  flex: 1;
  min-width: 0;
}
</style>
