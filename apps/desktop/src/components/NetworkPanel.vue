<template>
  <div class="network-panel">
    <div class="network-controls">
      <el-radio-group v-model="protocol" class="protocol-group">
        <el-radio-button value="tcp">TCP</el-radio-button>
        <el-radio-button value="http">HTTP</el-radio-button>
        <el-radio-button value="https">HTTPS</el-radio-button>
      </el-radio-group>

      <div v-if="protocol === 'tcp'" class="input-row">
        <el-input
          v-model="host"
          placeholder="主机地址，例如 127.0.0.1 或 example.com"
          style="flex: 1; min-width: 220px;"
          clearable
          @keyup.enter="runTest"
        />
        <el-input-number v-model="port" :min="1" :max="65535" placeholder="端口" style="width: 120px;" />
        <el-input-number
          v-model="timeoutMs"
          :min="100"
          :max="30000"
          :step="500"
          placeholder="超时(ms)"
          style="width: 140px;"
        />
        <el-button type="primary" :loading="loading" @click="runTest">测试连通性</el-button>
      </div>

      <div v-else class="input-row">
        <el-input
          v-model="httpUrl"
          :placeholder="protocol === 'https' ? 'https://example.com/health' : 'http://example.com/health'"
          style="flex: 1; min-width: 280px;"
          clearable
          @keyup.enter="runTest"
        />
        <el-input-number
          v-model="timeoutMs"
          :min="100"
          :max="30000"
          :step="500"
          placeholder="超时(ms)"
          style="width: 140px;"
        />
        <el-button type="primary" :loading="loading" @click="runTest">测试连通性</el-button>
      </div>

      <div class="quick-actions">
        <el-space wrap>
          <span class="quick-actions-label">常用：</span>
          <el-button size="small" @click="applyQuickTarget('127.0.0.1', 80)">localhost:80</el-button>
          <el-button size="small" @click="applyQuickTarget('127.0.0.1', 443)">localhost:443</el-button>
          <el-button size="small" @click="applyQuickTarget('127.0.0.1', 3306)">MySQL 3306</el-button>
          <el-button size="small" @click="applyQuickTarget('127.0.0.1', 6379)">Redis 6379</el-button>
          <el-button size="small" @click="applyQuickHttpPath('/health')">/health</el-button>
          <el-button size="small" @click="applyQuickHttpPath('/actuator/health')">/actuator/health</el-button>
        </el-space>
      </div>
    </div>

    <div v-if="loading" class="result-loading">
      <el-skeleton animated :rows="4" />
    </div>

    <div v-else-if="result" class="result-card" :class="result.reachable ? 'result-ok' : 'result-fail'">
      <div class="result-header">
        <span class="result-icon">{{ result.reachable ? "✓" : "✕" }}</span>
        <span class="result-status">{{ result.reachable ? "可达" : "不可达" }}</span>
        <el-tag size="small" :type="result.reachable ? 'success' : 'danger'">
          {{ result.latencyMs }} ms
        </el-tag>
        <el-tag
          v-if="result.statusCode != null"
          size="small"
          :type="statusTagType(result.statusCode)"
        >
          HTTP {{ result.statusCode }}
        </el-tag>
        <span class="result-time">{{ formatTime(lastCheckedAt) }}</span>
      </div>

      <el-descriptions :column="1" size="small" border style="margin-top: 12px;">
        <el-descriptions-item label="协议">{{ protocol.toUpperCase() }}</el-descriptions-item>
        <el-descriptions-item label="目标">{{ resultTargetText }}</el-descriptions-item>
        <el-descriptions-item label="超时">{{ timeoutMs }} ms</el-descriptions-item>
        <el-descriptions-item label="延迟">{{ result.latencyMs }} ms</el-descriptions-item>
        <el-descriptions-item v-if="result.statusCode != null" label="状态码">{{ result.statusCode }}</el-descriptions-item>
        <el-descriptions-item v-if="result.error" label="错误">
          <span class="error-text">{{ result.error }}</span>
        </el-descriptions-item>
      </el-descriptions>
    </div>

    <div v-else class="empty-hint">
      选择协议并输入目标地址，点击“测试连通性”查看结果
    </div>

    <div class="history-section">
      <el-divider content-position="left">最近测试</el-divider>
      <el-descriptions class="history-stats" :column="4" border size="small">
        <el-descriptions-item label="总次数">{{ historyStats.total }}</el-descriptions-item>
        <el-descriptions-item label="成功">{{ historyStats.success }}</el-descriptions-item>
        <el-descriptions-item label="失败">{{ historyStats.failed }}</el-descriptions-item>
        <el-descriptions-item label="成功率">{{ historyStats.successRate }}</el-descriptions-item>
      </el-descriptions>

      <div class="history-filters">
        <el-select v-model="historyProtocolFilter" style="width: 120px;">
          <el-option label="全部协议" value="all" />
          <el-option label="TCP" value="tcp" />
          <el-option label="HTTP" value="http" />
          <el-option label="HTTPS" value="https" />
        </el-select>
        <el-select v-model="historyResultFilter" style="width: 120px;">
          <el-option label="全部结果" value="all" />
          <el-option label="仅成功" value="success" />
          <el-option label="仅失败" value="failed" />
        </el-select>
        <el-input
          v-model="historyKeyword"
          placeholder="按目标/错误关键字过滤"
          clearable
          style="width: 240px;"
        />
        <el-button :disabled="filteredFailedHistory.length === 0 || retryingFailed" :loading="retryingFailed" @click="retryFailedHistory">
          批量重测失败项
        </el-button>
      </div>

      <el-table :data="filteredHistory" border stripe max-height="320">
        <el-table-column prop="checkedAt" label="时间" min-width="160">
          <template #default="{ row }">
            {{ formatTime(row.checkedAt) }}
          </template>
        </el-table-column>
        <el-table-column prop="protocol" label="协议" width="90" />
        <el-table-column prop="target" label="目标" min-width="260" show-overflow-tooltip />
        <el-table-column label="结果" width="110" align="center">
          <template #default="{ row }">
            <el-tag :type="row.reachable ? 'success' : 'danger'" size="small">
              {{ row.reachable ? "可达" : "失败" }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="latencyMs" label="延迟(ms)" width="100" />
        <el-table-column label="状态码" width="90" align="center">
          <template #default="{ row }">
            {{ row.statusCode ?? "-" }}
          </template>
        </el-table-column>
        <el-table-column prop="error" label="错误信息" min-width="180" show-overflow-tooltip>
          <template #default="{ row }">
            {{ row.error || "-" }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="148" fixed="right">
          <template #default="{ row }">
            <el-space>
              <el-button size="small" @click="retryHistory(row)">复测</el-button>
              <el-button size="small" type="danger" text @click="removeHistory(row.id)">删除</el-button>
            </el-space>
          </template>
        </el-table-column>
      </el-table>
      <div class="history-actions">
        <el-button :disabled="history.length === 0" @click="clearHistory">清空历史</el-button>
      </div>
      <div v-if="history.length === 0" class="empty-hint history-empty">
        暂无测试历史
      </div>
      <div v-else-if="filteredHistory.length === 0" class="empty-hint history-empty">
        无匹配的筛选结果
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { getSettingJson, setSettingJson } from "../composables/useSettings";

type Protocol = "tcp" | "http" | "https";

interface TestResult {
  reachable: boolean;
  latencyMs: number;
  host?: string;
  port?: number;
  url?: string;
  statusCode?: number | null;
  error?: string | null;
}

interface NetworkHistoryItem {
  id: string;
  checkedAt: number;
  protocol: Protocol;
  target: string;
  timeoutMs: number;
  reachable: boolean;
  latencyMs: number;
  statusCode: number | null;
  error: string | null;
}

const NETWORK_HISTORY_KEY = "network_test_history";
const MAX_HISTORY = 50;

const protocol = ref<Protocol>("tcp");
const host = ref("127.0.0.1");
const port = ref(80);
const timeoutMs = ref(2000);
const httpUrl = ref("");
const loading = ref(false);
const retryingFailed = ref(false);
const result = ref<TestResult | null>(null);
const lastCheckedAt = ref<number>(0);
const history = ref<NetworkHistoryItem[]>(loadHistory());
const historyProtocolFilter = ref<"all" | Protocol>("all");
const historyResultFilter = ref<"all" | "success" | "failed">("all");
const historyKeyword = ref("");

const resultTargetText = computed(() => {
  if (!result.value) return "-";
  if (protocol.value === "tcp") {
    return `${result.value.host ?? host.value}:${result.value.port ?? port.value}`;
  }
  return result.value.url ?? normalizeHttpUrl(httpUrl.value, protocol.value);
});

const filteredHistory = computed(() => {
  const keyword = historyKeyword.value.trim().toLowerCase();
  return history.value.filter((item) => {
    if (historyProtocolFilter.value !== "all" && item.protocol !== historyProtocolFilter.value) {
      return false;
    }
    if (historyResultFilter.value === "success" && !item.reachable) {
      return false;
    }
    if (historyResultFilter.value === "failed" && item.reachable) {
      return false;
    }
    if (keyword) {
      const targetMatch = item.target.toLowerCase().includes(keyword);
      const errorMatch = (item.error ?? "").toLowerCase().includes(keyword);
      if (!targetMatch && !errorMatch) return false;
    }
    return true;
  });
});

const filteredFailedHistory = computed(() => filteredHistory.value.filter((item) => !item.reachable));

const historyStats = computed(() => {
  const total = history.value.length;
  const success = history.value.filter((item) => item.reachable).length;
  const failed = total - success;
  const successRate = total > 0 ? `${Math.round((success / total) * 100)}%` : "0%";
  return { total, success, failed, successRate };
});

watch(protocol, () => {
  result.value = null;
  if (protocol.value === "http") {
    if (!httpUrl.value.trim()) {
      httpUrl.value = "http://";
    } else if (httpUrl.value.startsWith("https://")) {
      httpUrl.value = `http://${httpUrl.value.slice("https://".length)}`;
    }
    if (port.value === 443) port.value = 80;
  } else if (protocol.value === "https") {
    if (!httpUrl.value.trim()) {
      httpUrl.value = "https://";
    } else if (httpUrl.value.startsWith("http://")) {
      httpUrl.value = `https://${httpUrl.value.slice("http://".length)}`;
    }
    if (port.value === 80) port.value = 443;
  }
});

function loadHistory(): NetworkHistoryItem[] {
  const raw = getSettingJson<unknown[]>(NETWORK_HISTORY_KEY, []);
  if (!Array.isArray(raw)) return [];
  const rows = raw.filter((item): item is NetworkHistoryItem => {
    const v = item as Record<string, unknown>;
    return (
      typeof v?.id === "string" &&
      typeof v?.checkedAt === "number" &&
      (v?.protocol === "tcp" || v?.protocol === "http" || v?.protocol === "https") &&
      typeof v?.target === "string" &&
      typeof v?.timeoutMs === "number" &&
      typeof v?.reachable === "boolean" &&
      typeof v?.latencyMs === "number"
    );
  });
  rows.sort((a, b) => b.checkedAt - a.checkedAt);
  return rows.slice(0, MAX_HISTORY);
}

function persistHistory() {
  setSettingJson(NETWORK_HISTORY_KEY, history.value);
}

function appendHistory(item: Omit<NetworkHistoryItem, "id" | "checkedAt">) {
  const entry: NetworkHistoryItem = {
    id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
    checkedAt: Date.now(),
    ...item
  };
  history.value = [entry, ...history.value].slice(0, MAX_HISTORY);
  persistHistory();
}

function removeHistory(id: string) {
  history.value = history.value.filter((item) => item.id !== id);
  persistHistory();
}

function clearHistory() {
  history.value = [];
  persistHistory();
}

function reuseHistory(item: NetworkHistoryItem) {
  protocol.value = item.protocol;
  timeoutMs.value = item.timeoutMs;
  if (item.protocol === "tcp") {
    const [h, p] = item.target.split(":");
    host.value = h || host.value;
    const parsedPort = Number(p);
    if (Number.isFinite(parsedPort) && parsedPort >= 1 && parsedPort <= 65535) {
      port.value = parsedPort;
    }
  } else {
    httpUrl.value = item.target;
  }
}

async function retryHistory(item: NetworkHistoryItem) {
  reuseHistory(item);
  await runTest();
}

async function retryFailedHistory() {
  if (retryingFailed.value || loading.value) return;
  const targets = filteredFailedHistory.value.slice(0, 10);
  if (targets.length === 0) {
    ElMessage.info("当前筛选下没有失败记录");
    return;
  }
  retryingFailed.value = true;
  let ok = 0;
  for (const item of targets) {
    await retryHistory(item);
    if (result.value?.reachable) ok += 1;
  }
  retryingFailed.value = false;
  ElMessage.success(`批量复测完成，共 ${targets.length} 项，恢复可达 ${ok} 项`);
}

function applyQuickTarget(nextHost: string, nextPort: number) {
  protocol.value = "tcp";
  host.value = nextHost;
  port.value = nextPort;
}

function applyQuickHttpPath(path: string) {
  if (protocol.value === "tcp") {
    protocol.value = "http";
  }
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  const normalized = normalizeHttpUrl(httpUrl.value, protocol.value);
  if (!normalized || normalized === `${protocol.value}://`) {
    httpUrl.value = `${protocol.value}://127.0.0.1${normalizedPath}`;
    return;
  }
  try {
    const parsed = new URL(normalized);
    parsed.pathname = normalizedPath;
    parsed.search = "";
    parsed.hash = "";
    httpUrl.value = parsed.toString();
  } catch {
    // Ignore malformed url here, runTest will show validation.
  }
}

function normalizeHttpUrl(raw: string, p: Protocol): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    return trimmed;
  }
  return `${p}://${trimmed}`;
}

async function runTest() {
  if (loading.value) return;

  const currentProtocol = protocol.value;
  if (currentProtocol === "tcp") {
    if (!host.value.trim()) {
      ElMessage.warning("请输入主机地址");
      return;
    }
  } else {
    const normalizedUrl = normalizeHttpUrl(httpUrl.value, currentProtocol);
    if (!normalizedUrl) {
      ElMessage.warning("请输入 URL");
      return;
    }
    try {
      // Validate URL format before invoking Rust side.
      new URL(normalizedUrl);
      httpUrl.value = normalizedUrl;
    } catch {
      ElMessage.warning("URL 格式不正确");
      return;
    }
  }

  result.value = null;
  loading.value = true;
  try {
    let nextResult: TestResult;
    if (currentProtocol === "tcp") {
      const data = await invokeToolByChannel("tool:network:tcp-test", {
        host: host.value.trim(),
        port: port.value,
        timeoutMs: timeoutMs.value
      });
      nextResult = data as TestResult;
    } else {
      const data = await invokeToolByChannel("tool:network:http-test", {
        url: httpUrl.value.trim(),
        timeoutMs: timeoutMs.value
      });
      nextResult = data as TestResult;
    }

    result.value = nextResult;
    lastCheckedAt.value = Date.now();
    appendHistory({
      protocol: currentProtocol,
      target:
        currentProtocol === "tcp"
          ? `${nextResult.host ?? host.value.trim()}:${nextResult.port ?? port.value}`
          : nextResult.url ?? httpUrl.value.trim(),
      timeoutMs: timeoutMs.value,
      reachable: nextResult.reachable,
      latencyMs: Number(nextResult.latencyMs ?? 0),
      statusCode: nextResult.statusCode ?? null,
      error: nextResult.error ?? null
    });
  } catch (e) {
    const message = (e as Error).message;
    const failedResult: TestResult = {
      reachable: false,
      latencyMs: 0,
      error: message
    };
    result.value = failedResult;
    lastCheckedAt.value = Date.now();
    appendHistory({
      protocol: currentProtocol,
      target: currentProtocol === "tcp" ? `${host.value.trim()}:${port.value}` : httpUrl.value.trim(),
      timeoutMs: timeoutMs.value,
      reachable: false,
      latencyMs: 0,
      statusCode: null,
      error: message
    });
  } finally {
    loading.value = false;
  }
}

function formatTime(timestamp: number): string {
  if (!timestamp) return "-";
  const date = new Date(timestamp);
  const yyyy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const dd = String(date.getDate()).padStart(2, "0");
  const hh = String(date.getHours()).padStart(2, "0");
  const mi = String(date.getMinutes()).padStart(2, "0");
  const ss = String(date.getSeconds()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}:${ss}`;
}

function statusTagType(code: number): "success" | "warning" | "danger" | "info" {
  if (code >= 200 && code < 300) return "success";
  if (code >= 300 && code < 400) return "info";
  if (code >= 400 && code < 500) return "warning";
  return "danger";
}
</script>

<style scoped>
.network-panel {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.network-controls {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.protocol-group {
  align-self: flex-start;
}

.input-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.quick-actions {
  margin-top: 2px;
}

.quick-actions-label {
  color: var(--el-text-color-secondary);
  font-size: 13px;
}

.result-loading {
  margin-top: 2px;
}

.result-card {
  border-radius: 8px;
  padding: 14px;
  border: 1px solid;
}

.result-ok {
  border-color: var(--el-color-success-light-5);
  background-color: var(--el-color-success-light-9);
}

.result-fail {
  border-color: var(--el-color-danger-light-5);
  background-color: var(--el-color-danger-light-9);
}

.result-header {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  font-weight: 600;
  flex-wrap: wrap;
}

.result-icon {
  font-size: 18px;
}

.result-time {
  margin-left: auto;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  font-weight: 400;
}

.result-ok .result-icon,
.result-ok .result-status {
  color: var(--el-color-success);
}

.result-fail .result-icon,
.result-fail .result-status {
  color: var(--el-color-danger);
}

.error-text {
  color: var(--el-color-danger);
  word-break: break-word;
}

.history-section {
  margin-top: 2px;
}

.history-stats {
  margin-bottom: 10px;
}

.history-filters {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
  flex-wrap: wrap;
}

.history-actions {
  margin-top: 8px;
  display: flex;
  justify-content: flex-end;
}

.empty-hint {
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  text-align: center;
  padding: 28px 0;
}

.history-empty {
  padding: 10px 0 2px;
}
</style>
