<template>
  <div class="dns-panel">
    <div class="panel-header">
      <div class="header-title">
        <span class="title-icon">◈</span>
        <span class="title-text">DNS Resolver</span>
      </div>
      <div class="header-line"></div>
    </div>

    <el-tabs v-model="activeTab" class="dns-tabs">
      <!-- Query Tab -->
      <el-tab-pane name="query">
        <template #label>
          <span class="tab-label">
            <span class="tab-dot"></span>
            QUERY
          </span>
        </template>

        <div class="query-section">
          <div class="input-card">
            <div class="input-row primary">
              <div class="input-label">TARGET</div>
              <el-input
                v-model="domain"
                class="terminal-input"
                placeholder="domain.tld or 1.2.3.4"
                clearable
                @keyup.enter="runQuery"
              >
                <template #prefix>
                  <span class="input-prompt">»</span>
                </template>
              </el-input>
            </div>

            <div class="input-row secondary">
              <div class="input-label">SERVER</div>
              <el-input
                v-model="dnsServer"
                class="terminal-input small"
                placeholder="auto-detect"
                clearable
                @input="onServerInput"
              >
                <template #prefix>
                  <span class="input-prompt secondary">$</span>
                </template>
              </el-input>

              <button
                class="query-btn"
                :class="{ loading: loading }"
                @click="runQuery"
              >
                <span v-if="!loading">EXECUTE</span>
                <span v-else class="blink">···</span>
              </button>
            </div>
          </div>

          <div class="preset-grid">
            <div
              v-for="preset in PRESET_DNS_SERVERS"
              :key="preset.label"
              class="preset-chip"
              :class="{ active: selectedPreset === preset.label }"
              @click="selectPreset(preset)"
            >
              <span class="chip-indicator"></span>
              <span class="chip-label">{{ preset.label }}</span>
              <span v-if="preset.ip" class="chip-ip">{{ preset.ip }}</span>
            </div>
          </div>

          <div v-if="systemDnsHint" class="system-hint">
            <span class="hint-icon">◉</span>
            <span class="hint-text">SYSTEM_DNS: {{ systemDnsHint }}</span>
          </div>
        </div>

        <!-- Loading State -->
        <div v-if="loading" class="loading-container">
          <div class="loading-radar">
            <div class="radar-ring"></div>
            <div class="radar-ring"></div>
            <div class="radar-ring"></div>
          </div>
          <div class="loading-text">RESOLVING RECORDS...</div>
        </div>

        <!-- Results -->
        <template v-else-if="result">
          <div class="results-container">
            <div class="result-header">
              <div class="result-badge">
                <span class="badge-label">SERVER</span>
                <span class="badge-value">{{ result.server }}</span>
              </div>
              <div class="result-badge latency">
                <span class="badge-label">LATENCY</span>
                <span class="badge-value" :class="getLatencyClass(result.elapsed_ms)">
                  {{ result.elapsed_ms }}ms
                </span>
              </div>
              <button
                v-if="getRecords('A').length > 1"
                class="copy-all-btn"
                @click="copyAllIpv4"
              >
                <span class="copy-icon">⎘</span>
                COPY ALL
              </button>
            </div>

            <div class="records-grid">
              <div
                v-for="rt in visibleRecordTypes"
                :key="rt.type"
                class="record-card"
              >
                <div class="record-header">
                  <span class="record-type">{{ rt.type }}</span>
                  <span class="record-count">{{ getRecords(rt.type).length }}</span>
                </div>
                <div class="record-table">
                  <div
                    v-for="(record, idx) in getRecords(rt.type)"
                    :key="idx"
                    class="record-row"
                  >
                    <div class="row-main">
                      <template v-if="rt.type === 'A' || rt.type === 'AAAA'">
                        <span class="data-value mono">{{ (record as any).address }}</span>
                        <button
                          class="row-copy-btn"
                          @click="copyAddress(String((record as any).address))"
                        >COPY</button>
                      </template>
                      <template v-else-if="rt.type === 'CNAME'">
                        <span class="data-value">{{ (record as any).target }}</span>
                      </template>
                      <template v-else-if="rt.type === 'MX'">
                        <span class="data-priority">{{ (record as any).preference }}</span>
                        <span class="data-value">{{ (record as any).exchange }}</span>
                      </template>
                      <template v-else-if="rt.type === 'NS'">
                        <span class="data-value">{{ (record as any).host }}</span>
                      </template>
                      <template v-else-if="rt.type === 'TXT'">
                        <span class="data-value txt">{{ (record as any).text }}</span>
                      </template>
                      <template v-else-if="rt.type === 'PTR'">
                        <span class="data-value">{{ (record as any).hostname }}</span>
                      </template>
                      <template v-else-if="rt.type === 'SOA'">
                        <div class="soa-grid">
                          <span>MNAME: {{ (record as any).mname }}</span>
                          <span>EMAIL: {{ (record as any).rname }}</span>
                          <span>SERIAL: {{ (record as any).serial }}</span>
                        </div>
                      </template>
                      <template v-else-if="rt.type === 'SRV'">
                        <span class="data-priority">{{ (record as any).priority }}</span>
                        <span class="data-value">{{ (record as any).target }}:{{ (record as any).port }}</span>
                      </template>
                    </div>
                    <div class="row-meta">
                      <span class="ttl-badge">TTL {{ (record as any).ttl }}s</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </template>

        <!-- Empty State -->
        <div v-else-if="!loading" class="empty-state">
          <div class="empty-icon">◈</div>
          <div class="empty-title">READY</div>
          <div class="empty-subtitle">Enter a domain to resolve DNS records</div>
        </div>

        <!-- History -->
        <div class="history-section">
          <div class="section-divider">
            <span class="divider-line"></span>
            <span class="divider-text">HISTORY</span>
            <span class="divider-line"></span>
          </div>
          <div class="history-list">
            <div
              v-for="item in queryHistory.slice(0, 5)"
              :key="item.domain + item.dnsServer"
              class="history-item"
              @click="fillFromHistory(item)"
            >
              <span class="history-domain">{{ item.domain }}</span>
              <span class="history-server">{{ item.dnsServer || 'SYSTEM' }}</span>
              <span class="history-time">{{ formatHistoryTime(item.queriedAt) }}</span>
            </div>
            <div v-if="queryHistory.length === 0" class="history-empty">
              No queries yet
            </div>
          </div>
        </div>
      </el-tab-pane>

      <!-- Compare Tab -->
      <el-tab-pane name="compare">
        <template #label>
          <span class="tab-label">
            <span class="tab-dot compare"></span>
            BENCHMARK
          </span>
        </template>

        <div class="compare-section">
          <div class="input-card compact">
            <div class="input-row primary">
              <div class="input-label">TARGET</div>
              <el-input
                v-model="compareDomain"
                class="terminal-input"
                placeholder="domain.tld"
                clearable
                @keyup.enter="runCompare"
              >
                <template #prefix>
                  <span class="input-prompt">»</span>
                </template>
              </el-input>

              <button
                class="query-btn benchmark"
                :class="{ loading: compareLoading }"
                @click="runCompare"
              >
                <span v-if="!compareLoading">BENCHMARK</span>
                <span v-else class="blink">···</span>
              </button>
            </div>
          </div>

          <div class="server-select-panel">
            <div class="panel-title">DNS SERVERS</div>
            <div class="server-grid">
              <label
                v-for="s in compareServers"
                :key="s.key"
                class="server-checkbox"
                :class="{ checked: s.checked }"
              >
                <input
                  v-model="s.checked"
                  type="checkbox"
                  class="hidden-checkbox"
                >
                <span class="check-indicator">
                  <span v-if="s.checked" class="check-mark">◉</span>
                  <span v-else class="check-empty">○</span>
                </span>
                <span class="server-info">
                  <span class="server-name">{{ s.label }}</span>
                  <span v-if="s.ip" class="server-addr">{{ s.ip }}</span>
                  <span v-else class="server-addr system">SYSTEM</span>
                </span>
              </label>
            </div>

            <div class="custom-server-row">
              <el-input
                v-model="customServerInput"
                class="terminal-input small"
                placeholder="Custom IP"
                clearable
                @keyup.enter="addCustomServer"
              />
              <button class="add-btn" @click="addCustomServer">ADD</button>
            </div>
          </div>

          <!-- Compare Results -->
          <template v-if="compareResults.length > 0">
            <div class="benchmark-results">
              <div class="results-title">RESULTS</div>
              <div class="benchmark-grid">
                <div
                  v-for="(row, idx) in compareResults"
                  :key="row.ip"
                  class="benchmark-card"
                  :class="{ winner: idx === 0 && !row.error }"
                >
                  <div class="rank-badge" :class="getRankClass(idx, row.error)">
                    {{ row.error ? '✕' : '#' + (idx + 1) }}
                  </div>
                  <div class="benchmark-info">
                    <div class="bench-server">{{ row.server }}</div>
                    <div v-if="row.ip" class="bench-ip">{{ row.ip }}</div>
                  </div>
                  <div class="benchmark-stats">
                    <div
                      v-if="!row.error"
                      class="latency-display"
                      :class="getLatencyClass(row.elapsed_ms)"
                    >
                      {{ row.elapsed_ms }}<span class="unit">ms</span>
                    </div>
                    <div v-else class="error-display">FAILED</div>
                    <div v-if="row.addresses?.length" class="ip-count">
                      {{ row.addresses.length }} records
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </template>

          <div v-else-if="!compareLoading" class="empty-state small">
            <div class="empty-icon small">◈</div>
            <div class="empty-subtitle">Select servers and run benchmark</div>
          </div>
        </div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { getSettingJson, setSettingJson } from "../composables/useSettings";

interface DnsResult {
  domain: string;
  server: string;
  records: Record<string, Record<string, unknown>[]>;
  elapsed_ms: number;
}

interface ColumnDef {
  prop: string;
  label: string;
  width?: number;
}

interface RecordTypeDef {
  type: string;
  columns: ColumnDef[];
}

interface DnsHistoryEntry {
  domain: string;
  dnsServer: string;
  queriedAt: number;
}

interface SystemDnsResponse {
  ipv4?: string[];
  all?: string[];
}

interface CompareResult {
  server: string;
  ip: string;
  elapsed_ms: number | null;
  addresses: string[];
  error: string | null;
}

interface PresetDns {
  label: string;
  ip: string;
}

interface CompareServer {
  key: string;
  label: string;
  ip: string;
  checked: boolean;
}

const PRESET_DNS_SERVERS: PresetDns[] = [
  { label: "系统", ip: "" },
  { label: "Google", ip: "8.8.8.8" },
  { label: "Google2", ip: "8.8.4.4" },
  { label: "Cloudflare", ip: "1.1.1.1" },
  { label: "阿里云", ip: "223.5.5.5" },
  { label: "腾讯", ip: "119.29.29.29" },
  { label: "114DNS", ip: "114.114.114.114" },
];

const RECORD_TYPES: RecordTypeDef[] = [
  { type: "A", columns: [{ prop: "address", label: "IPv4 地址", width: 200 }] },
  { type: "AAAA", columns: [{ prop: "address", label: "IPv6 地址", width: 320 }] },
  { type: "CNAME", columns: [{ prop: "target", label: "目标", width: 300 }] },
  {
    type: "MX",
    columns: [
      { prop: "preference", label: "优先级", width: 80 },
      { prop: "exchange", label: "邮件服务器", width: 300 },
    ],
  },
  { type: "NS", columns: [{ prop: "host", label: "域名服务器", width: 300 }] },
  { type: "TXT", columns: [{ prop: "text", label: "文本内容", width: 500 }] },
  {
    type: "SOA",
    columns: [
      { prop: "mname", label: "主域名服务器", width: 200 },
      { prop: "rname", label: "管理邮箱", width: 200 },
      { prop: "serial", label: "序列号", width: 120 },
      { prop: "refresh", label: "刷新(s)", width: 90 },
      { prop: "retry", label: "重试(s)", width: 90 },
      { prop: "expire", label: "过期(s)", width: 90 },
      { prop: "minimum", label: "最小TTL", width: 90 },
    ],
  },
  {
    type: "SRV",
    columns: [
      { prop: "priority", label: "优先级", width: 80 },
      { prop: "weight", label: "权重", width: 80 },
      { prop: "port", label: "端口", width: 80 },
      { prop: "target", label: "目标", width: 300 },
    ],
  },
  { type: "PTR", columns: [{ prop: "hostname", label: "域名", width: 300 }] },
];

const DNS_QUERY_HISTORY_KEY = "dns_query_history";
const MAX_DNS_QUERY_HISTORY = 100;

const activeTab = ref("query");
const domain = ref("");
const dnsServer = ref("");
const loading = ref(false);
const result = ref<DnsResult | null>(null);
const systemDnsIpv4List = ref<string[]>([]);
const queryHistory = ref<DnsHistoryEntry[]>(loadDnsQueryHistory());
const selectedPreset = ref<string | null>(null);

const compareDomain = ref("");
const compareLoading = ref(false);
const compareResults = ref<CompareResult[]>([]);
const customServerInput = ref("");
const compareServers = ref<CompareServer[]>([
  { key: "system", label: "系统 DNS", ip: "", checked: true },
  { key: "google", label: "Google", ip: "8.8.8.8", checked: true },
  { key: "cloudflare", label: "Cloudflare", ip: "1.1.1.1", checked: true },
  { key: "aliyun", label: "阿里云", ip: "223.5.5.5", checked: true },
  { key: "tencent", label: "腾讯", ip: "119.29.29.29", checked: false },
  { key: "114dns", label: "114DNS", ip: "114.114.114.114", checked: false },
  { key: "google2", label: "Google2", ip: "8.8.4.4", checked: false },
]);

const visibleRecordTypes = computed(() => {
  return RECORD_TYPES.filter(rt => getRecords(rt.type).length > 0);
});

function getRecords(type: string): Record<string, unknown>[] {
  if (!result.value) return [];
  return result.value.records[type] ?? [];
}

function loadDnsQueryHistory(): DnsHistoryEntry[] {
  const parsed = getSettingJson<unknown[]>(DNS_QUERY_HISTORY_KEY, []);
  if (!Array.isArray(parsed)) return [];
  const entries = parsed.filter((item): item is DnsHistoryEntry => {
    const x = item as Record<string, unknown>;
    return (
      typeof x?.domain === "string" &&
      typeof x?.dnsServer === "string" &&
      typeof x?.queriedAt === "number"
    );
  });
  entries.sort((a, b) => b.queriedAt - a.queriedAt);
  return entries.slice(0, MAX_DNS_QUERY_HISTORY);
}

function persistDnsQueryHistory() {
  setSettingJson(DNS_QUERY_HISTORY_KEY, queryHistory.value);
}

function appendHistory(domainValue: string, dnsServerValue: string) {
  const normalizedDomain = domainValue.trim();
  const normalizedServer = dnsServerValue.trim();
  if (!normalizedDomain) return;

  const next: DnsHistoryEntry = {
    domain: normalizedDomain,
    dnsServer: normalizedServer,
    queriedAt: Date.now(),
  };
  const deduped = queryHistory.value.filter(
    (item) => !(item.domain === next.domain && item.dnsServer === next.dnsServer),
  );
  queryHistory.value = [next, ...deduped].slice(0, MAX_DNS_QUERY_HISTORY);
  persistDnsQueryHistory();
}

function fillFromHistory(item: DnsHistoryEntry) {
  domain.value = item.domain;
  dnsServer.value = item.dnsServer;
  selectedPreset.value = PRESET_DNS_SERVERS.find((p) => p.ip === item.dnsServer)?.label ?? null;
}

function formatHistoryTime(timestamp: number): string {
  const date = new Date(timestamp);
  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const dd = String(date.getDate()).padStart(2, "0");
  const hh = String(date.getHours()).padStart(2, "0");
  const mi = String(date.getMinutes()).padStart(2, "0");
  return `${mm}/${dd} ${hh}:${mi}`;
}

function selectPreset(preset: PresetDns) {
  if (preset.ip === "" && systemDnsIpv4List.value.length > 0) {
    dnsServer.value = systemDnsIpv4List.value[0];
  } else {
    dnsServer.value = preset.ip;
  }
  selectedPreset.value = preset.label;
}

function onServerInput() {
  selectedPreset.value = null;
}

function copyAddress(address: string) {
  navigator.clipboard.writeText(address).then(() => {
    ElMessage.success("已复制");
  }).catch(() => {
    ElMessage.error("复制失败");
  });
}

function copyAllIpv4() {
  const addresses = getRecords("A")
    .map((r) => String((r as Record<string, unknown>).address ?? ""))
    .filter(Boolean)
    .join("\n");
  copyAddress(addresses);
}

function addCustomServer() {
  const ip = customServerInput.value.trim();
  if (!ip) return;
  if (compareServers.value.some((s) => s.ip === ip)) {
    ElMessage.warning("该 DNS 服务器已在列表中");
    return;
  }
  compareServers.value.push({ key: `custom-${ip}`, label: ip, ip, checked: true });
  customServerInput.value = "";
}

function getLatencyClass(ms: number | null): string {
  if (ms === null) return "";
  if (ms < 100) return "fast";
  if (ms < 300) return "medium";
  return "slow";
}

function getRankClass(idx: number, hasError: boolean | null): string {
  if (hasError) return "error";
  if (idx === 0) return "gold";
  if (idx === 1) return "silver";
  if (idx === 2) return "bronze";
  return "";
}

async function loadSystemDnsDefaults() {
  try {
    const data = (await invokeToolByChannel("tool:dns:system-dns", {})) as SystemDnsResponse;
    const ipv4 = Array.isArray(data?.ipv4) ? data.ipv4.filter((v) => typeof v === "string") : [];
    systemDnsIpv4List.value = ipv4;
    if (!dnsServer.value.trim() && ipv4.length > 0) {
      dnsServer.value = ipv4[0];
      selectedPreset.value = PRESET_DNS_SERVERS.find((p) => p.ip === ipv4[0])?.label ?? null;
    }
  } catch {
    // ignore
  }
}

async function runQuery() {
  const d = domain.value.trim();
  if (!d) {
    ElMessage.warning("请输入域名");
    return;
  }

  result.value = null;
  loading.value = true;
  try {
    const data = await invokeToolByChannel("tool:dns:resolve", {
      domain: d,
      server: dnsServer.value.trim(),
    });
    result.value = data as DnsResult;
    appendHistory(d, dnsServer.value);
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

async function runCompare() {
  const d = compareDomain.value.trim();
  if (!d) {
    ElMessage.warning("请输入要测试的域名");
    return;
  }

  const checked = compareServers.value.filter((s) => s.checked);
  if (checked.length === 0) {
    ElMessage.warning("请至少选择一个 DNS 服务器");
    return;
  }

  compareResults.value = [];
  compareLoading.value = true;
  try {
    const data = await invokeToolByChannel("tool:dns:compare", {
      domain: d,
      servers: checked.map((s) => s.ip),
    });
    compareResults.value = data as CompareResult[];
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    compareLoading.value = false;
  }
}

onMounted(() => {
  loadSystemDnsDefaults();
});
</script>

<style scoped>
/* Deep Network Terminal Theme */

:root {
  --terminal-bg: #0a0c10;
  --terminal-panel: #11141a;
  --terminal-border: #1e232e;
  --terminal-accent: #ff9f43;
  --terminal-accent-dim: #c77d31;
  --terminal-green: #10b981;
  --terminal-yellow: #f59e0b;
  --terminal-red: #ef4444;
  --terminal-text: #e2e8f0;
  --terminal-text-dim: #64748b;
  --terminal-mono: 'JetBrains Mono', 'Fira Code', 'SF Mono', monospace;
}

.dns-panel {
  min-height: 100%;
  background: linear-gradient(180deg, #0d1117 0%, #070a0f 100%);
  color: var(--terminal-text);
  font-family: var(--terminal-mono);
  padding: 0;
}

/* Header */
.panel-header {
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--terminal-border);
  background: rgba(17, 20, 26, 0.6);
  backdrop-filter: blur(10px);
}

.header-title {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.title-icon {
  font-size: 24px;
  color: var(--terminal-accent);
  text-shadow: 0 0 20px rgba(255, 159, 67, 0.4);
}

.title-text {
  font-size: 18px;
  font-weight: 600;
  letter-spacing: 2px;
  color: var(--terminal-text);
}

.header-line {
  height: 2px;
  background: linear-gradient(90deg, var(--terminal-accent) 0%, transparent 60%);
  border-radius: 1px;
}

/* Tabs */
.dns-tabs :deep(.el-tabs__header) {
  margin: 0;
  border-bottom: 1px solid var(--terminal-border);
  background: rgba(17, 20, 26, 0.4);
  padding: 0 24px;
}

.dns-tabs :deep(.el-tabs__nav-wrap::after) {
  display: none;
}

.dns-tabs :deep(.el-tabs__item) {
  height: 48px;
  line-height: 48px;
  padding: 0 20px;
  color: var(--terminal-text-dim);
  font-size: 12px;
  letter-spacing: 1px;
  transition: all 0.3s ease;
}

.dns-tabs :deep(.el-tabs__item.is-active) {
  color: var(--terminal-accent);
}

.dns-tabs :deep(.el-tabs__item:hover) {
  color: var(--terminal-text);
}

.dns-tabs :deep(.el-tabs__active-bar) {
  background: var(--terminal-accent);
  height: 2px;
  box-shadow: 0 -2px 8px var(--terminal-accent);
}

.tab-label {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tab-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--terminal-green);
  box-shadow: 0 0 8px var(--terminal-green);
}

.tab-dot.compare {
  background: var(--terminal-accent);
  box-shadow: 0 0 8px var(--terminal-accent);
}

/* Query Section */
.query-section {
  padding: 24px;
}

.input-card {
  background: var(--terminal-panel);
  border: 1px solid var(--terminal-border);
  border-radius: 8px;
  padding: 20px;
  margin-bottom: 16px;
}

.input-card.compact {
  padding: 16px;
}

.input-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.input-row.primary {
  margin-bottom: 16px;
}

.input-label {
  font-size: 11px;
  color: var(--terminal-text-dim);
  letter-spacing: 2px;
  min-width: 60px;
}

/* Terminal Input Override */
:deep(.terminal-input .el-input__wrapper) {
  background: #0a0c10 !important;
  border: 1px solid var(--terminal-border) !important;
  box-shadow: inset 0 1px 3px rgba(0,0,0,0.5) !important;
  border-radius: 6px !important;
}

:deep(.terminal-input .el-input__inner) {
  color: var(--terminal-text) !important;
  font-family: var(--terminal-mono) !important;
  font-size: 14px !important;
}

:deep(.terminal-input .el-input__inner::placeholder) {
  color: var(--terminal-text-dim) !important;
}

:deep(.terminal-input.small .el-input__inner) {
  font-size: 13px !important;
}

.input-prompt {
  color: var(--terminal-accent);
  font-weight: bold;
  margin-right: 4px;
}

.input-prompt.secondary {
  color: var(--terminal-text-dim);
}

/* Query Button */
.query-btn {
  height: 40px;
  padding: 0 24px;
  background: linear-gradient(135deg, var(--terminal-accent) 0%, var(--terminal-accent-dim) 100%);
  border: none;
  border-radius: 6px;
  color: #0a0c10;
  font-family: var(--terminal-mono);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 1.5px;
  cursor: pointer;
  transition: all 0.3s ease;
  box-shadow: 0 4px 15px rgba(255, 159, 67, 0.3);
}

.query-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 6px 20px rgba(255, 159, 67, 0.4);
}

.query-btn:active {
  transform: translateY(0);
}

.query-btn.benchmark {
  background: linear-gradient(135deg, var(--terminal-green) 0%, #059669 100%);
  box-shadow: 0 4px 15px rgba(16, 185, 129, 0.3);
}

.query-btn.benchmark:hover {
  box-shadow: 0 6px 20px rgba(16, 185, 129, 0.4);
}

.query-btn.loading {
  background: var(--terminal-border);
  color: var(--terminal-text-dim);
  box-shadow: none;
  cursor: not-allowed;
}

.blink {
  animation: blink 1s infinite;
}

@keyframes blink {
  0%, 50% { opacity: 1; }
  51%, 100% { opacity: 0.3; }
}

/* Preset Grid */
.preset-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 12px;
}

.preset-chip {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--terminal-panel);
  border: 1px solid var(--terminal-border);
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.preset-chip:hover {
  border-color: var(--terminal-text-dim);
}

.preset-chip.active {
  border-color: var(--terminal-accent);
  background: rgba(255, 159, 67, 0.1);
  box-shadow: 0 0 12px rgba(255, 159, 67, 0.15);
}

.chip-indicator {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--terminal-text-dim);
}

.preset-chip.active .chip-indicator {
  background: var(--terminal-accent);
  box-shadow: 0 0 6px var(--terminal-accent);
}

.chip-label {
  font-size: 12px;
  color: var(--terminal-text);
}

.chip-ip {
  font-size: 11px;
  color: var(--terminal-text-dim);
  font-family: var(--terminal-mono);
}

/* System Hint */
.system-hint {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: rgba(16, 185, 129, 0.08);
  border: 1px solid rgba(16, 185, 129, 0.2);
  border-radius: 4px;
  font-size: 12px;
}

.hint-icon {
  color: var(--terminal-green);
}

.hint-text {
  color: var(--terminal-text-dim);
  font-family: var(--terminal-mono);
}

/* Loading */
.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 24px;
}

.loading-radar {
  position: relative;
  width: 80px;
  height: 80px;
  margin-bottom: 20px;
}

.radar-ring {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  border: 2px solid var(--terminal-accent);
  border-radius: 50%;
  opacity: 0;
  animation: radar 2s infinite;
}

.radar-ring:nth-child(1) {
  width: 20px;
  height: 20px;
  animation-delay: 0s;
}

.radar-ring:nth-child(2) {
  width: 50px;
  height: 50px;
  animation-delay: 0.4s;
}

.radar-ring:nth-child(3) {
  width: 80px;
  height: 80px;
  animation-delay: 0.8s;
}

@keyframes radar {
  0% { opacity: 0; transform: translate(-50%, -50%) scale(0.5); }
  50% { opacity: 0.6; }
  100% { opacity: 0; transform: translate(-50%, -50%) scale(1); }
}

.loading-text {
  font-size: 12px;
  letter-spacing: 3px;
  color: var(--terminal-text-dim);
}

/* Results */
.results-container {
  padding: 0 24px 24px;
}

.result-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
  flex-wrap: wrap;
}

.result-badge {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  background: var(--terminal-panel);
  border: 1px solid var(--terminal-border);
  border-radius: 6px;
}

.badge-label {
  font-size: 10px;
  color: var(--terminal-text-dim);
  letter-spacing: 1px;
}

.badge-value {
  font-family: var(--terminal-mono);
  font-size: 13px;
  color: var(--terminal-text);
}

.badge-value.fast {
  color: var(--terminal-green);
}

.badge-value.medium {
  color: var(--terminal-yellow);
}

.badge-value.slow {
  color: var(--terminal-red);
}

.copy-all-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  background: transparent;
  border: 1px solid var(--terminal-border);
  border-radius: 6px;
  color: var(--terminal-text-dim);
  font-family: var(--terminal-mono);
  font-size: 11px;
  letter-spacing: 1px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.copy-all-btn:hover {
  border-color: var(--terminal-accent);
  color: var(--terminal-accent);
  background: rgba(255, 159, 67, 0.1);
}

.copy-icon {
  font-size: 14px;
}

/* Records Grid */
.records-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 16px;
}

.record-card {
  background: var(--terminal-panel);
  border: 1px solid var(--terminal-border);
  border-radius: 8px;
  overflow: hidden;
}

.record-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: rgba(255, 159, 67, 0.05);
  border-bottom: 1px solid var(--terminal-border);
}

.record-type {
  font-size: 14px;
  font-weight: 700;
  color: var(--terminal-accent);
  letter-spacing: 1px;
}

.record-count {
  font-size: 11px;
  color: var(--terminal-text-dim);
  background: var(--terminal-bg);
  padding: 2px 8px;
  border-radius: 4px;
}

.record-table {
  padding: 8px;
}

.record-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: 4px;
  transition: background 0.2s ease;
}

.record-row:hover {
  background: rgba(255, 255, 255, 0.03);
}

.row-main {
  display: flex;
  align-items: center;
  gap: 12px;
}

.data-value {
  flex: 1;
  font-family: var(--terminal-mono);
  font-size: 13px;
  color: var(--terminal-text);
  word-break: break-all;
}

.data-value.mono {
  color: var(--terminal-accent);
}

.data-value.txt {
  font-size: 12px;
  color: var(--terminal-text-dim);
}

.data-priority {
  font-family: var(--terminal-mono);
  font-size: 12px;
  color: var(--terminal-green);
  min-width: 28px;
}

.row-copy-btn {
  padding: 4px 10px;
  background: transparent;
  border: 1px solid var(--terminal-border);
  border-radius: 4px;
  color: var(--terminal-text-dim);
  font-family: var(--terminal-mono);
  font-size: 10px;
  letter-spacing: 1px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.row-copy-btn:hover {
  border-color: var(--terminal-accent);
  color: var(--terminal-accent);
}

.row-meta {
  display: flex;
  align-items: center;
}

.ttl-badge {
  font-size: 10px;
  color: var(--terminal-text-dim);
  background: var(--terminal-bg);
  padding: 2px 8px;
  border-radius: 4px;
}

.soa-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  font-size: 11px;
  color: var(--terminal-text-dim);
}

/* Empty State */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 80px 24px;
}

.empty-state.small {
  padding: 40px 24px;
}

.empty-icon {
  font-size: 48px;
  color: var(--terminal-border);
  margin-bottom: 16px;
}

.empty-icon.small {
  font-size: 32px;
}

.empty-title {
  font-size: 18px;
  letter-spacing: 4px;
  color: var(--terminal-text);
  margin-bottom: 8px;
}

.empty-subtitle {
  font-size: 12px;
  color: var(--terminal-text-dim);
  letter-spacing: 1px;
}

/* History */
.history-section {
  padding: 0 24px 24px;
}

.section-divider {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 16px;
}

.divider-line {
  flex: 1;
  height: 1px;
  background: var(--terminal-border);
}

.divider-text {
  font-size: 10px;
  letter-spacing: 3px;
  color: var(--terminal-text-dim);
}

.history-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.history-item {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: var(--terminal-panel);
  border: 1px solid var(--terminal-border);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.history-item:hover {
  border-color: var(--terminal-accent);
  background: rgba(255, 159, 67, 0.05);
}

.history-domain {
  flex: 1;
  font-family: var(--terminal-mono);
  font-size: 13px;
  color: var(--terminal-text);
}

.history-server {
  font-family: var(--terminal-mono);
  font-size: 12px;
  color: var(--terminal-text-dim);
  min-width: 120px;
}

.history-time {
  font-size: 11px;
  color: var(--terminal-text-dim);
  min-width: 80px;
  text-align: right;
}

.history-empty {
  text-align: center;
  padding: 24px;
  color: var(--terminal-text-dim);
  font-size: 12px;
}

/* Compare Section */
.compare-section {
  padding: 24px;
}

.server-select-panel {
  background: var(--terminal-panel);
  border: 1px solid var(--terminal-border);
  border-radius: 8px;
  padding: 20px;
  margin-bottom: 20px;
}

.panel-title {
  font-size: 11px;
  letter-spacing: 2px;
  color: var(--terminal-text-dim);
  margin-bottom: 16px;
}

.server-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
  margin-bottom: 16px;
}

.server-checkbox {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px;
  background: var(--terminal-bg);
  border: 1px solid var(--terminal-border);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.server-checkbox:hover {
  border-color: var(--terminal-text-dim);
}

.server-checkbox.checked {
  border-color: var(--terminal-accent);
  background: rgba(255, 159, 67, 0.05);
}

.hidden-checkbox {
  display: none;
}

.check-indicator {
  font-size: 14px;
}

.check-mark {
  color: var(--terminal-accent);
}

.check-empty {
  color: var(--terminal-text-dim);
}

.server-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.server-name {
  font-size: 13px;
  color: var(--terminal-text);
}

.server-addr {
  font-family: var(--terminal-mono);
  font-size: 11px;
  color: var(--terminal-text-dim);
}

.server-addr.system {
  color: var(--terminal-green);
}

.add-btn {
  height: 32px;
  padding: 0 16px;
  background: var(--terminal-bg);
  border: 1px solid var(--terminal-border);
  border-radius: 4px;
  color: var(--terminal-text);
  font-family: var(--terminal-mono);
  font-size: 11px;
  letter-spacing: 1px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.add-btn:hover {
  border-color: var(--terminal-accent);
  color: var(--terminal-accent);
}

/* Benchmark Results */
.benchmark-results {
  margin-top: 24px;
}

.results-title {
  font-size: 11px;
  letter-spacing: 2px;
  color: var(--terminal-text-dim);
  margin-bottom: 16px;
}

.benchmark-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.benchmark-card {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px 20px;
  background: var(--terminal-panel);
  border: 1px solid var(--terminal-border);
  border-radius: 8px;
  transition: all 0.2s ease;
}

.benchmark-card.winner {
  border-color: var(--terminal-green);
  background: rgba(16, 185, 129, 0.08);
  box-shadow: 0 0 20px rgba(16, 185, 129, 0.1);
}

.rank-badge {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-family: var(--terminal-mono);
  font-size: 12px;
  font-weight: 700;
  background: var(--terminal-bg);
  border: 2px solid var(--terminal-border);
  color: var(--terminal-text-dim);
}

.rank-badge.gold {
  border-color: #ffd700;
  color: #ffd700;
  box-shadow: 0 0 12px rgba(255, 215, 0, 0.3);
}

.rank-badge.silver {
  border-color: #c0c0c0;
  color: #c0c0c0;
}

.rank-badge.bronze {
  border-color: #cd7f32;
  color: #cd7f32;
}

.rank-badge.error {
  border-color: var(--terminal-red);
  color: var(--terminal-red);
}

.benchmark-info {
  flex: 1;
}

.bench-server {
  font-size: 14px;
  color: var(--terminal-text);
  margin-bottom: 2px;
}

.bench-ip {
  font-family: var(--terminal-mono);
  font-size: 12px;
  color: var(--terminal-text-dim);
}

.benchmark-stats {
  text-align: right;
}

.latency-display {
  font-family: var(--terminal-mono);
  font-size: 24px;
  font-weight: 700;
  color: var(--terminal-text);
}

.latency-display .unit {
  font-size: 14px;
  font-weight: 400;
  color: var(--terminal-text-dim);
  margin-left: 4px;
}

.latency-display.fast {
  color: var(--terminal-green);
}

.latency-display.medium {
  color: var(--terminal-yellow);
}

.latency-display.slow {
  color: var(--terminal-red);
}

.error-display {
  font-family: var(--terminal-mono);
  font-size: 14px;
  color: var(--terminal-red);
  letter-spacing: 1px;
}

.ip-count {
  font-size: 11px;
  color: var(--terminal-text-dim);
  margin-top: 4px;
}

/* Error text */
.error-text {
  color: var(--terminal-red);
  font-size: 12px;
}

.empty-text {
  color: var(--terminal-text-dim);
}
</style>
