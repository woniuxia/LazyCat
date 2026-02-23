<template>
  <div class="network-panel">
    <!-- 主控制台区域 -->
    <div class="network-console">
      <div class="console-header">
        <span class="console-icon">🔌</span>
        <span class="console-title">连通性测试</span>
      </div>

      <!-- 协议选择 - 分段控制器 -->
      <div class="protocol-segmented">
        <button
          v-for="p in protocols"
          :key="p.value"
          class="protocol-btn"
          :class="{ active: protocol === p.value }"
          @click="protocol = p.value"
        >
          <span class="protocol-indicator" :class="p.value" />
          {{ p.label }}
        </button>
      </div>

      <!-- 输入区域 -->
      <div class="input-section">
        <template v-if="protocol === 'tcp'">
          <div class="tcp-inputs">
            <div class="input-group host-group">
              <label>主机地址</label>
              <input
                v-model="host"
                type="text"
                placeholder="127.0.0.1 或 example.com"
                @keyup.enter="runTest"
              />
            </div>
            <div class="input-group port-group">
              <label>端口</label>
              <input
                v-model.number="port"
                type="number"
                min="1"
                max="65535"
                placeholder="80"
                @keyup.enter="runTest"
              />
            </div>
          </div>
        </template>

        <template v-else>
          <div class="input-group">
            <label>URL 地址</label>
            <input
              v-model="httpUrl"
              type="text"
              :placeholder="protocol === 'https' ? 'https://api.example.com' : 'http://localhost:8080'"
              @keyup.enter="runTest"
            />
          </div>
        </template>

        <!-- 超时滑块 -->
        <div class="timeout-control">
          <label>超时时间</label>
          <div class="slider-wrapper">
            <input
              v-model.number="timeoutMs"
              type="range"
              min="100"
              max="10000"
              step="100"
              class="timeout-slider"
            />
            <span class="timeout-value">{{ timeoutMs }}ms</span>
          </div>
        </div>
      </div>

      <!-- 快捷端口网格 -->
      <div class="quick-ports-grid">
        <button
          v-for="item in quickPorts"
          :key="item.port"
          class="quick-port-card"
          @click="applyQuickTarget(item.host, item.port)"
        >
          <span class="port-icon">{{ item.icon }}</span>
          <span class="port-number">:{{ item.port }}</span>
          <span class="port-name">{{ item.name }}</span>
        </button>
      </div>

      <!-- HTTP 快捷路径 -->
      <div v-if="protocol !== 'tcp'" class="quick-paths">
        <button
          v-for="path in quickPaths"
          :key="path"
          class="quick-path-btn"
          @click="applyQuickHttpPath(path)"
        >
          {{ path }}
        </button>
      </div>

      <!-- 测试按钮 -->
      <button
        class="test-button"
        :class="{ testing: loading }"
        :disabled="loading"
        @click="runTest"
      >
        <span v-if="loading" class="btn-spinner" />
        <span v-else class="btn-icon">▶</span>
        {{ loading ? "测试中..." : "开始测试" }}
      </button>
    </div>

    <!-- 结果展示区域 -->
    <Transition name="result-slide">
      <div
        v-if="result && !loading"
        class="result-panel"
        :class="result.reachable ? 'result-success' : 'result-failed'"
      >
        <div class="result-glow" />
        <div class="result-content">
          <div class="result-main">
            <div class="result-status-badge">
              <span class="status-dot" />
              <span class="status-text">{{ result.reachable ? "可达" : "不可达" }}</span>
            </div>
            <div class="result-latency">
              <span class="latency-value">{{ result.latencyMs }}</span>
              <span class="latency-unit">ms</span>
            </div>
          </div>

          <div class="result-details">
            <div class="detail-item">
              <span class="detail-label">协议</span>
              <span class="detail-value protocol-badge">{{ protocol.toUpperCase() }}</span>
            </div>
            <div class="detail-item">
              <span class="detail-label">目标</span>
              <span class="detail-value target-text">{{ resultTargetText }}</span>
            </div>
            <div v-if="result.statusCode" class="detail-item">
              <span class="detail-label">状态码</span>
              <span class="detail-value" :class="'status-' + statusCategory(result.statusCode)">
                {{ result.statusCode }}
              </span>
            </div>
            <div v-if="result.error" class="detail-item error-item">
              <span class="detail-label">错误</span>
              <span class="detail-value error-text">{{ result.error }}</span>
            </div>
          </div>

          <div class="result-time">
            测试于 {{ formatTime(lastCheckedAt) }}
          </div>
        </div>
      </div>
    </Transition>

    <div v-if="!loading && !result" class="empty-state">
      <div class="empty-icon">📡</div>
      <p>输入目标地址，开始连通性测试</p>
    </div>

    <!-- 历史记录区域 -->
    <div class="history-panel">
      <div class="history-header">
        <div class="history-title">
          <span class="history-icon">📊</span>
          <span>最近测试记录</span>
          <span class="history-count">({{ history.length }})</span>
        </div>
        <button v-if="history.length > 0" class="clear-btn" @click="clearHistory">清空</button>
      </div>

      <!-- 统计信息 -->
      <div class="stats-bar">
        <div class="stat-item">
          <span class="stat-value">{{ historyStats.total }}</span>
          <span class="stat-label">总次数</span>
        </div>
        <div class="stat-item success">
          <span class="stat-value">{{ historyStats.success }}</span>
          <span class="stat-label">成功</span>
        </div>
        <div class="stat-item failed">
          <span class="stat-value">{{ historyStats.failed }}</span>
          <span class="stat-label">失败</span>
        </div>
        <div class="stat-item rate">
          <span class="stat-value">{{ historyStats.successRate }}</span>
          <span class="stat-label">成功率</span>
        </div>
      </div>

      <!-- 筛选器 -->
      <div class="history-filters">
        <select v-model="historyProtocolFilter" class="filter-select">
          <option value="all">全部协议</option>
          <option value="tcp">TCP</option>
          <option value="http">HTTP</option>
          <option value="https">HTTPS</option>
        </select>
        <select v-model="historyResultFilter" class="filter-select">
          <option value="all">全部结果</option>
          <option value="success">仅成功</option>
          <option value="failed">仅失败</option>
        </select>
        <input
          v-model="historyKeyword"
          type="text"
          class="filter-input"
          placeholder="搜索目标或错误..."
        />
        <button
          v-if="filteredFailedHistory.length > 0"
          class="retry-failed-btn"
          :disabled="retryingFailed"
          @click="retryFailedHistory"
        >
          <span v-if="retryingFailed" class="btn-spinner-small" />
          重测失败项
        </button>
      </div>

      <!-- 历史列表 - 卡片式 -->
      <div class="history-list">
        <TransitionGroup name="history-item">
          <div
            v-for="item in filteredHistory"
            :key="item.id"
            class="history-card"
            :class="item.reachable ? 'card-success' : 'card-failed'"
            @click="reuseHistory(item)"
          >
            <div class="card-status">
              <span class="card-dot" />
            </div>
            <div class="card-info">
              <div class="card-target">{{ item.target }}</div>
              <div class="card-meta">
                <span class="meta-protocol">{{ item.protocol.toUpperCase() }}</span>
                <span class="meta-time">{{ formatTime(item.checkedAt) }}</span>
              </div>
            </div>
            <div class="card-metrics">
              <span class="metric-latency" :class="latencyClass(item.latencyMs)">
                {{ item.latencyMs }}ms
              </span>
              <span v-if="item.statusCode" class="metric-status" :class="'status-' + statusCategory(item.statusCode)">
                {{ item.statusCode }}
              </span>
            </div>
            <div class="card-actions" @click.stop>
              <button class="action-btn retry" @click="retryHistory(item)">复测</button>
              <button class="action-btn delete" @click="removeHistory(item.id)">删除</button>
            </div>
          </div>
        </TransitionGroup>

        <!-- 空状态 -->
        <div v-if="history.length === 0" class="history-empty">
          <span class="empty-icon">📝</span>
          <p>暂无测试记录</p>
        </div>
        <div v-else-if="filteredHistory.length === 0" class="history-empty">
          <p>无匹配的筛选结果</p>
        </div>
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

const protocols = [
  { value: "tcp" as Protocol, label: "TCP" },
  { value: "http" as Protocol, label: "HTTP" },
  { value: "https" as Protocol, label: "HTTPS" }
];

const quickPorts = [
  { host: "127.0.0.1", port: 80, name: "HTTP", icon: "🌐" },
  { host: "127.0.0.1", port: 443, name: "HTTPS", icon: "🔒" },
  { host: "127.0.0.1", port: 3306, name: "MySQL", icon: "🐬" },
  { host: "127.0.0.1", port: 6379, name: "Redis", icon: "⚡" },
  { host: "127.0.0.1", port: 5432, name: "PostgreSQL", icon: "🐘" },
  { host: "127.0.0.1", port: 8080, name: "Dev", icon: "🛠️" },
  { host: "127.0.0.1", port: 3000, name: "Node", icon: "📦" },
  { host: "127.0.0.1", port: 5173, name: "Vite", icon: "⚡" }
];

const quickPaths = ["/health", "/actuator/health", "/api/health", "/status"];

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

function latencyClass(ms: number): string {
  if (ms < 50) return "excellent";
  if (ms < 200) return "good";
  if (ms < 500) return "fair";
  return "poor";
}

function statusCategory(code: number): string {
  if (code >= 200 && code < 300) return "2xx";
  if (code >= 300 && code < 400) return "3xx";
  if (code >= 400 && code < 500) return "4xx";
  return "5xx";
}

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
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();

  const hh = String(date.getHours()).padStart(2, "0");
  const mi = String(date.getMinutes()).padStart(2, "0");
  const ss = String(date.getSeconds()).padStart(2, "0");

  if (isToday) {
    return `${hh}:${mi}:${ss}`;
  }

  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const dd = String(date.getDate()).padStart(2, "0");
  return `${mm}-${dd} ${hh}:${mi}`;
}
</script>

<style scoped>
.network-panel {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 20px;
  max-width: 900px;
  margin: 0 auto;
}

/* 控制台区域 */
.network-console {
  background: linear-gradient(145deg, var(--el-bg-color) 0%, var(--el-fill-color-light) 100%);
  border: 1px solid var(--el-border-color);
  border-radius: 16px;
  padding: 24px;
  box-shadow:
    0 4px 6px -1px rgba(0, 0, 0, 0.05),
    0 2px 4px -1px rgba(0, 0, 0, 0.03),
    inset 0 1px 0 rgba(255, 255, 255, 0.1);
}

.console-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 20px;
  padding-bottom: 16px;
  border-bottom: 2px solid var(--el-border-color-lighter);
}

.console-icon {
  font-size: 24px;
}

.console-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  letter-spacing: -0.3px;
}

/* 协议分段控制器 */
.protocol-segmented {
  display: flex;
  gap: 4px;
  background: var(--el-fill-color);
  padding: 4px;
  border-radius: 10px;
  margin-bottom: 20px;
  width: fit-content;
}

.protocol-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border: none;
  background: transparent;
  color: var(--el-text-color-secondary);
  font-size: 14px;
  font-weight: 500;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s var(--lc-ease);
}

.protocol-btn:hover {
  color: var(--el-text-color-primary);
  background: var(--el-fill-color-light);
}

.protocol-btn.active {
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.08);
}

.protocol-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--el-text-color-placeholder);
  transition: all 0.2s ease;
}

.protocol-indicator.tcp {
  background: #409eff;
}

.protocol-indicator.http {
  background: #67c23a;
}

.protocol-indicator.https {
  background: #e6a23c;
}

.protocol-btn.active .protocol-indicator {
  box-shadow: 0 0 8px currentColor;
}

/* 输入区域 */
.input-section {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-bottom: 20px;
}

.tcp-inputs {
  display: grid;
  grid-template-columns: 1fr 120px;
  gap: 12px;
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.input-group label {
  font-size: 12px;
  font-weight: 500;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.input-group input {
  padding: 12px 16px;
  border: 2px solid var(--el-border-color);
  border-radius: 10px;
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
  font-size: 15px;
  transition: all 0.2s var(--lc-ease);
  outline: none;
}

.input-group input:focus {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 3px var(--el-color-primary-light-8);
}

.input-group input::placeholder {
  color: var(--el-text-color-placeholder);
}

/* 超时控制 */
.timeout-control {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.timeout-control label {
  font-size: 12px;
  font-weight: 500;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.slider-wrapper {
  display: flex;
  align-items: center;
  gap: 16px;
}

.timeout-slider {
  flex: 1;
  height: 6px;
  -webkit-appearance: none;
  appearance: none;
  background: var(--el-fill-color-dark);
  border-radius: 3px;
  outline: none;
  cursor: pointer;
}

.timeout-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 18px;
  height: 18px;
  background: var(--el-color-primary);
  border-radius: 50%;
  cursor: pointer;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  transition: transform 0.15s ease;
}

.timeout-slider::-webkit-slider-thumb:hover {
  transform: scale(1.1);
}

.timeout-value {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-color-primary);
  min-width: 60px;
  text-align: right;
  font-family: var(--lc-font-mono);
}

/* 快捷端口网格 */
.quick-ports-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 10px;
  margin-bottom: 16px;
}

@media (max-width: 640px) {
  .quick-ports-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

.quick-port-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 12px 8px;
  border: 1px solid var(--el-border-color);
  border-radius: 10px;
  background: var(--el-bg-color);
  cursor: pointer;
  transition: all 0.2s var(--lc-ease);
}

.quick-port-card:hover {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
}

.port-icon {
  font-size: 20px;
  line-height: 1;
}

.port-number {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  font-family: var(--lc-font-mono);
}

.port-name {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}

/* HTTP 快捷路径 */
.quick-paths {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 20px;
}

.quick-path-btn {
  padding: 6px 12px;
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  background: var(--el-fill-color-light);
  color: var(--el-text-color-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.quick-path-btn:hover {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}

/* 测试按钮 */
.test-button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  width: 100%;
  padding: 16px 24px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(135deg, var(--el-color-primary) 0%, var(--el-color-primary-light-3) 100%);
  color: white;
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s var(--lc-ease);
  box-shadow: 0 4px 14px rgba(64, 158, 255, 0.35);
}

.test-button:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 6px 20px rgba(64, 158, 255, 0.45);
}

.test-button:active:not(:disabled) {
  transform: translateY(0);
}

.test-button:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.test-button.testing {
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0%, 100% {
    box-shadow: 0 4px 14px rgba(64, 158, 255, 0.35);
  }
  50% {
    box-shadow: 0 4px 20px rgba(64, 158, 255, 0.6);
  }
}

.btn-icon {
  font-size: 12px;
}

.btn-spinner {
  width: 18px;
  height: 18px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* 结果面板 */
.result-panel {
  position: relative;
  border-radius: 16px;
  padding: 24px;
  overflow: hidden;
  transition: all 0.3s var(--lc-ease);
}

.result-success {
  background: linear-gradient(145deg, #f0f9eb 0%, #e6f7d9 100%);
  border: 1px solid #b3e19d;
}

.result-failed {
  background: linear-gradient(145deg, #fef0f0 0%, #fde2e2 100%);
  border: 1px solid #f9a8a8;
}

.result-glow {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 200px;
  height: 200px;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  filter: blur(60px);
  opacity: 0.5;
  pointer-events: none;
}

.result-success .result-glow {
  background: #67c23a;
  animation: glow-success 3s ease-in-out infinite;
}

.result-failed .result-glow {
  background: #f56c6c;
  animation: glow-failed 3s ease-in-out infinite;
}

@keyframes glow-success {
  0%, 100% {
    opacity: 0.3;
    transform: translate(-50%, -50%) scale(1);
  }
  50% {
    opacity: 0.5;
    transform: translate(-50%, -50%) scale(1.1);
  }
}

@keyframes glow-failed {
  0%, 100% {
    opacity: 0.3;
    transform: translate(-50%, -50%) scale(1);
  }
  50% {
    opacity: 0.5;
    transform: translate(-50%, -50%) scale(1.1);
  }
}

.result-content {
  position: relative;
  z-index: 1;
}

.result-main {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}

.result-status-badge {
  display: flex;
  align-items: center;
  gap: 10px;
}

.status-dot {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: currentColor;
  animation: dot-pulse 2s ease-in-out infinite;
}

.result-success .status-dot {
  background: #67c23a;
  box-shadow: 0 0 0 rgba(103, 194, 58, 0.4);
}

.result-failed .status-dot {
  background: #f56c6c;
  box-shadow: 0 0 0 rgba(245, 108, 108, 0.4);
}

@keyframes dot-pulse {
  0%, 100% {
    box-shadow: 0 0 0 0 currentColor;
  }
  50% {
    box-shadow: 0 0 0 8px transparent;
  }
}

.status-text {
  font-size: 24px;
  font-weight: 700;
}

.result-success .status-text {
  color: #529b2e;
}

.result-failed .status-text {
  color: #c45656;
}

.result-latency {
  display: flex;
  align-items: baseline;
  gap: 4px;
}

.latency-value {
  font-size: 42px;
  font-weight: 800;
  font-family: var(--lc-font-mono);
  line-height: 1;
}

.result-success .latency-value {
  color: #529b2e;
}

.result-failed .latency-value {
  color: #c45656;
}

.latency-unit {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-secondary);
}

.result-details {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
  padding: 16px;
  background: rgba(255, 255, 255, 0.5);
  border-radius: 10px;
}

.detail-item {
  display: flex;
  align-items: center;
  gap: 10px;
}

.detail-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  min-width: 50px;
}

.detail-value {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.protocol-badge {
  padding: 2px 10px;
  border-radius: 4px;
  background: var(--el-fill-color);
  font-size: 12px;
  font-family: var(--lc-font-mono);
}

.target-text {
  font-family: var(--lc-font-mono);
  word-break: break-all;
}

.status-2xx {
  color: #67c23a;
  font-weight: 600;
}

.status-3xx {
  color: #409eff;
  font-weight: 600;
}

.status-4xx {
  color: #e6a23c;
  font-weight: 600;
}

.status-5xx {
  color: #f56c6c;
  font-weight: 600;
}

.error-text {
  color: #c45656;
  word-break: break-word;
}

.result-time {
  margin-top: 16px;
  text-align: right;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

/* 结果滑入动画 */
.result-slide-enter-active,
.result-slide-leave-active {
  transition: all 0.4s var(--lc-ease-out);
}

.result-slide-enter-from,
.result-slide-leave-to {
  opacity: 0;
  transform: translateY(-20px);
}

/* 空状态 */
.empty-state {
  text-align: center;
  padding: 48px 20px;
  color: var(--el-text-color-placeholder);
}

.empty-icon {
  font-size: 48px;
  margin-bottom: 16px;
  opacity: 0.6;
}

.empty-state p {
  font-size: 14px;
}

/* 历史记录面板 */
.history-panel {
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color);
  border-radius: 16px;
  padding: 20px;
}

.history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.history-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.history-icon {
  font-size: 18px;
}

.history-count {
  color: var(--el-text-color-secondary);
  font-weight: 400;
}

.clear-btn {
  padding: 6px 12px;
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  background: transparent;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.clear-btn:hover {
  border-color: var(--el-color-danger);
  color: var(--el-color-danger);
  background: var(--el-color-danger-light-9);
}

/* 统计条 */
.stats-bar {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-bottom: 16px;
}

@media (max-width: 560px) {
  .stats-bar {
    grid-template-columns: repeat(2, 1fr);
  }
}

.stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 12px;
  background: var(--el-fill-color-light);
  border-radius: 10px;
  text-align: center;
}

.stat-value {
  font-size: 22px;
  font-weight: 700;
  color: var(--el-text-color-primary);
  font-family: var(--lc-font-mono);
}

.stat-label {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.stat-item.success .stat-value {
  color: #67c23a;
}

.stat-item.failed .stat-value {
  color: #f56c6c;
}

.stat-item.rate .stat-value {
  color: var(--el-color-primary);
}

/* 筛选器 */
.history-filters {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-bottom: 16px;
}

.filter-select,
.filter-input {
  padding: 8px 12px;
  border: 1px solid var(--el-border-color);
  border-radius: 8px;
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
  font-size: 13px;
  outline: none;
  transition: all 0.2s ease;
}

.filter-select {
  min-width: 110px;
  cursor: pointer;
}

.filter-input {
  flex: 1;
  min-width: 150px;
}

.filter-select:focus,
.filter-input:focus {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 2px var(--el-color-primary-light-8);
}

.retry-failed-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border: 1px solid var(--el-color-warning);
  border-radius: 8px;
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.retry-failed-btn:hover:not(:disabled) {
  background: var(--el-color-warning);
  color: white;
}

.retry-failed-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-spinner-small {
  width: 14px;
  height: 14px;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

/* 历史卡片列表 */
.history-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: 400px;
  overflow-y: auto;
  padding-right: 4px;
}

.history-list::-webkit-scrollbar {
  width: 6px;
}

.history-list::-webkit-scrollbar-track {
  background: transparent;
}

.history-list::-webkit-scrollbar-thumb {
  background: var(--el-border-color);
  border-radius: 3px;
}

.history-list::-webkit-scrollbar-thumb:hover {
  background: var(--el-text-color-disabled);
}

.history-card {
  display: grid;
  grid-template-columns: auto 1fr auto auto;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
  border-radius: 12px;
  border: 1px solid var(--el-border-color-lighter);
  background: var(--el-fill-color-light);
  cursor: pointer;
  transition: all 0.2s var(--lc-ease);
}

.history-card:hover {
  border-color: var(--el-border-color);
  background: var(--el-bg-color);
  transform: translateX(4px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.06);
}

.card-success {
  border-left: 3px solid #67c23a;
}

.card-failed {
  border-left: 3px solid #f56c6c;
}

.card-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.card-success .card-dot {
  background: #67c23a;
}

.card-failed .card-dot {
  background: #f56c6c;
}

.card-info {
  min-width: 0;
}

.card-target {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
  font-family: var(--lc-font-mono);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-bottom: 4px;
}

.card-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.meta-protocol {
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--el-fill-color-dark);
  color: var(--el-text-color-secondary);
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
}

.meta-time {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.card-metrics {
  display: flex;
  align-items: center;
  gap: 10px;
}

.metric-latency {
  font-size: 14px;
  font-weight: 600;
  font-family: var(--lc-font-mono);
  padding: 4px 10px;
  border-radius: 6px;
  background: var(--el-fill-color);
}

.metric-latency.excellent {
  color: #67c23a;
  background: #f0f9eb;
}

.metric-latency.good {
  color: #409eff;
  background: #ecf5ff;
}

.metric-latency.fair {
  color: #e6a23c;
  background: #fdf6ec;
}

.metric-latency.poor {
  color: #f56c6c;
  background: #fef0f0;
}

.metric-status {
  font-size: 12px;
  font-weight: 600;
  padding: 4px 10px;
  border-radius: 6px;
}

.card-actions {
  display: flex;
  gap: 6px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.history-card:hover .card-actions {
  opacity: 1;
}

.action-btn {
  padding: 6px 12px;
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  background: var(--el-bg-color);
  color: var(--el-text-color-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.action-btn:hover {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
}

.action-btn.delete:hover {
  border-color: var(--el-color-danger);
  color: var(--el-color-danger);
}

/* 历史项动画 */
.history-item-enter-active,
.history-item-leave-active {
  transition: all 0.3s var(--lc-ease);
}

.history-item-enter-from,
.history-item-leave-to {
  opacity: 0;
  transform: translateX(-20px);
}

/* 历史空状态 */
.history-empty {
  text-align: center;
  padding: 40px 20px;
  color: var(--el-text-color-placeholder);
}

.history-empty .empty-icon {
  font-size: 40px;
  margin-bottom: 12px;
  opacity: 0.5;
}

.history-empty p {
  font-size: 14px;
}
</style>
