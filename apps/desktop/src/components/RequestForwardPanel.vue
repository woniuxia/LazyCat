<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useToolInvoke } from "../composables/useToolInvoke";
import type {
  RequestForwardBatchOperationResult,
  RequestForwardRule,
  RequestForwardRuleForm,
  RequestForwardRuntimeState,
  RequestForwardRuntimeStatus,
} from "../types/request-forward";
import {
  getDefaultRequestForwardForm,
  getRequestForwardBatchMessage,
  isRequestForwardRuleReadonly,
  toRequestForwardRuleWriteInput,
  validateRequestForwardRuleForm,
} from "../utils/requestForward";
import RequestForwardRuleFormEditor from "./request-forward/RequestForwardRuleForm.vue";
import RequestForwardRuleList from "./request-forward/RequestForwardRuleList.vue";

type RuleListEnvelope = { items: RequestForwardRule[] };
type StatusListEnvelope = { items: RequestForwardRuntimeStatus[] };
type RuleEnvelope = { item: RequestForwardRule };
type StatusEnvelope = { item: RequestForwardRuntimeStatus };
type BatchEnvelope = { results: RequestForwardBatchOperationResult[] };

const { loading, invoke } = useToolInvoke();
const rules = ref<RequestForwardRule[]>([]);
const statuses = ref<RequestForwardRuntimeStatus[]>([]);
const selectedId = ref<number | null>(null);
const draft = ref(false);
const form = ref<RequestForwardRuleForm>(getDefaultRequestForwardForm());
const formDirty = ref(false);
const fieldErrors = ref<Partial<Record<keyof RequestForwardRuleForm, string>>>({});
const saving = ref(false);
const operating = ref(false);

let refreshRequestToken = 0;
let selectionIntentToken = 0;
let pollTimer: ReturnType<typeof setTimeout> | undefined;
let pollGeneration = 0;
let pollInFlight = false;

const selectedRule = computed(
  () => rules.value.find((rule) => rule.id === selectedId.value) ?? null,
);
const selectedStatus = computed<RequestForwardRuntimeStatus | null>(
  () => statuses.value.find((status) => status.ruleId === selectedId.value) ?? null,
);
const selectedState = computed<RequestForwardRuntimeState>(
  () => selectedStatus.value?.state ?? "stopped",
);
const readonly = computed(
  () => Boolean(selectedRule.value) && isRequestForwardRuleReadonly(selectedState.value),
);
const hasActiveRuntimeRule = computed(() =>
  statuses.value.some((status) => isRequestForwardRuleReadonly(status.state)),
);
const hasEditor = computed(() => draft.value || Boolean(selectedRule.value));

const stateCopy = computed(() => {
  const labels: Record<RequestForwardRuntimeState, string> = {
    stopped: "已停止",
    starting: "启动中",
    running: "运行中",
    stopping: "停止中",
    failed: "运行失败",
  };
  return labels[selectedState.value];
});

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function syncFormFromSelection() {
  if (!draft.value && !formDirty.value && selectedRule.value) {
    form.value = { ...selectedRule.value };
  }
}

async function refreshRules(options: { showLoading?: boolean } = {}) {
  const requestToken = ++refreshRequestToken;
  const intentToken = selectionIntentToken;
  try {
    const request = Promise.all([
      invoke<RuleListEnvelope>("tool:request-forward:list", {}),
      invoke<StatusListEnvelope>("tool:request-forward:status", {}),
    ]);
    if (options.showLoading) loading.value = true;
    const [ruleResult, statusResult] = await request;
    if (requestToken !== refreshRequestToken) return;
    rules.value = ruleResult.items;
    statuses.value = statusResult.items;
    if (selectionIntentToken !== intentToken || draft.value) return;
    const retained = rules.value.some((rule) => rule.id === selectedId.value);
    selectedId.value = retained ? selectedId.value : rules.value[0]?.id ?? null;
    syncFormFromSelection();
  } catch (error) {
    if (options.showLoading) ElMessage.error(`加载转发规则失败：${errorMessage(error)}`);
  } finally {
    if (options.showLoading && requestToken === refreshRequestToken) loading.value = false;
  }
}

function selectRule(id: number) {
  selectionIntentToken += 1;
  draft.value = false;
  selectedId.value = id;
  formDirty.value = false;
  fieldErrors.value = {};
  syncFormFromSelection();
}

function createDraft() {
  selectionIntentToken += 1;
  draft.value = true;
  selectedId.value = null;
  form.value = getDefaultRequestForwardForm();
  formDirty.value = false;
  fieldErrors.value = {};
}

function handleFormUpdate(value: RequestForwardRuleForm) {
  form.value = value;
  formDirty.value = true;
}

function validateForm(): boolean {
  const labels: Partial<Record<keyof RequestForwardRuleForm, string>> = {
    name: "请输入规则名称",
    bindHost: "请输入有效的 IPv4 或 IPv6 地址",
    listenPort: "监听端口必须为 1 到 65535 的整数",
    targetUrl: "请输入不含查询参数和片段的 HTTP/HTTPS 目标地址",
    targetHost: "请输入目标主机",
    targetPort: "目标端口必须为 1 到 65535 的整数",
  };
  const invalidFields = validateRequestForwardRuleForm(form.value);
  fieldErrors.value = Object.fromEntries(
    invalidFields.map((field) => [field, labels[field as keyof RequestForwardRuleForm] ?? "字段无效"]),
  );
  if (invalidFields.length) {
    ElMessage.error("请修正表单中的错误后再保存");
    return false;
  }
  return true;
}

async function saveRule(): Promise<RequestForwardRule | null> {
  if (readonly.value) {
    ElMessage.warning("运行中的规则不能修改，请先停止规则");
    return null;
  }
  if (!validateForm()) return null;
  saving.value = true;
  try {
    const payload = toRequestForwardRuleWriteInput(form.value);
    const result = draft.value
      ? await invoke<RuleEnvelope>("tool:request-forward:create", payload)
      : await invoke<RuleEnvelope>("tool:request-forward:update", {
          id: selectedRule.value!.id,
          ...payload,
        });
    draft.value = false;
    selectedId.value = result.item.id;
    selectionIntentToken += 1;
    form.value = { ...result.item };
    formDirty.value = false;
    await refreshRules();
    ElMessage.success("规则已保存");
    return result.item;
  } catch (error) {
    ElMessage.error(`保存规则失败：${errorMessage(error)}`);
    return null;
  } finally {
    saving.value = false;
  }
}

async function saveAndStart() {
  const saved = await saveRule();
  if (!saved) return;
  try {
    await startRule(saved.id, false);
    ElMessage.success("规则已保存并启动");
  } catch (error) {
    ElMessage.error(`规则已保存，但启动失败：${errorMessage(error)}`);
  }
}

async function startRule(id: number, feedback = true) {
  operating.value = true;
  try {
    const result = await invoke<StatusEnvelope>("tool:request-forward:start", { id });
    statuses.value = upsertStatus(statuses.value, result.item);
    await refreshRules();
    if (feedback) ElMessage.success("规则已启动");
  } catch (error) {
    if (feedback) ElMessage.error(`启动规则失败：${errorMessage(error)}`);
    if (!feedback) throw error;
  } finally {
    operating.value = false;
  }
}

async function stopRule(id: number, feedback = true) {
  operating.value = true;
  try {
    const result = await invoke<StatusEnvelope>("tool:request-forward:stop", { id });
    statuses.value = upsertStatus(statuses.value, result.item);
    await refreshRules();
    if (feedback) ElMessage.success("规则已停止");
  } catch (error) {
    if (feedback) ElMessage.error(`停止规则失败：${errorMessage(error)}`);
    if (!feedback) throw error;
  } finally {
    operating.value = false;
  }
}

async function handleStopAndEdit() {
  if (!selectedRule.value) return;
  try {
    await stopRule(selectedRule.value.id, false);
    ElMessage.success("规则已停止，可以编辑");
  } catch (error) {
    ElMessage.error(`停止失败，规则仍保持只读：${errorMessage(error)}`);
  }
}

async function runBatch(operation: "start" | "stop") {
  operating.value = true;
  try {
    const channel = `tool:request-forward:${operation}-all`;
    const result = await invoke<BatchEnvelope>(channel, {});
    const summary = {
      requested: result.results.length,
      succeeded: result.results.filter((item) => item.ok).length,
      failed: result.results.filter((item) => !item.ok).length,
    };
    const message = getRequestForwardBatchMessage(operation, summary);
    summary.failed ? ElMessage.warning(message) : ElMessage.success(message);
    await refreshRules();
  } catch (error) {
    ElMessage.error(`${operation === "start" ? "全部启动" : "全部停止"}失败：${errorMessage(error)}`);
  } finally {
    operating.value = false;
  }
}

async function deleteSelected() {
  const rule = selectedRule.value;
  if (!rule) return;
  if (isRequestForwardRuleReadonly(selectedState.value)) {
    ElMessage.warning("运行中的规则不能删除，请先停止规则");
    return;
  }
  try {
    await ElMessageBox.confirm(
      `确定删除规则“${rule.name}”吗？删除后无法恢复。`,
      "删除转发规则",
      { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" },
    );
  } catch {
    return;
  }
  operating.value = true;
  try {
    await invoke<{ ok: boolean }>("tool:request-forward:delete", { id: rule.id });
    selectionIntentToken += 1;
    selectedId.value = null;
    formDirty.value = false;
    await refreshRules();
    ElMessage.success("规则已删除");
  } catch (error) {
    ElMessage.error(`删除规则失败：${errorMessage(error)}`);
  } finally {
    operating.value = false;
  }
}

function upsertStatus(
  current: RequestForwardRuntimeStatus[],
  next: RequestForwardRuntimeStatus,
): RequestForwardRuntimeStatus[] {
  return [...current.filter((item) => item.ruleId !== next.ruleId), next];
}

function clearPolling() {
  pollGeneration += 1;
  if (pollTimer) clearTimeout(pollTimer);
  pollTimer = undefined;
}

function schedulePolling(generation: number) {
  pollTimer = setTimeout(() => void runPoll(generation), 2_000);
}

async function runPoll(generation: number) {
  if (
    generation !== pollGeneration ||
    pollInFlight ||
    !hasActiveRuntimeRule.value
  ) {
    return;
  }
  pollTimer = undefined;
  pollInFlight = true;
  try {
    await refreshRules();
  } finally {
    pollInFlight = false;
    if (generation === pollGeneration && hasActiveRuntimeRule.value) {
      schedulePolling(generation);
    }
  }
}

function syncPolling(active: boolean) {
  clearPolling();
  if (active) schedulePolling(pollGeneration);
}

watch(hasActiveRuntimeRule, syncPolling, { immediate: true });
onMounted(() => void refreshRules({ showLoading: true }));
onUnmounted(() => {
  refreshRequestToken += 1;
  clearPolling();
});
</script>

<template>
  <div class="request-forward-panel">
    <RequestForwardRuleList
      :rules="rules"
      :statuses="statuses"
      :selected-id="selectedId"
      :loading="loading"
      :busy="operating || saving"
      @add="createDraft"
      @select="selectRule"
      @start="startRule"
      @stop="stopRule"
      @start-all="runBatch('start')"
      @stop-all="runBatch('stop')"
    />

    <main class="rule-workbench">
      <template v-if="hasEditor">
        <header class="workbench-header">
          <div>
            <p class="workbench-header__eyebrow">{{ draft ? "NEW RULE" : `RULE #${selectedRule?.id}` }}</p>
            <h1>{{ draft ? "新建转发规则" : selectedRule?.name }}</h1>
            <p>{{ draft ? "新规则默认保持停止，保存后可按需启动。" : "配置本地监听端点与转发目标。" }}</p>
          </div>
          <div v-if="!draft" class="runtime-state" :class="`is-${selectedState}`">
            <span>{{ stateCopy }}</span>
            <small v-if="selectedStatus?.lastError">{{ selectedStatus.lastError }}</small>
          </div>
        </header>

        <div v-if="readonly" class="readonly-banner" role="status">
          <div>
            <strong>规则正在运行，配置已锁定</strong>
            <span>停止成功后才会解除只读，避免运行配置与持久化配置不一致。</span>
          </div>
          <el-button :loading="operating" @click="handleStopAndEdit">停止并编辑</el-button>
        </div>

        <div class="workbench-scroll">
          <RequestForwardRuleFormEditor
            :model-value="form"
            :readonly="readonly"
            :persisted="!draft"
            :errors="fieldErrors"
            @update:model-value="handleFormUpdate"
          />
        </div>

        <footer class="workbench-actions">
          <el-button
            v-if="!draft"
            type="danger"
            plain
            :disabled="readonly || operating"
            @click="deleteSelected"
          >
            删除规则
          </el-button>
          <span class="workbench-actions__spacer" />
          <el-button :disabled="readonly || operating" :loading="saving" @click="saveRule">
            仅保存
          </el-button>
          <el-button
            type="primary"
            :disabled="readonly || operating"
            :loading="saving"
            @click="saveAndStart"
          >
            保存并启动
          </el-button>
        </footer>
      </template>

      <div v-else class="workbench-empty">
        <div class="workbench-empty__mark">RF</div>
        <h1>选择或新建转发规则</h1>
        <p>在左侧选择已有规则查看配置，或新建 HTTP、TCP、UDP 转发规则。</p>
        <el-button type="primary" @click="createDraft">新建规则</el-button>
      </div>
    </main>
  </div>
</template>

<style scoped>
.request-forward-panel {
  display: grid;
  grid-template-columns: minmax(280px, 34%) minmax(0, 1fr);
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: #fff;
}

.rule-workbench {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  background: #fdfdfd;
}

.workbench-header {
  display: flex;
  flex: none;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  padding: 22px 26px 18px;
  border-bottom: 1px solid #e4e7eb;
}

.workbench-header h1,
.workbench-empty h1 {
  margin: 3px 0 5px;
  color: var(--text-primary, #1f2937);
  font-size: 20px;
}

.workbench-header p,
.workbench-empty p {
  margin: 0;
  color: var(--text-secondary, #64748b);
  font-size: 13px;
  line-height: 1.55;
}

.workbench-header__eyebrow {
  color: var(--el-color-primary, #409eff) !important;
  font-size: 10px !important;
  font-weight: 800;
  letter-spacing: 0.12em;
}

.runtime-state {
  display: grid;
  max-width: 300px;
  justify-items: end;
  gap: 4px;
  color: #6b7280;
  text-align: right;
}

.runtime-state span { font-size: 13px; font-weight: 700; }
.runtime-state span::before { content: "●"; margin-right: 6px; font-size: 9px; }
.runtime-state small { color: #c23b35; line-height: 1.45; }
.runtime-state.is-running { color: #168357; }
.runtime-state.is-starting,
.runtime-state.is-stopping { color: #a86608; }
.runtime-state.is-failed { color: #c23b35; }

.readonly-banner {
  display: flex;
  flex: none;
  align-items: center;
  gap: 18px;
  margin: 14px 26px 0;
  padding: 11px 12px;
  border: 1px solid #ecd6a9;
  border-radius: 6px;
  background: #fffaf0;
}

.readonly-banner > div { display: grid; min-width: 0; gap: 3px; margin-right: auto; }
.readonly-banner strong { color: #65450d; font-size: 13px; }
.readonly-banner span { color: #85672f; font-size: 12px; line-height: 1.45; }

.workbench-scroll {
  min-width: 0;
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding: 18px 26px 24px;
}

.workbench-actions {
  display: flex;
  flex: none;
  gap: 10px;
  padding: 14px 26px;
  border-top: 1px solid #e4e7eb;
  background: #fff;
}

.workbench-actions__spacer { flex: 1; }

.workbench-empty {
  display: grid;
  max-width: 440px;
  margin: auto;
  justify-items: center;
  padding: 30px;
  text-align: center;
}

.workbench-empty__mark {
  display: grid;
  width: 54px;
  height: 54px;
  margin-bottom: 12px;
  place-items: center;
  border: 1px solid #cfd6df;
  border-radius: 8px;
  background: #f6f8fa;
  color: #4c5d72;
  font-size: 15px;
  font-weight: 800;
  letter-spacing: 0.08em;
}

.workbench-empty .el-button { margin-top: 18px; }

@media (max-width: 780px) {
  .request-forward-panel {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    overflow: hidden;
  }
  .workbench-header { padding: 18px; }
  .readonly-banner { margin-inline: 18px; align-items: flex-start; }
  .workbench-scroll { padding-inline: 18px; }
  .workbench-actions { padding-inline: 18px; flex-wrap: wrap; }
}

@media (prefers-reduced-motion: reduce) {
  :deep(*) { scroll-behavior: auto !important; transition-duration: 0.01ms !important; }
}
</style>
