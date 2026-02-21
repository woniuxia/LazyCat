<template>
  <div class="cron-panel">
    <section class="panel-block">
      <div class="section-head">
        <h3>常用模板</h3>
      </div>
      <div class="template-list">
        <el-button
          v-for="tpl in templates"
          :key="tpl.label"
          size="small"
          @click="applyTemplate(tpl.expression)"
        >
          {{ tpl.label }}
        </el-button>
      </div>
    </section>

    <section class="panel-block">
      <div class="section-head">
        <h3>表达式</h3>
        <el-space>
          <el-tag :type="isExpressionValid ? 'success' : 'info'" effect="light">
            {{ isExpressionValid ? '已校验' : '未校验' }}
          </el-tag>
        </el-space>
      </div>

      <div class="expression-row">
        <el-input
          v-model="cronExpression"
          placeholder="支持 Spring 6 字段，兼容 5 字段（自动补秒）"
          clearable
        />
        <el-button type="primary" @click="normalizeAndDescribe">解析并规范化</el-button>
        <el-button @click="copyExpression">复制</el-button>
      </div>

      <el-alert
        v-if="warnings.length"
        type="warning"
        show-icon
        :closable="false"
        :title="warnings[0]"
      />

      <div class="summary-box" v-if="summary">
        <div class="summary-title">规则描述</div>
        <div class="summary-text">{{ summary }}</div>
        <div class="summary-details" v-if="summaryDetails.length">
          <div
            v-for="(item, index) in summaryDetailRows"
            :key="`${index}-${item.label}-${item.value}`"
            class="summary-detail-item"
          >
            <span class="summary-detail-label">{{ item.label }}</span>
            <span class="summary-detail-value">{{ item.value }}</span>
          </div>
        </div>
      </div>
    </section>

    <section class="panel-block">
      <div class="section-head">
        <h3>字段构建（Spring 6 字段）</h3>
      </div>

      <div class="field-grid">
        <div class="field-row" v-for="field in fieldDefs" :key="field.key">
          <label class="field-label">{{ field.label }}</label>
          <el-input
            v-model="fields[field.key]"
            :placeholder="field.placeholder"
            @input="onFieldInput"
          />
          <div class="field-shortcuts">
            <el-button
              v-for="shortcut in field.shortcuts"
              :key="shortcut"
              size="small"
              text
              @click="applyShortcut(field.key, shortcut)"
            >
              {{ shortcut }}
            </el-button>
          </div>
        </div>
      </div>

      <el-alert
        v-if="domDowHint"
        type="info"
        show-icon
        :closable="false"
        title="日与周字段都设置了固定规则，请确认是否符合你的业务语义。"
      />
    </section>

    <section class="panel-block">
      <div class="section-head">
        <h3>触发预览</h3>
      </div>

      <div class="preview-controls">
        <el-select v-model="previewTimezone" style="width: 220px">
          <el-option
            v-for="tz in timezoneOptions"
            :key="tz.value"
            :label="tz.label"
            :value="tz.value"
          />
        </el-select>
        <el-input-number v-model="previewCount" :min="1" :max="50" />
        <el-button type="primary" @click="previewCron">预览未来触发时间</el-button>
      </div>

      <el-table :data="previewItems" border stripe size="small" class="preview-table">
        <el-table-column label="#" width="60" align="center">
          <template #default="scope">{{ scope.$index + 1 }}</template>
        </el-table-column>
        <el-table-column prop="display" label="显示时间" min-width="210" />
        <el-table-column prop="iso" label="ISO" min-width="270" show-overflow-tooltip />
        <el-table-column prop="epochMs" label="Epoch(ms)" min-width="180" />
      </el-table>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { CronDescribeResponse, CronFieldParts, CronNormalizeResponse, CronPreviewItem, CronPreviewV2Response } from "../types";

const fieldDefs: Array<{
  key: keyof CronFieldParts;
  label: string;
  placeholder: string;
  shortcuts: string[];
}> = [
  { key: "second", label: "秒", placeholder: "0", shortcuts: ["0", "*", "*/5", "*/10"] },
  { key: "minute", label: "分", placeholder: "*", shortcuts: ["*", "*/5", "*/10", "0"] },
  { key: "hour", label: "时", placeholder: "*", shortcuts: ["*", "*/2", "9-18", "0"] },
  { key: "dayOfMonth", label: "日", placeholder: "*", shortcuts: ["*", "1", "1-5", "*/2"] },
  { key: "month", label: "月", placeholder: "*", shortcuts: ["*", "1", "1-6", "7-12"] },
  { key: "dayOfWeek", label: "周", placeholder: "*", shortcuts: ["*", "1-5", "1", "6,0"] },
];

const templates = [
  { label: "每30秒", expression: "*/30 * * * * *" },
  { label: "每分钟", expression: "0 * * * * *" },
  { label: "每5分钟", expression: "0 */5 * * * *" },
  { label: "每10分钟", expression: "0 */10 * * * *" },
  { label: "每15分钟", expression: "0 */15 * * * *" },
  { label: "每30分钟", expression: "0 */30 * * * *" },
  { label: "每小时整点", expression: "0 0 * * * *" },
  { label: "每2小时", expression: "0 0 */2 * * *" },
  { label: "每天 00:00", expression: "0 0 0 * * *" },
  { label: "每天 09:00", expression: "0 0 9 * * *" },
  { label: "每天 18:00", expression: "0 0 18 * * *" },
  { label: "工作日 09:00", expression: "0 0 9 * * 1-5" },
  { label: "工作日 18:00", expression: "0 0 18 * * 1-5" },
  { label: "每周一 09:00", expression: "0 0 9 * * 1" },
  { label: "每周五 18:00", expression: "0 0 18 * * 5" },
  { label: "每月 1 日 00:00", expression: "0 0 0 1 * *" },
  { label: "每月 1 日 09:00", expression: "0 0 9 1 * *" },
  { label: "每月最后一天 23:00", expression: "0 0 23 L * *" },
];

const timezoneOptions = [
  { label: "本地时区", value: "local" },
  { label: "UTC", value: "UTC" },
  { label: "Asia/Shanghai", value: "Asia/Shanghai" },
  { label: "America/Los_Angeles", value: "America/Los_Angeles" },
  { label: "Europe/London", value: "Europe/London" },
];

const fields = reactive<CronFieldParts>({
  second: "0",
  minute: "*",
  hour: "*",
  dayOfMonth: "*",
  month: "*",
  dayOfWeek: "*",
});

const cronExpression = ref("0 * * * * *");
const warnings = ref<string[]>([]);
const isExpressionValid = ref(false);

const summary = ref("");
const summaryDetails = ref<string[]>([]);

const previewCount = ref(8);
const previewTimezone = ref("local");
const previewItems = ref<CronPreviewItem[]>([]);

const domDowHint = computed(() => fields.dayOfMonth !== "*" && fields.dayOfWeek !== "*");
const summaryDetailRows = computed(() =>
  summaryDetails.value.map((item) => {
    const idx = item.indexOf(":");
    if (idx < 0) return { label: item, value: "" };
    return {
      label: item.slice(0, idx + 1),
      value: item.slice(idx + 1).trim(),
    };
  }),
);

function expressionFromFields(): string {
  return [fields.second, fields.minute, fields.hour, fields.dayOfMonth, fields.month, fields.dayOfWeek].join(" ");
}

function onFieldInput() {
  cronExpression.value = expressionFromFields();
  isExpressionValid.value = false;
}

function applyShortcut(key: keyof CronFieldParts, value: string) {
  fields[key] = value;
  onFieldInput();
}

function applyNormalizedParts(parts: CronFieldParts) {
  fields.second = parts.second;
  fields.minute = parts.minute;
  fields.hour = parts.hour;
  fields.dayOfMonth = parts.dayOfMonth;
  fields.month = parts.month;
  fields.dayOfWeek = parts.dayOfWeek;
  cronExpression.value = expressionFromFields();
}

async function normalizeAndDescribe() {
  try {
    await normalizeExpression(false);

    const described = (await invokeToolByChannel("tool:cron:describe", {
      expression: cronExpression.value,
      locale: "zh-CN",
    })) as CronDescribeResponse;

    summary.value = described.summary;
    summaryDetails.value = described.details;
    for (const warning of described.warnings || []) {
      if (!warnings.value.includes(warning)) warnings.value.push(warning);
    }

    ElMessage.success("Cron 表达式已规范化");
  } catch (error) {
    isExpressionValid.value = false;
    summary.value = "";
    summaryDetails.value = [];
    ElMessage.error((error as Error).message);
  }
}

async function normalizeExpression(showMessage: boolean) {
  const normalized = (await invokeToolByChannel("tool:cron:normalize", {
    expression: cronExpression.value,
    standard: "spring6",
  })) as CronNormalizeResponse;

  applyNormalizedParts(normalized.parts);
  cronExpression.value = normalized.normalizedExpression;
  warnings.value = normalized.warnings || [];
  isExpressionValid.value = true;

  if (showMessage) {
    ElMessage.success("Cron 表达式已规范化");
  }
}

async function previewCron() {
  try {
    await normalizeExpression(false);
    const data = (await invokeToolByChannel("tool:cron:preview-v2", {
      expression: cronExpression.value,
      count: previewCount.value,
      timezone: previewTimezone.value,
    })) as CronPreviewV2Response;

    cronExpression.value = data.normalizedExpression;
    previewItems.value = data.items || [];
    warnings.value = [...new Set([...(warnings.value || []), ...(data.warnings || [])])];
    isExpressionValid.value = true;
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function copyExpression() {
  try {
    await navigator.clipboard.writeText(cronExpression.value);
    ElMessage.success("已复制表达式");
  } catch {
    ElMessage.error("复制失败");
  }
}

function applyTemplate(expression: string) {
  cronExpression.value = expression;
  void normalizeAndDescribe();
}

void normalizeAndDescribe();
</script>

<style scoped>
.cron-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.panel-block {
  border: 1px solid var(--lc-border);
  border-radius: 10px;
  padding: 12px;
  background: var(--el-bg-color-page);
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.section-head h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}

.expression-row {
  display: grid;
  grid-template-columns: 1fr auto auto;
  gap: 8px;
}

.summary-box {
  border-radius: 8px;
  background: var(--el-fill-color-light);
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.summary-title {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.summary-text {
  font-size: 14px;
  font-weight: 500;
}

.summary-details {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 6px;
  min-height: 28px;
  align-content: start;
}

.summary-detail-item {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  padding: 4px 8px;
  font-size: 12px;
  color: var(--el-text-color-regular);
  line-height: 1.4;
  background: var(--el-fill-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.summary-detail-label {
  color: var(--el-text-color-secondary);
  margin-right: 4px;
}

.summary-detail-value {
  color: var(--el-color-primary-light-3);
  font-weight: 600;
}

.field-grid {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field-row {
  display: grid;
  grid-template-columns: 48px 260px minmax(320px, 1fr);
  align-items: center;
  gap: 8px;
}

.field-label {
  color: var(--el-text-color-secondary);
  font-size: 13px;
}

.field-shortcuts {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  align-items: center;
  min-height: 32px;
}

.field-shortcuts :deep(.el-button) {
  min-width: 56px;
  justify-content: center;
}

.template-list {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.preview-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.preview-table {
  width: 100%;
}

@media (max-width: 960px) {
  .expression-row {
    grid-template-columns: 1fr;
  }

  .field-row {
    grid-template-columns: 1fr;
    align-items: stretch;
  }
}
</style>
