<script setup lang="ts">
import { computed } from "vue";
import {
  CircleCheckFilled,
  CircleCloseFilled,
  Edit,
  Location,
  RefreshRight,
} from "@element-plus/icons-vue";
import type {
  RequestForwardBatchOperationResult,
  RequestForwardRule,
  RequestForwardRuntimeState,
} from "../../types/request-forward";
import { parseRequestForwardError } from "../../utils/requestForward";

const props = defineProps<{
  visible: boolean;
  operation: "start" | "stop";
  results: RequestForwardBatchOperationResult[];
  rules: RequestForwardRule[];
}>();

const emit = defineEmits<{
  "update:visible": [value: boolean];
  close: [];
  locate: [ruleId: number];
  retry: [ruleId: number];
  edit: [ruleId: number];
}>();

const operationLabel = computed(() => (props.operation === "start" ? "启动" : "停止"));
const succeeded = computed(() => props.results.filter((result) => result.ok).length);
const failed = computed(() => props.results.length - succeeded.value);
const ruleNames = computed(() => new Map(props.rules.map((rule) => [rule.id, rule.name])));
const rows = computed(() =>
  props.results.map((result) => ({
    ...result,
    ruleName: ruleNames.value.get(result.ruleId) ?? `规则 #${result.ruleId}`,
    ruleExists: ruleNames.value.has(result.ruleId),
    details: result.ok
      ? null
      : parseRequestForwardError(result.error ?? "未提供错误详情", result.state),
  })),
);

function stateLabel(state: RequestForwardRuntimeState): string {
  return {
    stopped: "已停止",
    starting: "启动中",
    running: "运行中",
    stopping: "停止中",
    failed: "失败",
  }[state];
}

function updateVisible(value: boolean) {
  emit("update:visible", value);
  if (!value) emit("close");
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :title="`批量${operationLabel}结果`"
    width="min(720px, 92vw)"
    class="request-forward-batch-result-dialog"
    @update:model-value="updateVisible"
  >
    <div class="batch-result">
      <div class="batch-result__summary" role="status">
        <strong>已处理 {{ results.length }} 条规则</strong>
        <span class="summary-count is-success">成功 {{ succeeded }}</span>
        <span class="summary-count" :class="failed ? 'is-failed' : 'is-muted'">
          失败 {{ failed }}
        </span>
      </div>

      <div v-if="rows.length" class="batch-result__list" aria-label="批量操作逐条结果">
        <article
          v-for="row in rows"
          :key="row.ruleId"
          class="result-row"
          :class="row.ok ? 'is-success' : 'is-failed'"
        >
          <el-icon class="result-row__icon" aria-hidden="true">
            <CircleCheckFilled v-if="row.ok" />
            <CircleCloseFilled v-else />
          </el-icon>

          <div class="result-row__body">
            <div class="result-row__title">
              <strong>{{ row.ruleName }}</strong>
              <span>{{ row.ok ? `${operationLabel}成功` : `${operationLabel}失败` }}</span>
              <small>{{ stateLabel(row.state) }}</small>
            </div>

            <div v-if="row.details" class="result-row__error" role="alert">
              <code>{{ row.details.code }}</code>
              <p>{{ row.details.message }}</p>
              <span>实际状态：{{ stateLabel(row.details.state) }}</span>
            </div>
          </div>

          <div v-if="!row.ok" class="result-row__actions" aria-label="失败恢复操作">
            <el-tooltip v-if="row.ruleExists" content="定位规则" placement="top">
              <el-button
                text
                circle
                :icon="Location"
                aria-label="定位规则"
                @click="emit('locate', row.ruleId)"
              />
            </el-tooltip>
            <el-tooltip :content="`重试${operationLabel}`" placement="top">
              <el-button
                text
                circle
                :icon="RefreshRight"
                :aria-label="`重试${operationLabel}`"
                @click="emit('retry', row.ruleId)"
              />
            </el-tooltip>
            <el-tooltip v-if="row.ruleExists" content="编辑规则" placement="top">
              <el-button
                text
                circle
                :icon="Edit"
                aria-label="编辑规则"
                @click="emit('edit', row.ruleId)"
              />
            </el-tooltip>
          </div>
        </article>
      </div>

      <div v-else class="batch-result__empty">没有需要处理的规则</div>
    </div>

    <template #footer>
      <el-button type="primary" @click="updateVisible(false)">关闭</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.batch-result {
  display: grid;
  max-height: min(62vh, 620px);
  gap: 12px;
  overflow: hidden;
  color: var(--text-primary, #273548);
}

.batch-result__summary {
  display: flex;
  min-height: 42px;
  align-items: center;
  gap: 14px;
  padding: 0 12px;
  border: 1px solid #dfe5ea;
  border-radius: 5px;
  background: #f7f9fa;
}

.batch-result__summary strong {
  margin-right: auto;
  font-size: 15px;
}

.summary-count {
  font-size: 13px;
  font-weight: 600;
}

.summary-count.is-success {
  color: #168357;
}
.summary-count.is-failed {
  color: #bd3e38;
}
.summary-count.is-muted {
  color: #697586;
}

.batch-result__list {
  min-height: 0;
  overflow-y: auto;
  border-top: 1px solid #e1e6eb;
}

.result-row {
  display: grid;
  min-width: 0;
  grid-template-columns: 22px minmax(0, 1fr) auto;
  gap: 10px;
  padding: 12px 6px;
  border-bottom: 1px solid #e1e6eb;
}

.result-row.is-failed {
  background: #fffafa;
}

.result-row__icon {
  margin-top: 2px;
  font-size: 18px;
}

.result-row.is-success .result-row__icon {
  color: #168357;
}
.result-row.is-failed .result-row__icon {
  color: #c23b35;
}

.result-row__body {
  min-width: 0;
}

.result-row__title {
  display: flex;
  min-width: 0;
  align-items: baseline;
  gap: 9px;
}

.result-row__title strong {
  min-width: 0;
  overflow: hidden;
  font-size: 15px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.result-row__title span {
  flex: none;
  color: #526174;
  font-size: 13px;
  font-weight: 600;
}

.result-row__title small {
  flex: none;
  color: #758194;
  font-size: 12px;
}

.result-row__error {
  display: grid;
  gap: 4px;
  margin-top: 7px;
  color: #8e312d;
  font-size: 13px;
}

.result-row__error code {
  width: fit-content;
  padding: 2px 5px;
  border-radius: 3px;
  background: #f9e4e2;
  color: #8e312d;
  font-size: 11px;
}

.result-row__error p {
  margin: 0;
  overflow-wrap: anywhere;
  line-height: 1.5;
}

.result-row__error span {
  color: #6f5960;
  font-size: 12px;
}

.result-row__actions {
  display: flex;
  align-items: flex-start;
  gap: 1px;
}

.result-row__actions :deep(.el-button) {
  width: 30px;
  height: 30px;
  margin: 0;
}

.batch-result__empty {
  display: grid;
  min-height: 120px;
  place-items: center;
  color: #697586;
  font-size: 14px;
}

@media (max-width: 620px) {
  .batch-result__summary {
    flex-wrap: wrap;
    gap: 6px 12px;
    padding: 9px 10px;
  }
  .batch-result__summary strong {
    width: 100%;
  }
  .result-row {
    grid-template-columns: 20px minmax(0, 1fr);
  }
  .result-row__actions {
    grid-column: 2;
  }
  .result-row__title {
    flex-wrap: wrap;
    gap: 4px 8px;
  }
}
</style>
