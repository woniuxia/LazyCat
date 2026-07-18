<template>
  <section class="network-quick-probe" aria-labelledby="quick-probe-title">
    <section class="quick-section favorites-section" aria-labelledby="quick-favorites-title">
      <header class="section-header">
        <div class="section-title-row">
          <el-icon><Star /></el-icon>
          <h2 id="quick-favorites-title">常用目标</h2>
          <span class="section-count">{{ networkFavorites.length }}</span>
        </div>
      </header>

      <div v-if="networkFavorites.length" class="favorite-list">
        <div v-for="item in networkFavorites" :key="item.id" class="favorite-item">
          <button type="button" class="favorite-select" @click="applyFavorite(item)">
            <strong>{{ item.name }}</strong>
            <span>{{ favoriteTargetText(item) }}</span>
          </button>
          <el-button
            class="favorite-delete"
            :icon="Delete"
            text
            :aria-label="`删除收藏 ${item.name}`"
            @click="removeFavorite(item.id)"
          />
        </div>
      </div>
      <p v-else class="section-empty">暂无收藏，可从最近测试记录中保存常用目标。</p>
    </section>

    <section class="quick-section probe-console" aria-labelledby="quick-probe-title">
      <header class="section-header probe-header">
        <h2 id="quick-probe-title">单项探测</h2>
        <div class="protocol-segmented" aria-label="单项探测协议">
          <button
            v-for="item in PROTOCOLS"
            :key="item.value"
            type="button"
            :class="{ 'is-active': protocol === item.value }"
            :aria-pressed="protocol === item.value"
            :disabled="loading"
            @click="protocol = item.value"
          >
            {{ item.label }}
          </button>
        </div>
      </header>

      <form class="probe-form" @submit.prevent="runTest">
        <label class="form-field host-field">
          <span>主机地址</span>
          <input
            v-model="host"
            type="text"
            autocomplete="off"
            spellcheck="false"
            placeholder="127.0.0.1、::1 或 example.com"
            :disabled="loading"
          />
        </label>

        <label v-if="protocol !== 'ping'" class="form-field port-field">
          <span>端口</span>
          <input
            v-model.number="port"
            type="number"
            inputmode="numeric"
            min="1"
            max="65535"
            :disabled="loading"
          />
        </label>

        <label class="form-field timeout-field">
          <span>超时</span>
          <div class="timeout-input">
            <input
              v-model.number="timeoutMs"
              type="number"
              inputmode="numeric"
              min="100"
              max="10000"
              step="100"
              :disabled="loading"
            />
            <span>ms</span>
          </div>
        </label>

        <el-button
          class="probe-submit"
          type="primary"
          native-type="submit"
          :loading="loading"
          :icon="loading ? Loading : VideoPlay"
        >
          {{ loading ? "探测中" : "开始探测" }}
        </el-button>
      </form>

      <div v-if="protocol !== 'ping'" class="quick-ports" aria-label="常用端口">
        <button
          v-for="item in QUICK_PORTS"
          :key="item.port"
          type="button"
          :disabled="loading"
          @click="applyQuickPort(item.port)"
        >
          <strong>{{ item.port }}</strong>
          <span>{{ item.name }}</span>
        </button>
      </div>

      <div class="result-region" aria-live="polite" aria-atomic="true">
        <div v-if="loading" class="probe-progress" role="status">
          <el-icon class="is-loading"><Loading /></el-icon>
          <span>正在探测 {{ testTarget }}</span>
        </div>

        <article
          v-else-if="result && resultPresentation"
          class="probe-result"
          :class="`is-${resultPresentation.outcome}`"
        >
          <div class="result-status">
            <el-icon v-if="resultPresentation.outcome === 'success'"><CircleCheck /></el-icon>
            <el-icon v-else-if="resultPresentation.outcome === 'unverified'"
              ><WarningFilled
            /></el-icon>
            <el-icon v-else><CircleCloseFilled /></el-icon>
            <div>
              <strong>{{ resultPresentation.label }}</strong>
              <span>{{ testProtocol.toUpperCase() }} · {{ testTarget }}</span>
            </div>
          </div>
          <div class="result-latency">
            <strong>{{ result.latencyMs }}</strong>
            <span>ms</span>
          </div>
          <p v-if="resultPresentation.detail" class="result-detail">
            {{ resultPresentation.detail }}
          </p>
          <dl v-if="testProtocol === 'ping' && result.packetsSent != null" class="ping-details">
            <div>
              <dt>已发送</dt>
              <dd>{{ result.packetsSent }}</dd>
            </div>
            <div>
              <dt>已接收</dt>
              <dd>{{ result.packetsReceived ?? 0 }}</dd>
            </div>
            <div>
              <dt>丢包率</dt>
              <dd>{{ result.packetLoss ?? 0 }}%</dd>
            </div>
          </dl>
          <time class="result-time" :datetime="new Date(lastCheckedAt).toISOString()">
            {{ formatTime(lastCheckedAt) }}
          </time>
        </article>

        <div v-else class="probe-empty">选择协议并输入目标后开始探测。</div>
      </div>
    </section>

    <section class="quick-section history-section" aria-labelledby="quick-history-title">
      <header class="section-header history-header">
        <div class="section-title-row">
          <el-icon><Refresh /></el-icon>
          <h2 id="quick-history-title">最近测试记录</h2>
          <span class="section-count">{{ history.length }}</span>
        </div>
        <el-button v-if="history.length" size="small" text @click="clearHistory">清空</el-button>
      </header>

      <div class="stats-grid">
        <div class="stat-item">
          <span>总次数</span>
          <strong>{{ historyStats.total }}</strong>
        </div>
        <div class="stat-item is-success">
          <span>成功</span>
          <strong>{{ historyStats.success }}</strong>
        </div>
        <div class="stat-item is-failed">
          <span>失败</span>
          <strong>{{ historyStats.failed }}</strong>
        </div>
        <div class="stat-item is-unverified">
          <span>无法判断</span>
          <strong>{{ historyStats.unverified }}</strong>
        </div>
        <div class="stat-item">
          <span>确认成功率</span>
          <strong>{{ historyStats.successRate }}</strong>
        </div>
      </div>

      <div class="history-toolbar">
        <label>
          <span class="sr-only">按协议筛选</span>
          <select v-model="historyProtocolFilter">
            <option value="all">全部协议</option>
            <option value="ping">PING</option>
            <option value="tcp">TCP</option>
            <option value="udp">UDP</option>
          </select>
        </label>
        <label>
          <span class="sr-only">按结果筛选</span>
          <select v-model="historyResultFilter">
            <option value="all">全部结果</option>
            <option value="success">仅成功</option>
            <option value="failed">仅失败</option>
            <option value="unverified">无法判断</option>
          </select>
        </label>
        <label class="history-search">
          <span class="sr-only">搜索测试记录</span>
          <input v-model="historyKeyword" type="search" placeholder="搜索目标或错误" />
        </label>
        <el-button
          v-if="filteredFailedHistory.length"
          size="small"
          :loading="retryingFailed"
          :disabled="loading"
          @click="retryFailedHistory"
        >
          重测失败项
        </el-button>
      </div>

      <div v-if="filteredHistory.length" class="history-list">
        <article
          v-for="item in filteredHistory"
          :key="item.id"
          class="history-row"
          :class="`is-${historyOutcome(item)}`"
        >
          <button type="button" class="history-reuse" @click="reuseHistory(item)">
            <span class="history-state">
              <el-icon v-if="historyOutcome(item) === 'success'"><CircleCheck /></el-icon>
              <el-icon v-else-if="historyOutcome(item) === 'unverified'"><WarningFilled /></el-icon>
              <el-icon v-else><CircleCloseFilled /></el-icon>
            </span>
            <span class="history-main">
              <strong>{{ item.target }}</strong>
              <small>{{ item.protocol.toUpperCase() }} · {{ formatTime(item.checkedAt) }}</small>
            </span>
            <span class="history-result">
              <strong>{{ historyStatusLabel(item) }}</strong>
              <small>{{ item.latencyMs }} ms</small>
            </span>
          </button>
          <div class="history-actions">
            <el-button
              size="small"
              text
              :disabled="isHistoryFavoriteSaved(item)"
              @click="saveHistoryFavorite(item)"
            >
              {{ isHistoryFavoriteSaved(item) ? "已收藏" : "收藏" }}
            </el-button>
            <el-button size="small" text :disabled="loading" @click="retryHistory(item)">
              复测
            </el-button>
            <el-button
              size="small"
              text
              type="danger"
              :aria-label="`删除记录 ${item.target}`"
              @click="removeHistory(item.id)"
            >
              删除
            </el-button>
          </div>
          <p v-if="item.error || item.note" class="history-error">
            {{ item.error || item.note }}
          </p>
        </article>
      </div>
      <p v-else-if="history.length" class="section-empty">没有符合当前筛选条件的记录。</p>
      <p v-else class="section-empty">暂无测试记录。</p>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  CircleCheck,
  CircleCloseFilled,
  Delete,
  Loading,
  Refresh,
  Star,
  VideoPlay,
  WarningFilled,
} from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../../bridge/tauri";
import { getSettingJson, setSettingJson } from "../../composables/useSettings";
import {
  addNetworkFavorite,
  buildNetworkFavorite,
  favoriteToNetworkForm,
  isSameNetworkFavoriteTarget,
  type NetworkFavoriteItem,
  type NetworkFavoriteProtocol,
} from "../../utils/networkFavorites";
import {
  migrateNetworkDiagnosticsSettings,
  NETWORK_DIAGNOSTICS_SETTINGS_KEY,
  type NetworkDiagnosticsSettings,
  type NetworkHistorySummary,
} from "../../utils/networkDiagnosticsPersistence";

type Protocol = NetworkFavoriteProtocol;
type QuickProbeOutcome = "success" | "failed" | "unverified";
type HistoryResultFilter = "all" | QuickProbeOutcome;

interface QuickProbeResult {
  reachable: boolean;
  latencyMs: number;
  error?: string | null;
  note?: string | null;
  packetLoss?: number;
  packetsSent?: number;
  packetsReceived?: number;
  latencies?: number[];
}

interface ResultPresentation {
  outcome: QuickProbeOutcome;
  label: string;
  detail: string;
}

const NETWORK_HISTORY_KEY = "network_test_history";
const NETWORK_FAVORITES_KEY = "network_test_favorites";

const PROTOCOLS: Array<{ value: Protocol; label: string }> = [
  { value: "ping", label: "PING" },
  { value: "tcp", label: "TCP" },
  { value: "udp", label: "UDP" },
];

const QUICK_PORTS = [
  { port: 22, name: "SSH" },
  { port: 80, name: "HTTP" },
  { port: 443, name: "HTTPS" },
  { port: 3306, name: "MySQL" },
  { port: 6379, name: "Redis" },
  { port: 8080, name: "Dev" },
];

const protocol = ref<Protocol>("ping");
const host = ref("127.0.0.1");
const port = ref(80);
const timeoutMs = ref(2000);
const loading = ref(false);
const retryingFailed = ref(false);
const result = ref<QuickProbeResult | null>(null);
const testProtocol = ref<Protocol>("ping");
const testTarget = ref("");
const lastCheckedAt = ref(0);
const initialSettings = loadNetworkDiagnosticsSettings();
const history = ref<NetworkHistorySummary[]>(initialSettings.history);
const networkFavorites = ref<NetworkFavoriteItem[]>(initialSettings.favorites);
const historyProtocolFilter = ref<"all" | Protocol>("all");
const historyResultFilter = ref<HistoryResultFilter>("all");
const historyKeyword = ref("");

const resultPresentation = computed<ResultPresentation | null>(() => {
  if (!result.value) return null;
  return presentResult(testProtocol.value, result.value);
});

const filteredHistory = computed(() => {
  const keyword = historyKeyword.value.trim().toLowerCase();
  return history.value.filter((item) => {
    if (historyProtocolFilter.value !== "all" && item.protocol !== historyProtocolFilter.value) {
      return false;
    }
    if (historyResultFilter.value !== "all" && historyOutcome(item) !== historyResultFilter.value) {
      return false;
    }
    if (!keyword) return true;
    return (
      item.target.toLowerCase().includes(keyword) ||
      (item.error ?? "").toLowerCase().includes(keyword) ||
      (item.note ?? "").toLowerCase().includes(keyword)
    );
  });
});

const filteredFailedHistory = computed(() =>
  filteredHistory.value.filter((item) => historyOutcome(item) === "failed"),
);

const historyStats = computed(() => {
  const outcomes = history.value.map(historyOutcome);
  const success = outcomes.filter((value) => value === "success").length;
  const failed = outcomes.filter((value) => value === "failed").length;
  const unverified = outcomes.filter((value) => value === "unverified").length;
  const verified = success + failed;
  return {
    total: outcomes.length,
    success,
    failed,
    unverified,
    successRate: verified ? `${Math.round((success / verified) * 100)}%` : "-",
  };
});

watch(protocol, () => {
  if (!loading.value) result.value = null;
});

function isProtocol(value: unknown): value is Protocol {
  return value === "ping" || value === "tcp" || value === "udp";
}

function isOutcome(value: unknown): value is QuickProbeOutcome {
  return value === "success" || value === "failed" || value === "unverified";
}

function loadNetworkDiagnosticsSettings(): NetworkDiagnosticsSettings {
  const result = migrateNetworkDiagnosticsSettings({
    current: getSettingJson<unknown>(NETWORK_DIAGNOSTICS_SETTINGS_KEY, null),
    legacyFavorites: getSettingJson<unknown>(NETWORK_FAVORITES_KEY, []),
    legacyHistory: getSettingJson<unknown>(NETWORK_HISTORY_KEY, []),
  });
  if (result.migrated) setSettingJson(NETWORK_DIAGNOSTICS_SETTINGS_KEY, result.settings);
  return result.settings;
}

function persistSettings() {
  const current = loadNetworkDiagnosticsSettings();
  setSettingJson(NETWORK_DIAGNOSTICS_SETTINGS_KEY, {
    ...current,
    favorites: networkFavorites.value,
    history: history.value,
  });
}

function persistHistory() {
  setSettingJson(NETWORK_HISTORY_KEY, history.value);
  persistSettings();
}

function persistFavorites() {
  setSettingJson(NETWORK_FAVORITES_KEY, networkFavorites.value);
  persistSettings();
}
function appendHistory(item: Omit<NetworkHistorySummary, "id" | "checkedAt">) {
  history.value = [
    {
      id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
      checkedAt: Date.now(),
      ...item,
    },
    ...history.value,
  ].slice(0, 50);
  persistHistory();
}

function normalizeHost(value: string): string {
  const trimmed = value.trim();
  if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

function formatSocketTarget(value: string, targetPort: number): string {
  const normalized = normalizeHost(value);
  return normalized.includes(":") ? `[${normalized}]:${targetPort}` : `${normalized}:${targetPort}`;
}

function hostForSocketAction(value: string): string {
  const normalized = normalizeHost(value);
  return normalized.includes(":") ? `[${normalized}]` : normalized;
}

function parseSocketTarget(value: string): { host: string; port: number } | null {
  const target = value.trim();
  const bracketed = /^\[([^\]]+)]:(\d+)$/.exec(target);
  if (bracketed) {
    const parsedPort = Number(bracketed[2]);
    return validPort(parsedPort) ? { host: bracketed[1], port: parsedPort } : null;
  }
  const separator = target.lastIndexOf(":");
  if (separator <= 0) return null;
  const parsedPort = Number(target.slice(separator + 1));
  if (!validPort(parsedPort)) return null;
  return { host: normalizeHost(target.slice(0, separator)), port: parsedPort };
}

function validPort(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 65535;
}

function validTimeout(value: number): boolean {
  return Number.isFinite(value) && value >= 100 && value <= 10000;
}

function applyQuickPort(nextPort: number) {
  port.value = nextPort;
}

function presentResult(currentProtocol: Protocol, current: QuickProbeResult): ResultPresentation {
  if (currentProtocol === "udp") {
    if (current.note) {
      return {
        outcome: "unverified",
        label: "无响应，无法判断",
        detail: current.note,
      };
    }
    if (current.reachable) {
      return { outcome: "success", label: "收到响应", detail: "目标返回了 UDP 数据。" };
    }
    const explicitRejection = /不可达|拒绝|refused|unreachable|icmp/i.test(current.error ?? "");
    return {
      outcome: "failed",
      label: explicitRejection ? "明确不可达" : "探测失败",
      detail: current.error ?? "UDP 探测失败。",
    };
  }
  return {
    outcome: current.reachable ? "success" : "failed",
    label: current.reachable ? "可达" : "不可达",
    detail: current.error ?? "",
  };
}

function historyOutcome(item: NetworkHistorySummary): QuickProbeOutcome {
  if (item.outcome) return item.outcome;
  if (item.protocol === "udp" && item.reachable) return "unverified";
  return item.reachable ? "success" : "failed";
}

function historyStatusLabel(item: NetworkHistorySummary): string {
  const outcome = historyOutcome(item);
  if (item.protocol === "udp") {
    if (outcome === "success") return "收到响应";
    if (outcome === "unverified") return "无法判断";
    return /不可达|拒绝|refused|unreachable|icmp/i.test(item.error ?? "")
      ? "明确不可达"
      : "探测失败";
  }
  return outcome === "success" ? "可达" : "不可达";
}

function reuseHistory(item: NetworkHistorySummary): boolean {
  protocol.value = item.protocol;
  timeoutMs.value = item.timeoutMs;
  if (item.protocol === "ping") {
    host.value = normalizeHost(item.target);
    return true;
  }
  const parsed = parseSocketTarget(item.target);
  if (!parsed) {
    ElMessage.warning("历史目标格式无效，无法回填端口");
    return false;
  }
  host.value = parsed.host;
  port.value = parsed.port;
  return true;
}

async function retryHistory(item: NetworkHistorySummary) {
  if (!reuseHistory(item)) return;
  await runTest();
}

async function retryFailedHistory() {
  if (loading.value || retryingFailed.value) return;
  const targets = filteredFailedHistory.value.slice(0, 10);
  if (!targets.length) return;
  retryingFailed.value = true;
  let recovered = 0;
  try {
    for (const item of targets) {
      await retryHistory(item);
      if (resultPresentation.value?.outcome === "success") recovered += 1;
    }
    ElMessage.success(`批量复测完成，共 ${targets.length} 项，恢复 ${recovered} 项`);
  } finally {
    retryingFailed.value = false;
  }
}

function buildFavoriteForHistory(item: NetworkHistorySummary, name: string): NetworkFavoriteItem {
  if (item.protocol === "ping") {
    return buildNetworkFavorite(
      {
        protocol: item.protocol,
        host: normalizeHost(item.target),
        port: 80,
        timeoutMs: item.timeoutMs,
      },
      name,
    );
  }
  const parsed = parseSocketTarget(item.target);
  if (!parsed) throw new Error("历史目标格式无效");
  return buildNetworkFavorite(
    { protocol: item.protocol, host: parsed.host, port: parsed.port, timeoutMs: item.timeoutMs },
    name,
  );
}

function isHistoryFavoriteSaved(item: NetworkHistorySummary): boolean {
  try {
    const candidate = buildFavoriteForHistory(item, "");
    return networkFavorites.value.some((favorite) =>
      isSameNetworkFavoriteTarget(favorite, candidate),
    );
  } catch {
    return false;
  }
}

async function saveHistoryFavorite(item: NetworkHistorySummary) {
  if (isHistoryFavoriteSaved(item)) return;
  let draft: NetworkFavoriteItem;
  try {
    draft = buildFavoriteForHistory(item, "");
  } catch (error) {
    ElMessage.warning(errorMessage(error));
    return;
  }
  try {
    const { value } = await ElMessageBox.prompt("为这个目标命名", "添加收藏", {
      confirmButtonText: "保存",
      cancelButtonText: "取消",
      inputValue: draft.name,
      inputPlaceholder: "例如 生产 Redis",
      inputValidator: (name: string) => name.trim().length > 0 || "请输入收藏名称",
    });
    networkFavorites.value = addNetworkFavorite(
      networkFavorites.value,
      buildFavoriteForHistory(item, value),
    );
    persistFavorites();
    ElMessage.success("已加入收藏");
  } catch (error) {
    if (error === "cancel" || error === "close") return;
    ElMessage.error(errorMessage(error));
  }
}

function favoriteTargetText(item: NetworkFavoriteItem): string {
  const form = favoriteToNetworkForm(item);
  return form.protocol === "ping"
    ? `PING ${normalizeHost(form.host)}`
    : `${form.protocol.toUpperCase()} ${formatSocketTarget(form.host, form.port)}`;
}

function applyFavorite(item: NetworkFavoriteItem) {
  const form = favoriteToNetworkForm(item);
  protocol.value = form.protocol;
  host.value = normalizeHost(form.host);
  port.value = form.port;
  timeoutMs.value = form.timeoutMs;
  result.value = null;
}

function removeFavorite(id: string) {
  networkFavorites.value = networkFavorites.value.filter((item) => item.id !== id);
  persistFavorites();
}

function removeHistory(id: string) {
  history.value = history.value.filter((item) => item.id !== id);
  persistHistory();
}

function clearHistory() {
  history.value = [];
  persistHistory();
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function runTest() {
  if (loading.value) return;
  const normalizedHost = normalizeHost(host.value);
  if (!normalizedHost) {
    ElMessage.warning("请输入主机地址");
    return;
  }
  if (protocol.value !== "ping" && !validPort(Number(port.value))) {
    ElMessage.warning("端口范围必须是 1-65535");
    return;
  }
  if (!validTimeout(Number(timeoutMs.value))) {
    ElMessage.warning("超时时间必须是 100-10000 ms");
    return;
  }

  const currentProtocol = protocol.value;
  testProtocol.value = currentProtocol;
  testTarget.value =
    currentProtocol === "ping" ? normalizedHost : formatSocketTarget(normalizedHost, port.value);
  result.value = null;
  loading.value = true;

  try {
    let data: unknown;
    if (currentProtocol === "tcp") {
      data = await invokeToolByChannel("tool:network:tcp-test", {
        host: hostForSocketAction(normalizedHost),
        port: port.value,
        timeoutMs: timeoutMs.value,
      });
    } else if (currentProtocol === "udp") {
      data = await invokeToolByChannel("tool:network:udp-test", {
        host: hostForSocketAction(normalizedHost),
        port: port.value,
        timeoutMs: timeoutMs.value,
      });
    } else {
      data = await invokeToolByChannel("tool:network:ping-test", {
        host: normalizedHost,
        timeoutMs: timeoutMs.value,
        count: 3,
      });
    }

    const next = data as QuickProbeResult;
    const normalizedResult: QuickProbeResult = {
      ...next,
      reachable: Boolean(next.reachable),
      latencyMs: Number(next.latencyMs ?? 0),
      error: next.error ?? null,
      note: next.note ?? null,
    };
    result.value = normalizedResult;
    lastCheckedAt.value = Date.now();
    const presentation = presentResult(currentProtocol, normalizedResult);
    appendHistory({
      protocol: currentProtocol,
      target: testTarget.value,
      timeoutMs: timeoutMs.value,
      reachable: normalizedResult.reachable,
      latencyMs: normalizedResult.latencyMs,
      statusCode: null,
      error: normalizedResult.error ?? null,
      note: normalizedResult.note ?? null,
      outcome: presentation.outcome,
    });
  } catch (error) {
    const failedResult: QuickProbeResult = {
      reachable: false,
      latencyMs: 0,
      error: errorMessage(error),
      note: null,
    };
    result.value = failedResult;
    lastCheckedAt.value = Date.now();
    appendHistory({
      protocol: currentProtocol,
      target: testTarget.value,
      timeoutMs: timeoutMs.value,
      reachable: false,
      latencyMs: 0,
      statusCode: null,
      error: failedResult.error ?? null,
      note: null,
      outcome: "failed",
    });
  } finally {
    loading.value = false;
  }
}

function formatTime(timestamp: number): string {
  if (!timestamp) return "-";
  const date = new Date(timestamp);
  const now = new Date();
  const time = [date.getHours(), date.getMinutes(), date.getSeconds()]
    .map((value) => String(value).padStart(2, "0"))
    .join(":");
  if (date.toDateString() === now.toDateString()) return time;
  return `${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")} ${time}`;
}
</script>

<style scoped>
.network-quick-probe {
  display: grid;
  gap: 12px;
  min-width: 0;
}

.quick-section {
  min-width: 0;
  padding: 14px;
  border: 1px solid var(--lc-border);
  border-radius: 8px;
  background: var(--lc-surface-0);
}

.section-header,
.section-title-row,
.probe-header,
.history-toolbar,
.result-status,
.history-actions {
  display: flex;
  align-items: center;
}

.section-header {
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.section-title-row {
  gap: 7px;
  min-width: 0;
  color: var(--lc-text-secondary);
}

.section-header h2 {
  margin: 0;
  color: var(--lc-text);
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0;
}

.section-count {
  display: inline-grid;
  min-width: 22px;
  height: 20px;
  place-items: center;
  border-radius: 6px;
  background: var(--lc-surface-2);
  color: var(--lc-text-secondary);
  font-size: 11px;
}

.section-empty,
.probe-empty {
  margin: 0;
  padding: 18px;
  color: var(--lc-text-secondary);
  font-size: 13px;
  text-align: center;
}

.favorite-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 7px;
}

.favorite-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 32px;
  align-items: center;
  border: 1px solid var(--lc-border);
  border-radius: 7px;
  background: var(--lc-surface-1);
}

.favorite-select {
  display: grid;
  min-width: 0;
  gap: 3px;
  padding: 8px 10px;
  border: 0;
  background: transparent;
  color: var(--lc-text);
  cursor: pointer;
  text-align: left;
}

.favorite-select strong,
.favorite-select span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.favorite-select strong {
  font-size: 12px;
}
.favorite-select span {
  color: var(--lc-text-secondary);
  font: 11px var(--lc-font-mono);
}
.favorite-delete {
  width: 28px;
}

.favorite-item:hover,
.favorite-item:focus-within {
  border-color: var(--lc-border-hover);
  background: var(--lc-surface-0);
}

.probe-header {
  align-items: flex-start;
}

.protocol-segmented {
  display: inline-flex;
  flex: none;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--lc-border);
  border-radius: 7px;
  background: var(--lc-surface-2);
}

.protocol-segmented button {
  min-width: 58px;
  min-height: 28px;
  padding: 0 10px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--lc-text-secondary);
  cursor: pointer;
  font-size: 12px;
  font-weight: 650;
}

.protocol-segmented button.is-active {
  background: var(--lc-surface-0);
  color: var(--lc-accent);
  box-shadow: var(--lc-shadow-sm);
}

.protocol-segmented button:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.probe-form {
  display: grid;
  grid-template-columns: minmax(190px, 1fr) 110px 140px auto;
  align-items: end;
  gap: 9px;
}

.form-field {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.form-field > span {
  color: var(--lc-text-secondary);
  font-size: 11px;
  font-weight: 600;
}

.form-field input,
.history-toolbar input,
.history-toolbar select {
  width: 100%;
  height: 34px;
  min-width: 0;
  box-sizing: border-box;
  border: 1px solid var(--lc-border-hover);
  border-radius: 6px;
  outline: 0;
  background: var(--lc-surface-0);
  color: var(--lc-text);
  font: 12px var(--lc-font-body);
}

.form-field input,
.history-toolbar input {
  padding: 0 10px;
}
.history-toolbar select {
  padding: 0 28px 0 9px;
}

.form-field input:focus-visible,
.history-toolbar input:focus-visible,
.history-toolbar select:focus-visible,
.favorite-select:focus-visible,
.protocol-segmented button:focus-visible,
.quick-ports button:focus-visible,
.history-reuse:focus-visible {
  outline: 2px solid var(--lc-accent);
  outline-offset: 2px;
}

.timeout-input {
  position: relative;
}

.timeout-input input {
  padding-right: 34px;
}
.timeout-input span {
  position: absolute;
  top: 50%;
  right: 9px;
  color: var(--lc-text-muted);
  font: 10px var(--lc-font-mono);
  transform: translateY(-50%);
}

.probe-submit {
  min-width: 112px;
}

.quick-ports {
  display: grid;
  grid-template-columns: repeat(6, minmax(64px, 1fr));
  gap: 6px;
  margin-top: 10px;
}

.quick-ports button {
  display: grid;
  gap: 2px;
  min-height: 42px;
  place-items: center;
  border: 1px solid var(--lc-border);
  border-radius: 6px;
  background: var(--lc-surface-1);
  color: var(--lc-text);
  cursor: pointer;
}

.quick-ports button:hover:not(:disabled) {
  border-color: var(--lc-border-hover);
  background: var(--lc-accent-dim);
}

.quick-ports strong {
  font: 600 12px var(--lc-font-mono);
}
.quick-ports span {
  color: var(--lc-text-secondary);
  font-size: 10px;
}

.result-region {
  min-height: 76px;
  margin-top: 10px;
  border-top: 1px solid var(--lc-border-subtle);
  padding-top: 10px;
}

.probe-progress {
  display: flex;
  min-height: 66px;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--lc-accent);
  font: 12px var(--lc-font-mono);
}

.probe-result {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px 16px;
  padding: 11px 12px;
  border: 1px solid var(--lc-border);
  border-left-width: 3px;
  border-radius: 7px;
  background: var(--lc-surface-1);
}

.probe-result.is-success {
  border-left-color: var(--lc-success);
}
.probe-result.is-failed {
  border-left-color: var(--lc-danger);
}
.probe-result.is-unverified {
  border-left-color: var(--lc-warning);
}
.probe-result.is-success .result-status > .el-icon {
  color: var(--lc-success);
}
.probe-result.is-failed .result-status > .el-icon {
  color: var(--lc-danger);
}
.probe-result.is-unverified .result-status > .el-icon {
  color: #a86608;
}

.result-status {
  gap: 9px;
  min-width: 0;
}
.result-status > .el-icon {
  flex: none;
  font-size: 19px;
}
.result-status div {
  display: grid;
  min-width: 0;
  gap: 2px;
}
.result-status strong {
  color: var(--lc-text);
  font-size: 13px;
}
.result-status span {
  overflow-wrap: anywhere;
  color: var(--lc-text-secondary);
  font: 11px var(--lc-font-mono);
}

.result-latency {
  display: flex;
  align-items: baseline;
  gap: 3px;
  color: var(--lc-text-secondary);
}

.result-latency strong {
  color: var(--lc-text);
  font: 600 20px var(--lc-font-mono);
}
.result-latency span {
  font-size: 10px;
}
.result-detail {
  grid-column: 1 / -1;
  margin: 0;
  color: var(--lc-text-secondary);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.ping-details {
  display: flex;
  grid-column: 1 / -1;
  gap: 18px;
  margin: 0;
}

.ping-details div {
  display: flex;
  gap: 5px;
}
.ping-details dt {
  color: var(--lc-text-secondary);
  font-size: 11px;
}
.ping-details dd {
  margin: 0;
  color: var(--lc-text);
  font: 600 11px var(--lc-font-mono);
}
.result-time {
  grid-column: 1 / -1;
  color: var(--lc-text-muted);
  font-size: 10px;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 6px;
  margin-bottom: 10px;
}

.stat-item {
  display: grid;
  gap: 3px;
  padding: 7px 9px;
  border: 1px solid var(--lc-border-subtle);
  border-radius: 6px;
  background: var(--lc-surface-1);
}

.stat-item span {
  color: var(--lc-text-secondary);
  font-size: 10px;
}
.stat-item strong {
  color: var(--lc-text);
  font: 600 15px var(--lc-font-mono);
}
.stat-item.is-success strong {
  color: #168357;
}
.stat-item.is-failed strong {
  color: #b83c36;
}
.stat-item.is-unverified strong {
  color: #a86608;
}

.history-toolbar {
  gap: 7px;
  margin-bottom: 10px;
}

.history-toolbar label {
  min-width: 110px;
}
.history-search {
  flex: 1;
}

.history-list {
  display: grid;
  max-height: 420px;
  gap: 6px;
  overflow-y: auto;
}

.history-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  border: 1px solid var(--lc-border);
  border-left-width: 3px;
  border-radius: 7px;
  background: var(--lc-surface-1);
}

.history-row.is-success {
  border-left-color: var(--lc-success);
}
.history-row.is-failed {
  border-left-color: var(--lc-danger);
}
.history-row.is-unverified {
  border-left-color: var(--lc-warning);
}

.history-reuse {
  display: grid;
  grid-template-columns: 20px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 8px 10px;
  border: 0;
  background: transparent;
  color: var(--lc-text);
  cursor: pointer;
  text-align: left;
}

.history-state {
  font-size: 15px;
}
.history-row.is-success .history-state {
  color: #168357;
}
.history-row.is-failed .history-state {
  color: #b83c36;
}
.history-row.is-unverified .history-state {
  color: #a86608;
}

.history-main,
.history-result {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.history-main strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font: 12px var(--lc-font-mono);
}
.history-main small,
.history-result small {
  color: var(--lc-text-secondary);
  font-size: 10px;
}
.history-result {
  justify-items: end;
}
.history-result strong {
  font-size: 11px;
}
.history-actions {
  gap: 1px;
  padding-right: 5px;
}
.history-error {
  grid-column: 1 / -1;
  margin: 0;
  padding: 0 10px 8px 38px;
  color: var(--lc-text-secondary);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

@media (max-width: 840px) {
  .probe-form {
    grid-template-columns: minmax(0, 1fr) 100px 130px;
  }
  .probe-submit {
    grid-column: 1 / -1;
    justify-self: end;
  }
  .quick-ports {
    grid-template-columns: repeat(3, minmax(64px, 1fr));
  }
  .stats-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

@media (max-width: 600px) {
  .probe-header {
    align-items: stretch;
    flex-direction: column;
  }
  .protocol-segmented {
    align-self: flex-start;
  }
  .probe-form {
    grid-template-columns: minmax(0, 1fr) 100px;
  }
  .host-field {
    grid-column: 1 / -1;
  }
  .timeout-field {
    grid-column: 1 / -1;
  }
  .probe-submit {
    width: 100%;
  }
  .stats-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .history-toolbar {
    align-items: stretch;
    flex-direction: column;
  }
  .history-toolbar label {
    width: 100%;
  }
  .history-row {
    grid-template-columns: minmax(0, 1fr);
  }
  .history-actions {
    justify-content: flex-end;
    padding: 0 6px 6px;
  }
}

@media (prefers-reduced-motion: reduce) {
  :deep(*) {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}
</style>
