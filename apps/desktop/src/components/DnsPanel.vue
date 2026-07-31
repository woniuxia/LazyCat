<template>
  <div class="dns-panel">
    <el-tabs v-model="activeTab" class="dns-tabs">
      <!-- 查询 Tab -->
      <el-tab-pane label="DNS 查询" name="query">
        <div class="dns-controls">
          <div class="input-row">
            <el-input
              v-model="domain"
              placeholder="输入域名或 IP（IP 自动查询 PTR 反向记录）"
              style="flex: 1; min-width: 220px"
              clearable
              @keyup.enter="runQuery"
            />
            <el-input
              v-model="dnsServer"
              placeholder="DNS 服务器（留空使用系统DNS）"
              style="width: 240px"
              clearable
              @input="onServerInput"
            />
            <el-button type="primary" :loading="loading" @click="runQuery">查询</el-button>
          </div>
          <div class="preset-dns-row">
            <el-button
              v-for="preset in PRESET_DNS_SERVERS"
              :key="preset.label"
              size="small"
              :type="selectedPreset === preset.label ? 'primary' : 'default'"
              @click="selectPreset(preset)"
              >{{ preset.label }}</el-button
            >
          </div>
          <div v-if="systemDnsHint" class="system-dns-hint">系统 IPv4 DNS: {{ systemDnsHint }}</div>
        </div>

        <div v-if="loading" class="dns-loading-placeholder">
          <el-skeleton animated :rows="6" />
        </div>

        <template v-else-if="result">
          <div class="result-meta">
            <el-tag size="small" type="info">DNS: {{ result.server }}</el-tag>
            <el-tag size="small" type="info">耗时: {{ result.elapsed_ms }} ms</el-tag>
            <el-button v-if="getRecords('A').length > 1" size="small" @click="copyAllIpv4"
              >复制全部 IPv4</el-button
            >
          </div>

          <template v-for="rt in RECORD_TYPES" :key="rt.type">
            <div v-if="getRecords(rt.type).length > 0" class="record-section">
              <el-divider content-position="left">{{ rt.type }} 记录</el-divider>
              <el-table :data="getRecords(rt.type)" size="small" border stripe>
                <el-table-column
                  v-for="col in rt.columns"
                  :key="col.prop"
                  :prop="col.prop"
                  :label="col.label"
                  :min-width="col.width"
                  show-overflow-tooltip
                />
                <el-table-column prop="ttl" label="TTL (s)" width="100" />
                <el-table-column v-if="rt.type === 'A'" label="" width="66" align="center">
                  <template #default="{ row }">
                    <el-button size="small" text @click="copyAddress(String(row.address))"
                      >复制</el-button
                    >
                  </template>
                </el-table-column>
              </el-table>
            </div>
          </template>

          <div v-if="noRecords" class="empty-hint">未查询到任何 DNS 记录</div>
        </template>

        <div v-else-if="!loading" class="empty-hint">
          输入域名后点击"查询"，获取 A/AAAA/CNAME/MX/NS/TXT/SOA/SRV/PTR 记录
        </div>

        <div class="record-section">
          <el-divider content-position="left">历史查询</el-divider>
          <el-table :data="queryHistory" size="small" border stripe>
            <el-table-column prop="domain" label="域名" min-width="220" show-overflow-tooltip />
            <el-table-column
              prop="dnsServer"
              label="DNS 服务器"
              min-width="180"
              show-overflow-tooltip
            />
            <el-table-column label="上一次查询时间" width="180">
              <template #default="{ row }">
                {{ formatHistoryTime(row.queriedAt) }}
              </template>
            </el-table-column>
            <el-table-column label="操作" width="96" align="center">
              <template #default="{ row }">
                <el-button size="small" @click="fillFromHistory(row)">回填</el-button>
              </template>
            </el-table-column>
          </el-table>
          <div v-if="queryHistory.length === 0" class="empty-hint history-empty">
            暂无历史查询记录
          </div>
        </div>
      </el-tab-pane>

      <!-- 性能对比 Tab -->
      <el-tab-pane label="服务器性能对比" name="compare">
        <div class="compare-controls">
          <div class="input-row">
            <el-input
              v-model="compareDomain"
              placeholder="输入要测试的域名，如 example.com"
              style="flex: 1; min-width: 220px"
              clearable
              @keyup.enter="runCompare"
            />
            <el-button type="primary" :loading="compareLoading" @click="runCompare"
              >开始对比</el-button
            >
          </div>

          <div class="compare-servers-label">选择要对比的 DNS 服务器：</div>
          <div class="compare-server-list">
            <el-checkbox
              v-for="s in compareServers"
              :key="s.key"
              v-model="s.checked"
              class="compare-server-item"
            >
              <span class="server-label">{{ s.label }}</span>
              <span v-if="s.ip" class="server-ip">{{ s.ip }}</span>
            </el-checkbox>
          </div>

          <div class="custom-server-row">
            <el-input
              v-model="customServerInput"
              placeholder="自定义 DNS IP（如 180.76.76.76）"
              style="flex: 1"
              clearable
              @keyup.enter="addCustomServer"
            />
            <el-button @click="addCustomServer">添加</el-button>
          </div>
        </div>

        <div v-if="compareLoading" class="dns-loading-placeholder">
          <el-skeleton animated :rows="4" />
        </div>

        <template v-else-if="compareResults.length > 0">
          <el-divider content-position="left">对比结果（按响应时间升序）</el-divider>
          <el-table :data="compareResults" size="small" border stripe>
            <el-table-column label="排名" width="60" align="center">
              <template #default="{ $index }">{{ $index + 1 }}</template>
            </el-table-column>
            <el-table-column prop="server" label="DNS 名称" min-width="100" />
            <el-table-column prop="ip" label="IP" min-width="140" show-overflow-tooltip />
            <el-table-column label="响应时间" width="120" align="center">
              <template #default="{ row }">
                <el-tag v-if="row.error" type="danger" size="small">失败</el-tag>
                <el-tag v-else-if="row.elapsed_ms < 100" type="success" size="small"
                  >{{ row.elapsed_ms }} ms</el-tag
                >
                <el-tag v-else-if="row.elapsed_ms < 300" type="warning" size="small"
                  >{{ row.elapsed_ms }} ms</el-tag
                >
                <el-tag v-else type="danger" size="small">{{ row.elapsed_ms }} ms</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="A 记录" min-width="200" show-overflow-tooltip>
              <template #default="{ row }">
                <span v-if="row.addresses && row.addresses.length > 0">{{
                  row.addresses.join(", ")
                }}</span>
                <span v-else-if="row.error" class="error-text">{{ row.error }}</span>
                <span v-else class="empty-text">—</span>
              </template>
            </el-table-column>
          </el-table>
        </template>

        <div v-else-if="!compareLoading" class="empty-hint">
          选择 DNS 服务器并输入域名后点击"开始对比"
        </div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { ElMessage } from "element-plus";
import { getSettingJson, setSettingJson } from "../composables/useSettings";
import { useToolInvoke } from "../composables/useToolInvoke";

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

// 查询 Tab 状态
const activeTab = ref("query");
const domain = ref("");
const dnsServer = ref("");
const result = ref<DnsResult | null>(null);
const systemDnsIpv4List = ref<string[]>([]);
const queryHistory = ref<DnsHistoryEntry[]>(loadDnsQueryHistory());
const selectedPreset = ref<string | null>(null);
const { loading, invokeWithLoading: invokeQuery } = useToolInvoke();
const { invokeSilent } = useToolInvoke();

// 性能对比 Tab 状态
const compareDomain = ref("");
const compareResults = ref<CompareResult[]>([]);
const customServerInput = ref("");
const { loading: compareLoading, invokeWithLoading: invokeCompare } = useToolInvoke();
const compareServers = ref<CompareServer[]>([
  { key: "system", label: "系统 DNS", ip: "", checked: true },
  { key: "google", label: "Google", ip: "8.8.8.8", checked: true },
  { key: "cloudflare", label: "Cloudflare", ip: "1.1.1.1", checked: true },
  { key: "aliyun", label: "阿里云", ip: "223.5.5.5", checked: true },
  { key: "tencent", label: "腾讯", ip: "119.29.29.29", checked: false },
  { key: "114dns", label: "114DNS", ip: "114.114.114.114", checked: false },
  { key: "google2", label: "Google2", ip: "8.8.4.4", checked: false },
]);

const noRecords = computed(() => {
  if (!result.value) return false;
  return RECORD_TYPES.every((rt) => getRecords(rt.type).length === 0);
});

const systemDnsHint = computed(() => {
  if (systemDnsIpv4List.value.length === 0) return "";
  return systemDnsIpv4List.value.join(", ");
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
  const yyyy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const dd = String(date.getDate()).padStart(2, "0");
  const hh = String(date.getHours()).padStart(2, "0");
  const mi = String(date.getMinutes()).padStart(2, "0");
  const ss = String(date.getSeconds()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}:${ss}`;
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
  // 手动修改时清除预置高亮
  selectedPreset.value = null;
}

function copyAddress(address: string) {
  navigator.clipboard
    .writeText(address)
    .then(() => {
      ElMessage.success("已复制");
    })
    .catch(() => {
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

async function loadSystemDnsDefaults() {
  // 系统 DNS 只是输入默认值，读取失败不阻断手动查询。
  const data = await invokeSilent<SystemDnsResponse>("tool:dns:system-dns", {});
  if (!data) return;
  const ipv4 = Array.isArray(data.ipv4) ? data.ipv4.filter((v) => typeof v === "string") : [];
  systemDnsIpv4List.value = ipv4;
  if (!dnsServer.value.trim() && ipv4.length > 0) {
    dnsServer.value = ipv4[0];
    selectedPreset.value = PRESET_DNS_SERVERS.find((p) => p.ip === ipv4[0])?.label ?? null;
  }
}

async function runQuery() {
  const d = domain.value.trim();
  if (!d) {
    ElMessage.warning("请输入域名");
    return;
  }

  result.value = null;
  const data = await invokeQuery<DnsResult>(
    "tool:dns:resolve",
    {
      domain: d,
      server: dnsServer.value.trim(),
    },
    { errorPrefix: "查询失败：" },
  );
  if (!data) return;
  result.value = data;
  appendHistory(d, dnsServer.value);
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
  const data = await invokeCompare<CompareResult[]>(
    "tool:dns:compare",
    {
      domain: d,
      servers: checked.map((s) => s.ip),
    },
    { errorPrefix: "对比失败：" },
  );
  if (!data) return;
  compareResults.value = data;
}

onMounted(() => {
  loadSystemDnsDefaults();
});
</script>

<style scoped>
.dns-panel {
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0;
  background: #ffffff;
  min-height: 100vh;
  font-family: var(--lc-font-mono);
  position: relative;
  overflow: hidden;
}

.dns-tabs {
  flex: 1;
  position: relative;
  z-index: 1;
}

.dns-tabs :deep(.el-tabs__header) {
  background: #f8f9fa;
  border-bottom: 2px solid #0066cc;
  margin: 0;
  padding: 0 24px;
  box-shadow: 0 2px 8px rgba(0, 102, 204, 0.1);
}

.dns-tabs :deep(.el-tabs__nav-wrap::after) {
  display: none;
}

.dns-tabs :deep(.el-tabs__item) {
  color: #666;
  font-family: var(--lc-font-display);
  font-weight: 600;
  font-size: 14px;
  letter-spacing: 1px;
  text-transform: uppercase;
  padding: 0 24px;
  height: 50px;
  line-height: 50px;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
}

.dns-tabs :deep(.el-tabs__item::before) {
  content: "";
  position: absolute;
  bottom: 0;
  left: 50%;
  width: 0;
  height: 2px;
  background: linear-gradient(90deg, #0066cc, #00aaff);
  transform: translateX(-50%);
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.dns-tabs :deep(.el-tabs__item:hover) {
  color: #0066cc;
}

.dns-tabs :deep(.el-tabs__item.is-active) {
  color: #0066cc;
}

.dns-tabs :deep(.el-tabs__item.is-active::before) {
  width: 100%;
}

.dns-tabs :deep(.el-tabs__active-bar) {
  display: none;
}

.dns-tabs :deep(.el-tabs__content) {
  padding: 32px 24px;
}

.dns-controls,
.compare-controls {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-bottom: 32px;
  background: #f8f9fa;
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  padding: 24px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  position: relative;
  overflow: hidden;
}

.dns-controls::before,
.compare-controls::before {
  content: "";
  position: absolute;
  top: 0;
  left: -100%;
  width: 100%;
  height: 100%;
  background: linear-gradient(90deg, transparent, rgba(0, 102, 204, 0.05), transparent);
  animation: shimmer 3s infinite;
}

@keyframes shimmer {
  0% {
    left: -100%;
  }
  100% {
    left: 100%;
  }
}

.input-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.input-row :deep(.el-input__wrapper) {
  background: #ffffff;
  border: 1px solid #d0d0d0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
  transition: all 0.3s ease;
}

.input-row :deep(.el-input__wrapper:hover) {
  border-color: #0066cc;
  box-shadow: 0 1px 3px rgba(0, 102, 204, 0.1);
}

.input-row :deep(.el-input__wrapper.is-focus) {
  border-color: #0066cc;
  box-shadow: 0 0 0 3px rgba(0, 102, 204, 0.1);
}

.input-row :deep(.el-input__inner) {
  color: #333;
  font-family: var(--lc-font-mono);
  font-size: 13px;
}

.input-row :deep(.el-input__inner::placeholder) {
  color: #999;
}

.input-row :deep(.el-button) {
  background: linear-gradient(135deg, #0066cc 0%, #0088ff 100%);
  border: none;
  color: #ffffff;
  font-family: var(--lc-font-display);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 1px;
  padding: 12px 28px;
  box-shadow: 0 2px 8px rgba(0, 102, 204, 0.3);
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
  overflow: hidden;
}

.input-row :deep(.el-button::before) {
  content: "";
  position: absolute;
  top: 50%;
  left: 50%;
  width: 0;
  height: 0;
  background: rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  transform: translate(-50%, -50%);
  transition:
    width 0.6s,
    height 0.6s;
}

.input-row :deep(.el-button:hover) {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 102, 204, 0.4);
}

.input-row :deep(.el-button:hover::before) {
  width: 300px;
  height: 300px;
}

.input-row :deep(.el-button:active) {
  transform: translateY(0);
}

.preset-dns-row {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
}

.preset-dns-row :deep(.el-button) {
  background: #ffffff;
  border: 1px solid #d0d0d0;
  color: #666;
  font-family: var(--lc-font-mono);
  font-size: 12px;
  padding: 8px 16px;
  transition: all 0.3s ease;
}

.preset-dns-row :deep(.el-button:hover) {
  border-color: #0066cc;
  color: #0066cc;
  background: #f0f7ff;
}

.preset-dns-row :deep(.el-button--primary) {
  background: linear-gradient(135deg, #0066cc 0%, #0088ff 100%);
  border: none;
  color: #ffffff;
  box-shadow: 0 2px 8px rgba(0, 102, 204, 0.3);
}

.result-meta {
  display: flex;
  gap: 12px;
  margin-bottom: 20px;
  flex-wrap: wrap;
  align-items: center;
}

.result-meta :deep(.el-tag) {
  background: #f0f7ff;
  border: 1px solid #b3d9ff;
  color: #0066cc;
  font-family: var(--lc-font-mono);
  font-size: 12px;
  padding: 6px 14px;
}

.result-meta :deep(.el-button) {
  background: #fff0f6;
  border: 1px solid #ffb3d9;
  color: #cc0066;
  font-family: var(--lc-font-mono);
  font-size: 12px;
  transition: all 0.3s ease;
}

.result-meta :deep(.el-button:hover) {
  background: #ffe0f0;
  box-shadow: 0 2px 8px rgba(204, 0, 102, 0.2);
}

.system-dns-hint {
  color: #666;
  font-size: 12px;
  font-family: var(--lc-font-mono);
  padding: 8px 12px;
  background: #f0f7ff;
  border-left: 3px solid #0066cc;
  border-radius: 4px;
}

.record-section {
  margin-bottom: 24px;
  animation: fadeInUp 0.6s ease-out;
}

@keyframes fadeInUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.record-section :deep(.el-divider) {
  border-color: #e0e0e0;
  margin: 24px 0 16px;
}

.record-section :deep(.el-divider__text) {
  background: transparent;
  color: #0066cc;
  font-family: var(--lc-font-display);
  font-weight: 600;
  font-size: 14px;
  letter-spacing: 2px;
  text-transform: uppercase;
  padding: 0 16px;
}

.record-section :deep(.el-table) {
  background: #ffffff;
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  overflow: hidden;
}

.record-section :deep(.el-table__header-wrapper) {
  background: #f8f9fa;
}

.record-section :deep(.el-table th) {
  background: transparent;
  color: #0066cc;
  font-family: var(--lc-font-display);
  font-weight: 600;
  font-size: 12px;
  letter-spacing: 1px;
  text-transform: uppercase;
  border-color: #e0e0e0;
}

.record-section :deep(.el-table td) {
  border-color: #f0f0f0;
  color: #333;
  font-family: var(--lc-font-mono);
  font-size: 13px;
}

.record-section :deep(.el-table__row) {
  background: transparent;
  transition: all 0.3s ease;
}

.record-section :deep(.el-table__row:hover) {
  background: #f0f7ff;
}

.record-section :deep(.el-table--striped .el-table__body tr.el-table__row--striped) {
  background: #fafafa;
}

.record-section :deep(.el-button--text) {
  color: #cc0066;
  font-family: var(--lc-font-mono);
  font-size: 12px;
  transition: all 0.3s ease;
}

.record-section :deep(.el-button--text:hover) {
  color: #cc0066;
}

.dns-loading-placeholder {
  margin-top: 20px;
  padding: 24px;
  background: #f8f9fa;
  border: 1px solid #e0e0e0;
  border-radius: 8px;
}

.dns-loading-placeholder :deep(.el-skeleton__item) {
  background: linear-gradient(90deg, #f0f0f0 25%, #e0e0e0 50%, #f0f0f0 75%);
  background-size: 200% 100%;
  animation: loading 1.5s ease-in-out infinite;
}

@keyframes loading {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}

.empty-hint {
  color: #999;
  font-size: 14px;
  text-align: center;
  padding: 48px 24px;
  font-family: var(--lc-font-mono);
  background: #f8f9fa;
  border: 1px dashed #d0d0d0;
  border-radius: 8px;
  margin: 20px 0;
}

.history-empty {
  padding: 24px 0;
  margin: 0;
}

.compare-servers-label {
  font-size: 13px;
  color: #666;
  font-family: var(--lc-font-display);
  font-weight: 600;
  letter-spacing: 1px;
  text-transform: uppercase;
  margin-top: 8px;
}

.compare-server-list {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 20px;
}

.compare-server-item {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.compare-server-item :deep(.el-checkbox__input.is-checked .el-checkbox__inner) {
  background: linear-gradient(135deg, #0066cc 0%, #0088ff 100%);
  border-color: #0066cc;
}

.compare-server-item :deep(.el-checkbox__inner) {
  background: #ffffff;
  border-color: #d0d0d0;
}

.compare-server-item :deep(.el-checkbox__label) {
  color: #333;
  font-family: var(--lc-font-mono);
  font-size: 13px;
}

.server-label {
  font-weight: 500;
  color: #333;
}

.server-ip {
  color: #999;
  font-size: 12px;
  font-family: var(--lc-font-mono);
}

.custom-server-row {
  display: flex;
  gap: 12px;
  align-items: center;
  max-width: 500px;
}

.error-text {
  color: #cc0033;
  font-size: 12px;
  font-family: var(--lc-font-mono);
}

.empty-text {
  color: #ccc;
}

.record-section :deep(.el-tag--success) {
  background: #e6f7ed;
  border-color: #66cc88;
  color: #00aa44;
}

.record-section :deep(.el-tag--warning) {
  background: #fff7e6;
  border-color: #ffcc66;
  color: #cc8800;
}

.record-section :deep(.el-tag--danger) {
  background: #ffe6ed;
  border-color: #ff6688;
  color: #cc0033;
}
</style>
