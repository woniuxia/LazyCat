<template>
  <el-dialog
    :model-value="modelValue"
    title="导入 cURL"
    width="min(960px, calc(100vw - 32px))"
    :close-on-click-modal="false"
    @update:model-value="emit('update:modelValue', $event)"
    @closed="reset"
  >
    <div class="curl-import-layout">
      <div class="curl-import-input">
        <el-input
          v-model="commandText"
          type="textarea"
          :rows="14"
          placeholder="粘贴完整 cURL 命令，例如：curl -X POST https://example.com/api -H 'Content-Type: application/json' -d '{...}'"
        />
      </div>
      <div class="curl-import-preview">
        <el-empty v-if="!commandText.trim()" description="输入后实时解析预览" :image-size="72" />
        <el-alert
          v-else-if="parseError"
          type="error"
          :title="parseError"
          show-icon
          :closable="false"
        />
        <template v-else-if="parsed">
          <el-alert
            v-for="warning in parsed.warnings"
            :key="warning"
            type="warning"
            :title="warning"
            show-icon
            :closable="false"
          />
          <dl class="curl-import-summary">
            <dt>Method</dt>
            <dd>
              <span :class="getApiWorkbenchMethodClass(parsed.draft.method)">
                {{ parsed.draft.method }}
              </span>
            </dd>
            <dt>URL</dt>
            <dd class="curl-import-url">{{ parsed.draft.url || "（空）" }}</dd>
            <dt>Query</dt>
            <dd>{{ parsed.draft.query.length }} 个参数</dd>
            <dt>Headers</dt>
            <dd>
              <template v-if="parsed.draft.headers.length > 0">
                <div v-for="row in parsed.draft.headers" :key="row.key" class="curl-import-header-row">
                  <span class="curl-import-header-key">{{ row.key }}</span>
                  <span class="curl-import-header-value">{{ row.value }}</span>
                </div>
              </template>
              <span v-else>无</span>
            </dd>
            <dt>Body</dt>
            <dd>{{ bodySummary }}</dd>
          </dl>
        </template>
      </div>
    </div>
    <template #footer>
      <el-button @click="emit('update:modelValue', false)">取消</el-button>
      <el-button type="primary" :disabled="!parsed || Boolean(parseError)" @click="confirm">
        导入
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { ApiWorkbenchCurlParseResult } from "../utils/apiWorkbenchCurl";
import { parseApiWorkbenchCurl } from "../utils/apiWorkbenchCurl";
import { getApiWorkbenchMethodClass } from "../utils/apiWorkbench";

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (event: "update:modelValue", value: boolean): void;
  (event: "confirm", result: ApiWorkbenchCurlParseResult): void;
}>();

const commandText = ref("");
const parsed = ref<ApiWorkbenchCurlParseResult | null>(null);
const parseError = ref("");
let parseTimer: ReturnType<typeof setTimeout> | null = null;

const bodySummary = computed(() => {
  const draft = parsed.value?.draft;
  if (!draft) return "无";
  if (draft.bodyType === "none") return "无";
  if (draft.bodyType === "form-urlencoded") return `form-urlencoded（${draft.form.length} 项）`;
  const trimmed = draft.body.length > 120 ? `${draft.body.slice(0, 120)}…` : draft.body;
  return `${draft.bodyType}：${trimmed || "（空）"}`;
});

watch(commandText, (next) => {
  if (parseTimer) clearTimeout(parseTimer);
  parseTimer = setTimeout(() => runParse(next), 200);
});

watch(
  () => props.modelValue,
  (visible) => {
    if (visible && commandText.value.trim()) {
      runParse(commandText.value);
    }
  },
);

function runParse(input: string) {
  if (!input.trim()) {
    parsed.value = null;
    parseError.value = "";
    return;
  }
  try {
    parsed.value = parseApiWorkbenchCurl(input);
    parseError.value = "";
  } catch (error) {
    parsed.value = null;
    parseError.value = error instanceof Error ? error.message : "cURL 解析失败";
  }
}

function confirm() {
  if (!parsed.value) return;
  emit("confirm", parsed.value);
}

function reset() {
  commandText.value = "";
  parsed.value = null;
  parseError.value = "";
}
</script>

<style scoped>
.curl-import-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 12px;
}

.curl-import-input :deep(.el-textarea__inner) {
  font-family: var(--lc-font-mono);
  font-size: 12px;
  line-height: 1.55;
}

.curl-import-preview {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 8px;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
  padding: 10px;
  overflow: auto;
  max-height: 420px;
}

.curl-import-summary {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 6px 10px;
  margin: 0;
  font-size: 12px;
}

.curl-import-summary dt {
  color: var(--el-text-color-secondary);
}

.curl-import-summary dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
}

.curl-import-url {
  font-family: var(--lc-font-mono);
}

.curl-import-header-row {
  display: flex;
  gap: 6px;
  font-family: var(--lc-font-mono);
}

.curl-import-header-key {
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.curl-import-header-value {
  color: var(--el-text-color-regular);
  overflow-wrap: anywhere;
}
</style>
