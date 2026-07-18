<template>
  <section class="diagnosis-workspace" :aria-busy="isBusy">
    <div class="target-bar">
      <div class="target-field">
        <label for="diagnosis-target">目标地址</label>
        <input
          id="diagnosis-target"
          v-model="target"
          type="text"
          autocomplete="off"
          spellcheck="false"
          placeholder="example.com、https://example.com/path 或 [2001:db8::1]:443"
          :disabled="isBusy"
          @keyup.enter="startDiagnosis"
        />
      </div>
      <button
        v-if="isRunning"
        type="button"
        class="action-button danger"
        :disabled="cancelling"
        @click="cancelDiagnosis"
      >
        <el-icon><VideoPause /></el-icon>
        {{ cancelling ? "正在取消" : "取消诊断" }}
      </button>
      <button
        v-else
        type="button"
        class="action-button primary"
        :disabled="starting"
        @click="startDiagnosis"
      >
        <el-icon :class="{ rotating: starting }"
          ><Loading v-if="starting" /><VideoPlay v-else
        /></el-icon>
        {{ starting ? "正在启动" : "开始诊断" }}
      </button>
    </div>

    <button
      type="button"
      class="advanced-toggle"
      :aria-expanded="advancedOpen"
      aria-controls="diagnosis-advanced"
      :disabled="isBusy"
      @click="advancedOpen = !advancedOpen"
    >
      <el-icon><ArrowDown v-if="advancedOpen" /><ArrowRight v-else /></el-icon>
      高级参数
      <span v-if="activeOverrideCount" class="override-count">{{ activeOverrideCount }}</span>
    </button>

    <div v-show="advancedOpen" id="diagnosis-advanced" class="advanced-grid">
      <label class="form-field">
        <span>默认协议</span>
        <select v-model="defaultProtocol" :disabled="isBusy">
          <option value="https">HTTPS</option>
          <option value="http">HTTP</option>
        </select>
      </label>
      <label class="form-field">
        <span>连接 IP</span>
        <input
          v-model="connectionIp"
          type="text"
          placeholder="留空使用解析结果"
          :disabled="isBusy"
        />
      </label>
      <label class="form-field">
        <span>TLS SNI</span>
        <input v-model="sni" type="text" placeholder="留空使用目标域名" :disabled="isBusy" />
      </label>
      <label class="form-field">
        <span>证书校验名</span>
        <input
          v-model="verifyHostname"
          type="text"
          placeholder="留空使用目标主机"
          :disabled="isBusy"
        />
      </label>
      <label class="form-field">
        <span>HTTP Host</span>
        <input v-model="httpHost" type="text" placeholder="留空自动生成" :disabled="isBusy" />
      </label>
      <label class="form-field">
        <span>指定 DNS</span>
        <input v-model="dnsServers" type="text" placeholder="1.1.1.1, 8.8.8.8" :disabled="isBusy" />
      </label>
      <label class="form-field">
        <span>代理画像</span>
        <select v-model="proxyProfile" :disabled="isBusy">
          <option value="auto">自动决策</option>
          <option value="environment">环境变量</option>
          <option value="windows_user">WinINET</option>
          <option value="winhttp">WinHTTP</option>
          <option value="direct">强制直连</option>
        </select>
      </label>
      <label class="form-field">
        <span>单步超时</span>
        <div class="number-input">
          <input
            v-model.number="stepTimeoutMs"
            type="number"
            min="500"
            max="60000"
            step="500"
            :disabled="isBusy"
          />
          <span>ms</span>
        </div>
      </label>
      <label class="form-field">
        <span>整体超时</span>
        <div class="number-input">
          <input
            v-model.number="overallTimeoutMs"
            type="number"
            min="1000"
            max="300000"
            step="1000"
            :disabled="isBusy"
          />
          <span>ms</span>
        </div>
      </label>
    </div>

    <div class="live-region" aria-live="polite" aria-atomic="true">
      {{ liveMessage }}
    </div>

    <div v-if="runError" class="run-error" role="alert">
      <el-icon><WarningFilled /></el-icon>
      <div>
        <strong>诊断未能继续</strong>
        <span>{{ runError }}</span>
      </div>
      <button v-if="!isRunning" type="button" class="text-button" @click="startDiagnosis">
        重新运行
      </button>
    </div>

    <template v-if="snapshot">
      <header class="run-header">
        <div class="run-state">
          <span class="state-dot" :class="'tone-' + runTone" />
          <div>
            <strong>{{ runStatusText }}</strong>
            <span>序号 {{ snapshot.sequence }} · {{ elapsedText }}</span>
          </div>
        </div>
        <code :title="snapshot.runId">{{ snapshot.runId }}</code>
      </header>

      <section class="normalized-strip" aria-label="本次诊断参数">
        <div>
          <span>请求 URL</span>
          <strong>{{ snapshot.report.input.url }}</strong>
        </div>
        <div>
          <span>连接目标</span>
          <strong>{{ connectionTargetText }}</strong>
        </div>
        <div>
          <span>TLS</span>
          <strong>{{ tlsTargetText }}</strong>
        </div>
        <div>
          <span>HTTP Host</span>
          <strong>{{ snapshot.report.input.httpHost }}</strong>
        </div>
      </section>

      <section class="summary-band" aria-labelledby="diagnosis-summary-title">
        <div class="section-heading">
          <div>
            <span class="section-kicker">SUMMARY</span>
            <h3 id="diagnosis-summary-title">
              {{ snapshot.report.conclusions.length ? "诊断结论" : "步骤汇总" }}
            </h3>
          </div>
          <div class="summary-actions">
            <span class="summary-count"
              >{{ completedStepCount }}/{{ snapshot.report.steps.length }} 已结束</span
            >
            <button type="button" class="text-button" :disabled="isRunning" @click="copyReport">
              <el-icon><CopyDocument /></el-icon>
              复制报告
            </button>
            <button type="button" class="text-button" :disabled="isRunning" @click="exportReport">
              <el-icon><Download /></el-icon>
              导出报告
            </button>
          </div>
        </div>
        <div v-if="snapshot.report.conclusions.length" class="conclusion-list">
          <div
            v-for="conclusion in snapshot.report.conclusions"
            :key="conclusion.id"
            class="conclusion-item"
            :class="'severity-' + conclusion.severity"
          >
            <span>{{ conclusionSeverityLabel(conclusion.severity) }}</span>
            <p>{{ conclusion.message }}</p>
          </div>
        </div>
        <p v-else class="summary-copy">{{ stepSummaryText }}</p>
        <div v-if="snapshot.report.recommendations.length" class="recommendation-list">
          <div v-for="item in snapshot.report.recommendations" :key="item.id">
            <strong>{{ item.title }}</strong>
            <span>{{ item.action }}</span>
          </div>
        </div>
      </section>

      <section class="steps-section" aria-labelledby="diagnosis-steps-title">
        <div class="section-heading">
          <div>
            <span class="section-kicker">ACCESS PATH</span>
            <h3 id="diagnosis-steps-title">链路步骤</h3>
          </div>
        </div>
        <ol class="step-list">
          <li v-for="(step, index) in snapshot.report.steps" :key="step.id" class="step-item">
            <div class="step-rail" aria-hidden="true">
              <span :class="'tone-' + stepTone(step)">{{ index + 1 }}</span>
              <i v-if="index < snapshot.report.steps.length - 1" />
            </div>
            <div class="step-body">
              <div class="step-heading">
                <div>
                  <strong>{{ stepName(step.id) }}</strong>
                  <span>{{ stepDescription(step.id) }}</span>
                </div>
                <div class="step-states">
                  <span class="lifecycle-badge">{{ lifecycleLabel(step.lifecycle) }}</span>
                  <span v-if="step.outcome" class="outcome-badge" :class="'tone-' + step.outcome">
                    {{ outcomeLabel(step.outcome) }}
                  </span>
                </div>
              </div>
              <div v-if="step.error" class="step-error" role="status">
                <strong>{{ step.error.code }}</strong>
                <span>{{ step.error.message }}</span>
                <em>{{ step.error.retriable ? "可重试" : "不可重试" }}</em>
              </div>
              <details v-if="evidenceForStep(step.id).length" class="evidence-details">
                <summary>证据 {{ evidenceForStep(step.id).length }} 项</summary>
                <div class="evidence-list">
                  <article v-for="evidence in evidenceForStep(step.id)" :key="evidence.id">
                    <header>
                      <strong>{{ evidence.kind }}</strong>
                      <time v-if="evidence.observedAt">{{
                        formatTimestamp(evidence.observedAt)
                      }}</time>
                    </header>
                    <pre>{{ formatEvidence(evidence.value) }}</pre>
                  </article>
                </div>
              </details>
            </div>
          </li>
        </ol>
      </section>
    </template>

    <div v-else-if="starting" class="workspace-empty loading-empty">
      <el-icon class="rotating"><Loading /></el-icon>
      <strong>正在建立诊断任务</strong>
    </div>
    <div v-else class="workspace-empty">
      <el-icon><Connection /></el-icon>
      <strong>等待诊断目标</strong>
    </div>
  </section>
</template>

<script setup lang="ts">
import {
  ArrowDown,
  ArrowRight,
  Connection,
  CopyDocument,
  Download,
  Loading,
  VideoPause,
  VideoPlay,
  WarningFilled,
} from "@element-plus/icons-vue";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { ElMessage } from "element-plus";
import {
  diagnosisCancel,
  invokeToolByChannel,
  diagnosisGet,
  diagnosisStart,
  onAccessPathDiagnosisSnapshot,
} from "../../bridge/tauri";
import type {
  AccessPathConclusionSeverity,
  AccessPathDiagnosisRunSnapshot,
  AccessPathEvidence,
  AccessPathJsonValue,
  AccessPathProtocol,
  AccessPathProxyProfile,
  AccessPathStepId,
} from "../../types/access-path-diagnostics";
import { normalizeAccessPathInput } from "../../utils/accessPathInput";
import { formatAccessPathReport } from "../../utils/accessPathReport";
import {
  appendDiagnosticReport,
  migrateNetworkDiagnosticsSettings,
  NETWORK_DIAGNOSTICS_SETTINGS_KEY,
} from "../../utils/networkDiagnosticsPersistence";
import { getSettingJson, setSettingJson } from "../../composables/useSettings";
import {
  acceptDiagnosisSnapshot,
  diagnosisStatusLabel,
  lifecycleLabel,
  outcomeLabel,
  parseDnsServerList,
  stepTone,
} from "../../utils/accessPathDiagnosticsView";

const POLL_INTERVAL_MS = 1200;
const TERMINAL_LIFECYCLES = new Set(["completed", "blocked", "skipped", "cancelled"]);

const target = ref("");
const defaultProtocol = ref<AccessPathProtocol>("https");
const connectionIp = ref("");
const sni = ref("");
const verifyHostname = ref("");
const httpHost = ref("");
const dnsServers = ref("");
const proxyProfile = ref<AccessPathProxyProfile>("auto");
const stepTimeoutMs = ref(5000);
const overallTimeoutMs = ref(30000);
const advancedOpen = ref(false);
const starting = ref(false);
const cancelling = ref(false);
const activeRunId = ref<string | null>(null);
const snapshot = ref<AccessPathDiagnosisRunSnapshot | null>(null);
const runError = ref("");

const pendingSnapshots = new Map<string, AccessPathDiagnosisRunSnapshot>();
let pollTimer: ReturnType<typeof setTimeout> | undefined;
let pollGeneration = 0;
let unlisten: (() => void) | null = null;
let listenerPromise: Promise<void> | null = null;
let disposed = false;
const persistedReportIds = new Set<string>();

const isRunning = computed(() => snapshot.value?.status === "running");
const isBusy = computed(() => starting.value || cancelling.value || isRunning.value);
const activeOverrideCount = computed(
  () =>
    [connectionIp.value, sni.value, verifyHostname.value, httpHost.value, dnsServers.value].filter(
      (value) => value.trim().length > 0,
    ).length +
    (proxyProfile.value === "auto" ? 0 : 1) +
    (defaultProtocol.value === "https" ? 0 : 1),
);
const runStatusText = computed(() =>
  snapshot.value ? diagnosisStatusLabel(snapshot.value.status) : "等待",
);
const runTone = computed(() => {
  const status = snapshot.value?.status;
  if (status === "running") return "running";
  if (status === "completed") return "success";
  if (status === "cancelled") return "cancelled";
  if (status === "timed_out") return "warning";
  return "failed";
});
const completedStepCount = computed(
  () =>
    snapshot.value?.report.steps.filter((step) => TERMINAL_LIFECYCLES.has(step.lifecycle)).length ??
    0,
);
const elapsedText = computed(() => {
  if (!snapshot.value) return "";
  const start = Date.parse(snapshot.value.report.startedAt);
  const end = snapshot.value.report.finishedAt
    ? Date.parse(snapshot.value.report.finishedAt)
    : Date.now();
  if (!Number.isFinite(start) || !Number.isFinite(end)) return "耗时未知";
  return "耗时 " + Math.max(0, end - start) + " ms";
});
const connectionTargetText = computed(() => {
  const input = snapshot.value?.report.input;
  if (!input) return "";
  const host = input.connectionIp ?? input.hostname;
  const authority = host.includes(":") ? "[" + host + "]" : host;
  return authority + ":" + input.port;
});
const tlsTargetText = computed(() => {
  const input = snapshot.value?.report.input;
  if (!input) return "";
  if (input.protocol !== "https") return "不适用";
  return "SNI " + (input.sni ?? "无") + " · 校验 " + input.verifyHostname;
});
const stepSummaryText = computed(() => {
  if (!snapshot.value) return "";
  const steps = snapshot.value.report.steps;
  const success = steps.filter((step) => step.outcome === "success").length;
  const warning = steps.filter((step) => step.outcome === "warning").length;
  const failed = steps.filter((step) => step.outcome === "failed").length;
  const unverified = steps.filter((step) => step.outcome === "unverified").length;
  return (
    "成功 " +
    success +
    "，警告 " +
    warning +
    "，失败 " +
    failed +
    "，无法验证 " +
    unverified +
    "。各步骤结果和原始错误见下方证据。"
  );
});
const liveMessage = computed(() => {
  if (runError.value) return "诊断错误：" + runError.value;
  if (starting.value) return "正在启动访问链路诊断";
  if (cancelling.value) return "正在取消访问链路诊断";
  if (!snapshot.value) return "";
  const activeStep = snapshot.value.report.steps.find((step) => step.lifecycle === "running");
  return activeStep
    ? runStatusText.value + "，当前步骤：" + stepName(activeStep.id)
    : runStatusText.value + "，已结束 " + completedStepCount.value + " 个步骤";
});

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function clampTimeout(value: number, minimum: number, maximum: number, label: string): number {
  if (!Number.isFinite(value) || !Number.isInteger(value) || value < minimum || value > maximum) {
    throw new Error(label + "必须在 " + minimum + " 到 " + maximum + " ms 之间");
  }
  return value;
}

function cacheSnapshot(incoming: AccessPathDiagnosisRunSnapshot): void {
  const current = pendingSnapshots.get(incoming.runId);
  if (!current || incoming.sequence > current.sequence)
    pendingSnapshots.set(incoming.runId, incoming);
}

function applySnapshot(incoming: AccessPathDiagnosisRunSnapshot): void {
  const accepted = acceptDiagnosisSnapshot(snapshot.value, incoming, activeRunId.value);
  if (accepted === snapshot.value) return;
  snapshot.value = accepted;
  if (accepted?.status !== "running") {
    stopPolling();
    persistReport(accepted.report);
  }
}

function handleSnapshotEvent(event: {
  runId: string;
  sequence: number;
  snapshot: AccessPathDiagnosisRunSnapshot;
}): void {
  if (event.runId !== event.snapshot.runId || event.sequence !== event.snapshot.sequence) return;
  if (event.runId === activeRunId.value) {
    applySnapshot(event.snapshot);
    return;
  }
  if (starting.value) cacheSnapshot(event.snapshot);
}

function ensureListener(): Promise<void> {
  if (unlisten) return Promise.resolve();
  if (listenerPromise) return listenerPromise;
  listenerPromise = onAccessPathDiagnosisSnapshot(handleSnapshotEvent)
    .then((stop) => {
      if (disposed) {
        stop();
        return;
      }
      unlisten = stop;
    })
    .finally(() => {
      listenerPromise = null;
    });
  return listenerPromise;
}

function stopPolling(): void {
  pollGeneration += 1;
  if (pollTimer) clearTimeout(pollTimer);
  pollTimer = undefined;
}

function schedulePoll(runId: string, generation: number): void {
  if (disposed || generation !== pollGeneration || activeRunId.value !== runId) return;
  pollTimer = setTimeout(async () => {
    try {
      const latest = await diagnosisGet(runId);
      if (generation !== pollGeneration || activeRunId.value !== runId) return;
      applySnapshot(latest);
    } catch (error) {
      if (generation !== pollGeneration || activeRunId.value !== runId) return;
      runError.value = errorMessage(error);
    }
    if (
      generation === pollGeneration &&
      activeRunId.value === runId &&
      (snapshot.value === null || isRunning.value)
    ) {
      schedulePoll(runId, generation);
    }
  }, POLL_INTERVAL_MS);
}

function beginPolling(runId: string): void {
  stopPolling();
  const generation = pollGeneration;
  schedulePoll(runId, generation);
}

async function startDiagnosis(): Promise<void> {
  if (starting.value || isRunning.value) return;
  runError.value = "";
  starting.value = true;
  cancelling.value = false;
  stopPolling();
  pendingSnapshots.clear();
  activeRunId.value = null;
  snapshot.value = null;
  try {
    await ensureListener();
    const input = normalizeAccessPathInput(target.value, {
      defaultProtocol: defaultProtocol.value,
      connectionIp: connectionIp.value || null,
      sni: sni.value || null,
      verifyHostname: verifyHostname.value || null,
      httpHost: httpHost.value || null,
    });
    const response = await diagnosisStart({
      input,
      overallTimeoutMs: clampTimeout(overallTimeoutMs.value, 1000, 300000, "整体超时"),
      stepTimeoutMs: clampTimeout(stepTimeoutMs.value, 500, 60000, "单步超时"),
      dnsServers: parseDnsServerList(dnsServers.value),
      proxyProfile: proxyProfile.value,
    });
    activeRunId.value = response.runId;
    const pending = pendingSnapshots.get(response.runId);
    if (pending) applySnapshot(pending);
    pendingSnapshots.clear();
    applySnapshot(await diagnosisGet(response.runId));
    if (isRunning.value) beginPolling(response.runId);
  } catch (error) {
    runError.value = errorMessage(error);
    if (activeRunId.value && (snapshot.value === null || isRunning.value)) {
      beginPolling(activeRunId.value);
    }
  } finally {
    starting.value = false;
  }
}

async function cancelDiagnosis(): Promise<void> {
  const runId = activeRunId.value;
  if (!runId || cancelling.value || !isRunning.value) return;
  cancelling.value = true;
  runError.value = "";
  try {
    const response = await diagnosisCancel(runId);
    if (response.runId === runId) applySnapshot(response.snapshot);
  } catch (error) {
    runError.value = errorMessage(error);
  } finally {
    cancelling.value = false;
  }
}

function persistReport(report: AccessPathDiagnosisRunSnapshot["report"]): void {
  if (persistedReportIds.has(report.reportId)) return;
  const migrated = migrateNetworkDiagnosticsSettings({
    current: getSettingJson<unknown>(NETWORK_DIAGNOSTICS_SETTINGS_KEY, null),
    legacyFavorites: getSettingJson<unknown>("network_test_favorites", []),
    legacyHistory: getSettingJson<unknown>("network_test_history", []),
  });
  setSettingJson(
    NETWORK_DIAGNOSTICS_SETTINGS_KEY,
    appendDiagnosticReport(migrated.settings, report),
  );
  persistedReportIds.add(report.reportId);
}

async function copyReport(): Promise<void> {
  if (!snapshot.value) return;
  try {
    await navigator.clipboard.writeText(formatAccessPathReport(snapshot.value.report));
    ElMessage.success("脱敏报告已复制");
  } catch (error) {
    ElMessage.error("复制失败：" + errorMessage(error));
  }
}

async function exportReport(): Promise<void> {
  if (!snapshot.value) return;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const filePath = await save({
      defaultPath: `access-path-report-${snapshot.value.report.reportId}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!filePath) return;
    await invokeToolByChannel("tool:file:write-text", {
      path: filePath,
      content: formatAccessPathReport(snapshot.value.report),
    });
    ElMessage.success("脱敏报告已导出");
  } catch (error) {
    ElMessage.error("导出失败：" + errorMessage(error));
  }
}
function evidenceForStep(stepId: AccessPathStepId): AccessPathEvidence[] {
  if (!snapshot.value) return [];
  const step = snapshot.value.report.steps.find((item) => item.id === stepId);
  if (!step) return [];
  const ids = new Set(step.evidenceIds);
  return snapshot.value.report.evidence.filter(
    (evidence) => evidence.stepId === stepId && (ids.size === 0 || ids.has(evidence.id)),
  );
}

function formatEvidence(value: AccessPathJsonValue): string {
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}

function formatTimestamp(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleTimeString("zh-CN", { hour12: false });
}

function stepName(id: AccessPathStepId): string {
  return {
    proxy: "代理决策",
    hosts: "Hosts",
    dns: "DNS 解析",
    tcp: "TCP 连接",
    tls: "TLS 握手",
    http: "HTTP 请求",
  }[id];
}

function stepDescription(id: AccessPathStepId): string {
  return {
    proxy: "配置来源与当前目标的直连 / 代理路径",
    hosts: "本机 Hosts 命中、重复、冲突和格式",
    dns: "系统解析与指定 DNS 查询结果",
    tcp: "逐地址族建立连接并保留原始错误",
    tls: "SNI、证书链、主机名校验与信任结果",
    http: "受限 HEAD / GET、重定向与响应状态",
  }[id];
}

function conclusionSeverityLabel(severity: AccessPathConclusionSeverity): string {
  return { info: "信息", warning: "警告", error: "错误" }[severity];
}

onMounted(() => {
  ensureListener().catch((error) => {
    runError.value = "无法监听诊断进度：" + errorMessage(error);
  });
});

onUnmounted(() => {
  disposed = true;
  stopPolling();
  const runId = activeRunId.value;
  if (runId && isRunning.value) void diagnosisCancel(runId).catch(() => undefined);
  if (unlisten) unlisten();
  unlisten = null;
});
</script>

<style scoped>
.diagnosis-workspace {
  color: #182230;
}

.target-bar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: end;
  gap: 12px;
  padding: 18px 20px 14px;
  border-bottom: 1px solid #e4eaf1;
  background: #fff;
}

.target-field,
.form-field {
  display: flex;
  flex-direction: column;
  gap: 7px;
  min-width: 0;
}

.target-field label,
.form-field > span {
  color: #475467;
  font-size: 12px;
  font-weight: 650;
}

input,
select {
  width: 100%;
  min-width: 0;
  height: 36px;
  box-sizing: border-box;
  border: 1px solid #cfd8e3;
  border-radius: 6px;
  outline: none;
  background: #fff;
  color: #182230;
  font: inherit;
  font-size: 13px;
  letter-spacing: 0;
  transition:
    border-color 0.15s ease,
    box-shadow 0.15s ease;
}

input {
  padding: 0 11px;
}

select {
  padding: 0 30px 0 10px;
}

input:focus,
select:focus {
  border-color: #0b78e3;
  box-shadow: 0 0 0 3px rgba(11, 120, 227, 0.14);
}

input:disabled,
select:disabled {
  background: #f4f6f8;
  color: #667085;
  cursor: not-allowed;
}

.target-field input {
  height: 40px;
  font-family: "Cascadia Code", Consolas, monospace;
}

button {
  font: inherit;
  letter-spacing: 0;
}

button:focus-visible,
summary:focus-visible {
  outline: 3px solid rgba(11, 120, 227, 0.24);
  outline-offset: 2px;
}

.action-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  min-width: 112px;
  height: 40px;
  padding: 0 15px;
  border: 1px solid transparent;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 650;
}

.action-button.primary {
  background: #0876d8;
  color: #fff;
}

.action-button.danger {
  border-color: #f2b8b5;
  background: #fff;
  color: #b42318;
}

.action-button:disabled {
  cursor: not-allowed;
  opacity: 0.64;
}

.advanced-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  height: 38px;
  padding: 0 20px;
  border: 0;
  border-bottom: 1px solid #e8edf3;
  background: #f8fafc;
  color: #344054;
  cursor: pointer;
  font-size: 12px;
  font-weight: 650;
  text-align: left;
}

.advanced-toggle:disabled {
  cursor: not-allowed;
  color: #98a2b3;
}

.override-count {
  display: inline-grid;
  place-items: center;
  min-width: 18px;
  height: 18px;
  border-radius: 9px;
  background: #dbeafe;
  color: #175cd3;
  font-size: 11px;
}

.advanced-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 14px 16px;
  padding: 16px 20px 18px;
  border-bottom: 1px solid #e4eaf1;
  background: #fbfcfd;
}

.number-input {
  position: relative;
}

.number-input input {
  padding-right: 38px;
}

.number-input span {
  position: absolute;
  top: 50%;
  right: 10px;
  color: #667085;
  font-size: 11px;
  transform: translateY(-50%);
  pointer-events: none;
}

.live-region {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  clip-path: inset(50%);
  white-space: nowrap;
}

.run-error {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  margin: 16px 20px 0;
  padding: 11px 12px;
  border: 1px solid #f4c7c3;
  border-radius: 6px;
  background: #fff8f7;
  color: #b42318;
}

.run-error > div {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.run-error strong {
  font-size: 12px;
}

.run-error span {
  overflow-wrap: anywhere;
  font-size: 12px;
}

.text-button {
  padding: 4px 6px;
  border: 0;
  background: transparent;
  color: #0876d8;
  cursor: pointer;
  font-size: 12px;
  font-weight: 650;
}

.run-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 18px 20px 12px;
}

.run-state {
  display: flex;
  align-items: center;
  gap: 10px;
}

.run-state > div {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.run-state strong {
  font-size: 14px;
}

.run-state span:not(.state-dot) {
  color: #667085;
  font-size: 11px;
}

.state-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #98a2b3;
}

.state-dot.tone-running {
  background: #0876d8;
  box-shadow: 0 0 0 4px #dbeafe;
}

.state-dot.tone-success {
  background: #168755;
  box-shadow: 0 0 0 4px #dff4e9;
}

.state-dot.tone-warning,
.state-dot.tone-cancelled {
  background: #d97706;
  box-shadow: 0 0 0 4px #fef0c7;
}

.state-dot.tone-failed {
  background: #d92d20;
  box-shadow: 0 0 0 4px #fee4e2;
}

.run-header code {
  max-width: 42%;
  overflow: hidden;
  color: #667085;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.normalized-strip {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  margin: 0 20px;
  border-top: 1px solid #e4eaf1;
  border-bottom: 1px solid #e4eaf1;
  background: #f8fafc;
}

.normalized-strip > div {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
  padding: 11px 12px;
  border-right: 1px solid #e4eaf1;
}

.normalized-strip > div:last-child {
  border-right: 0;
}

.normalized-strip span {
  color: #667085;
  font-size: 10px;
  font-weight: 650;
  text-transform: uppercase;
}

.normalized-strip strong {
  overflow: hidden;
  color: #263244;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.summary-band,
.steps-section {
  padding: 20px;
}

.summary-band {
  border-bottom: 1px solid #e4eaf1;
}

.section-heading {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.section-kicker {
  display: block;
  margin-bottom: 3px;
  color: #0876d8;
  font-size: 9px;
  font-weight: 750;
}

.section-heading h3 {
  margin: 0;
  color: #182230;
  font-size: 15px;
  letter-spacing: 0;
}

.summary-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.summary-actions .text-button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.summary-count {
  color: #667085;
  font-size: 11px;
}

.summary-copy {
  margin: 0;
  color: #475467;
  font-size: 12px;
  line-height: 1.7;
}

.conclusion-list,
.recommendation-list {
  display: grid;
  gap: 8px;
}

.conclusion-item {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr);
  gap: 9px;
  padding: 9px 10px;
  border-left: 3px solid #98a2b3;
  background: #f8fafc;
}

.conclusion-item.severity-warning {
  border-color: #d97706;
}

.conclusion-item.severity-error {
  border-color: #d92d20;
}

.conclusion-item span {
  color: #667085;
  font-size: 11px;
  font-weight: 650;
}

.conclusion-item p {
  margin: 0;
  font-size: 12px;
}

.recommendation-list {
  margin-top: 12px;
}

.recommendation-list > div {
  display: grid;
  grid-template-columns: minmax(120px, 0.3fr) minmax(0, 1fr);
  gap: 12px;
  padding-top: 9px;
  border-top: 1px dashed #d8e0e9;
  font-size: 12px;
}

.recommendation-list span {
  color: #475467;
}

.step-list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.step-item {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr);
}

.step-rail {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.step-rail span {
  display: grid;
  place-items: center;
  flex: 0 0 26px;
  width: 26px;
  border: 1px solid #d4dce6;
  border-radius: 50%;
  background: #fff;
  color: #667085;
  font-size: 11px;
  font-weight: 700;
}

.step-rail span.tone-running {
  border-color: #58a6ed;
  background: #eaf4ff;
  color: #0876d8;
}

.step-rail span.tone-success {
  border-color: #7bc6a3;
  background: #eaf8f1;
  color: #117549;
}

.step-rail span.tone-warning,
.step-rail span.tone-unverified {
  border-color: #e9b949;
  background: #fff8e1;
  color: #a15c07;
}

.step-rail span.tone-failed,
.step-rail span.tone-blocked,
.step-rail span.tone-cancelled {
  border-color: #ef9a94;
  background: #fff0ef;
  color: #b42318;
}

.step-rail i {
  width: 1px;
  min-height: 38px;
  flex: 1;
  background: #dce3eb;
}

.step-body {
  min-width: 0;
  padding: 2px 0 16px 8px;
}

.step-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  min-height: 34px;
}

.step-heading > div:first-child {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.step-heading strong {
  font-size: 13px;
}

.step-heading > div:first-child span {
  color: #667085;
  font-size: 11px;
}

.step-states {
  display: flex;
  flex: 0 0 auto;
  gap: 6px;
}

.lifecycle-badge,
.outcome-badge {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 7px;
  border: 1px solid #d8e0e9;
  border-radius: 4px;
  background: #f8fafc;
  color: #475467;
  font-size: 10px;
  font-weight: 650;
}

.outcome-badge.tone-success {
  border-color: #b7dfcb;
  background: #edf9f3;
  color: #117549;
}

.outcome-badge.tone-warning,
.outcome-badge.tone-unverified {
  border-color: #f0d38a;
  background: #fffae9;
  color: #9a5b08;
}

.outcome-badge.tone-failed {
  border-color: #f0b7b2;
  background: #fff2f1;
  color: #b42318;
}

.step-error {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: start;
  gap: 8px;
  margin-top: 9px;
  padding: 8px 9px;
  border-left: 3px solid #d92d20;
  background: #fff7f6;
  color: #b42318;
  font-size: 11px;
}

.step-error span {
  overflow-wrap: anywhere;
}

.step-error em {
  color: #667085;
  font-style: normal;
  white-space: nowrap;
}

.evidence-details {
  margin-top: 9px;
  border-top: 1px dashed #d8e0e9;
}

.evidence-details summary {
  width: max-content;
  padding: 8px 0 4px;
  color: #0876d8;
  cursor: pointer;
  font-size: 11px;
  font-weight: 650;
}

.evidence-list {
  display: grid;
  gap: 8px;
  padding-top: 4px;
}

.evidence-list article {
  min-width: 0;
  border: 1px solid #dfe5ec;
  border-radius: 6px;
  overflow: hidden;
  background: #fbfcfd;
}

.evidence-list header {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  padding: 7px 9px;
  border-bottom: 1px solid #e5eaf0;
  color: #475467;
  font-size: 10px;
}

.evidence-list time {
  color: #98a2b3;
}

.evidence-list pre {
  max-height: 280px;
  margin: 0;
  padding: 9px;
  overflow: auto;
  color: #263244;
  font:
    11px/1.55 "Cascadia Code",
    Consolas,
    monospace;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.workspace-empty {
  display: grid;
  place-items: center;
  align-content: center;
  min-height: 330px;
  color: #98a2b3;
}

.workspace-empty .el-icon {
  margin-bottom: 12px;
  font-size: 30px;
}

.workspace-empty strong {
  color: #475467;
  font-size: 13px;
}

.workspace-empty span {
  margin-top: 6px;
  font-size: 11px;
}

.loading-empty {
  min-height: 220px;
}

.rotating {
  animation: rotate 0.9s linear infinite;
}

@keyframes rotate {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 900px) {
  .advanced-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .normalized-strip {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .normalized-strip > div:nth-child(2) {
    border-right: 0;
  }

  .normalized-strip > div:nth-child(-n + 2) {
    border-bottom: 1px solid #e4eaf1;
  }
}

@media (max-width: 620px) {
  .target-bar {
    grid-template-columns: minmax(0, 1fr);
  }

  .action-button {
    width: 100%;
  }

  .advanced-grid,
  .normalized-strip {
    grid-template-columns: minmax(0, 1fr);
  }

  .normalized-strip > div {
    border-right: 0;
    border-bottom: 1px solid #e4eaf1;
  }

  .normalized-strip > div:last-child {
    border-bottom: 0;
  }

  .run-header,
  .step-heading {
    align-items: flex-start;
    flex-direction: column;
  }

  .run-header code {
    max-width: 100%;
  }

  .step-states {
    margin-top: 5px;
  }

  .step-error {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    scroll-behavior: auto !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
</style>
