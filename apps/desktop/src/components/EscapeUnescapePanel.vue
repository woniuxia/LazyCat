<template>
  <div class="panel-grid">
    <div class="panel-grid-full">
      <el-radio-group v-model="mode" size="small">
        <el-radio-button value="json">JSON 字符串</el-radio-button>
        <el-radio-button value="html">HTML 实体</el-radio-button>
        <el-radio-button value="sql">SQL 字符串</el-radio-button>
        <el-radio-button value="js">JS 字符串</el-radio-button>
      </el-radio-group>
    </div>

    <div class="textarea-wrap">
      <el-input
        v-model="input"
        type="textarea"
        :rows="12"
        placeholder="输入原始文本或已转义文本"
      />
      <span class="char-count">{{ input.length }} 字符</span>
    </div>

    <div class="textarea-wrap">
      <el-input
        v-model="output"
        type="textarea"
        :rows="12"
        readonly
        placeholder="结果"
      />
      <span class="char-count">{{ output.length }} 字符</span>
    </div>

    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="runEscape">转义</el-button>
        <el-button @click="runUnescape">反转义</el-button>
        <el-button @click="swapInputOutput">互换</el-button>
        <el-button @click="copyOutput">复制结果</el-button>
        <el-button @click="clearAll">清空</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";

type EscapeMode = "json" | "html" | "sql" | "js";

const mode = ref<EscapeMode>("json");
const input = ref("");
const output = ref("");

const HTML_ESCAPE_MAP: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  "\"": "&quot;",
  "'": "&#39;",
};

const HTML_UNESCAPE_MAP: Record<string, string> = {
  "&amp;": "&",
  "&lt;": "<",
  "&gt;": ">",
  "&quot;": "\"",
  "&#39;": "'",
  "&apos;": "'",
};

function escapeForJsonString(value: string): string {
  return JSON.stringify(value).slice(1, -1);
}

function unescapeForJsonString(value: string): string {
  const quoted = `"${value.replaceAll("\"", "\\\"")}"`;
  return JSON.parse(quoted) as string;
}

function escapeForHtml(value: string): string {
  return value.replace(/[&<>"']/g, (ch) => HTML_ESCAPE_MAP[ch] ?? ch);
}

function unescapeForHtml(value: string): string {
  return value.replace(/&(amp|lt|gt|quot|apos|#39);/g, (entity) => {
    return HTML_UNESCAPE_MAP[entity] ?? entity;
  });
}

function escapeForSqlString(value: string): string {
  return value.replaceAll("'", "''");
}

function unescapeForSqlString(value: string): string {
  return value.replaceAll("''", "'");
}

function escapeForJsString(value: string): string {
  return value
    .replaceAll("\\", "\\\\")
    .replaceAll("\"", "\\\"")
    .replaceAll("'", "\\'")
    .replaceAll("\n", "\\n")
    .replaceAll("\r", "\\r")
    .replaceAll("\t", "\\t")
    .replaceAll("\b", "\\b")
    .replaceAll("\f", "\\f");
}

function unescapeForJsString(value: string): string {
  return value.replace(/\\(u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|["'\\bfnrt])/g, (match, token: string) => {
    if (token === "\"") return "\"";
    if (token === "'") return "'";
    if (token === "\\") return "\\";
    if (token === "b") return "\b";
    if (token === "f") return "\f";
    if (token === "n") return "\n";
    if (token === "r") return "\r";
    if (token === "t") return "\t";
    if (token.startsWith("x")) {
      return String.fromCharCode(parseInt(token.slice(1), 16));
    }
    if (token.startsWith("u{")) {
      const codePoint = parseInt(token.slice(2, -1), 16);
      return String.fromCodePoint(codePoint);
    }
    if (token.startsWith("u")) {
      return String.fromCharCode(parseInt(token.slice(1), 16));
    }
    return match;
  });
}

function runEscape() {
  try {
    if (mode.value === "json") {
      output.value = escapeForJsonString(input.value);
      return;
    }
    if (mode.value === "html") {
      output.value = escapeForHtml(input.value);
      return;
    }
    if (mode.value === "sql") {
      output.value = escapeForSqlString(input.value);
      return;
    }
    output.value = escapeForJsString(input.value);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function runUnescape() {
  try {
    if (mode.value === "json") {
      output.value = unescapeForJsonString(input.value);
      return;
    }
    if (mode.value === "html") {
      output.value = unescapeForHtml(input.value);
      return;
    }
    if (mode.value === "sql") {
      output.value = unescapeForSqlString(input.value);
      return;
    }
    output.value = unescapeForJsString(input.value);
  } catch {
    ElMessage.error("输入内容格式不正确，无法反转义");
  }
}

function swapInputOutput() {
  input.value = output.value;
  output.value = "";
}

async function copyOutput() {
  if (!output.value) return;
  try {
    await navigator.clipboard.writeText(output.value);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}

function clearAll() {
  input.value = "";
  output.value = "";
}
</script>

<style scoped>
.textarea-wrap {
  position: relative;
}

.char-count {
  position: absolute;
  bottom: 6px;
  right: 10px;
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  pointer-events: none;
}
</style>
