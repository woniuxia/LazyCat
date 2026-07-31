<template>
  <div class="panel-grid ports-panel">
    <div class="panel-grid-full">
      <el-button type="primary" :loading="loading" @click="loadPortUsage">
        {{ loading ? "查询中..." : "查询端口占用" }}
      </el-button>
    </div>

    <el-tabs class="panel-grid-full ports-tabs" v-loading="loading">
      <!-- Tab 1: 统计数据 -->
      <el-tab-pane label="统计数据">
        <div class="tab-content">
          <div class="stat-cards">
            <div class="stat-card">
              <span class="stat-card__label">总连接</span>
              <span class="stat-card__value">{{ portUsageSummary.total }}</span>
            </div>
            <div class="stat-card stat-card--tcp">
              <span class="stat-card__label">TCP</span>
              <span class="stat-card__value">{{ portUsageSummary.tcp }}</span>
            </div>
            <div class="stat-card stat-card--udp">
              <span class="stat-card__label">UDP</span>
              <span class="stat-card__value">{{ portUsageSummary.udp }}</span>
            </div>
          </div>

          <div v-if="portUsageStateRows.length > 0" class="state-distribution">
            <div class="section-label">连接状态分布</div>
            <div class="state-chips">
              <div
                v-for="row in portUsageStateRows"
                :key="row.state"
                class="state-chip"
                :class="stateChipClass(row.state)"
              >
                <span class="state-chip__name">{{ row.state }}</span>
                <span class="state-chip__count">{{ row.count }}</span>
              </div>
            </div>
          </div>

          <template v-if="portUsageSummary.total > 0">
            <!-- 协议分布 -->
            <div class="stats-section">
              <div class="section-label">协议分布</div>
              <div class="ratio-bar-row">
                <span class="ratio-label">TCP</span>
                <div class="ratio-track">
                  <div
                    class="ratio-fill ratio-fill--tcp"
                    :style="{ width: protocolRatio.tcp + '%' }"
                  />
                </div>
                <span class="ratio-value">{{ protocolRatio.tcp }}%</span>
              </div>
              <div class="ratio-bar-row">
                <span class="ratio-label">UDP</span>
                <div class="ratio-track">
                  <div
                    class="ratio-fill ratio-fill--udp"
                    :style="{ width: protocolRatio.udp + '%' }"
                  />
                </div>
                <span class="ratio-value">{{ protocolRatio.udp }}%</span>
              </div>
            </div>

            <!-- 热门端口 Top 10 -->
            <div v-if="topPorts.length > 0" class="stats-section">
              <div class="section-label">热门端口 Top 10</div>
              <div v-for="item in topPorts" :key="item.port" class="ratio-bar-row">
                <span class="ratio-label mono">:{{ item.port }}</span>
                <div class="ratio-track">
                  <div class="ratio-fill ratio-fill--tcp" :style="{ width: item.pct + '%' }" />
                </div>
                <span class="ratio-value">{{ item.count }}</span>
              </div>
            </div>

            <!-- 监听地址类型分布 -->
            <div class="stats-section">
              <div class="section-label">监听地址类型</div>
              <div v-for="item in bindingTypeStats" :key="item.label" class="ratio-bar-row">
                <span class="ratio-label">{{ item.label }}</span>
                <div class="ratio-track">
                  <div class="ratio-fill ratio-fill--success" :style="{ width: item.pct + '%' }" />
                </div>
                <span class="ratio-value">{{ item.count }}</span>
              </div>
            </div>

            <!-- 活跃进程 Top 5 -->
            <div v-if="topProcesses.length > 0" class="stats-section">
              <div class="section-label">活跃进程 Top 5</div>
              <div v-for="item in topProcesses" :key="item.pid" class="ratio-bar-row">
                <span class="ratio-label ratio-label--process">{{ item.processName }}</span>
                <div class="ratio-track">
                  <div class="ratio-fill ratio-fill--warning" :style="{ width: item.pct + '%' }" />
                </div>
                <span class="ratio-value">{{ item.connectionCount }}</span>
              </div>
            </div>
          </template>

          <div v-if="portUsageSummary.total === 0" class="empty-hint">
            点击"查询端口占用"加载数据
          </div>
        </div>
      </el-tab-pane>

      <!-- Tab 2: 按应用汇总 -->
      <el-tab-pane>
        <template #label>
          按应用汇总
          <el-badge
            v-if="portProcessRows.length > 0"
            :value="portProcessRows.length"
            class="tab-badge"
          />
        </template>
        <div class="tab-content">
          <el-input
            v-model="portFilter"
            placeholder="按端口/进程名/PID 过滤"
            clearable
            class="tab-filter"
          />
          <el-table :data="filteredPortProcessRows" border max-height="420">
            <el-table-column prop="processName" label="应用" min-width="160" />
            <el-table-column label="PID" width="90">
              <template #default="{ row }">
                <span class="mono">{{ row.pid }}</span>
              </template>
            </el-table-column>
            <el-table-column label="监听端口" min-width="200">
              <template #default="{ row }">
                <template v-if="row.listeningPorts.length > 0">
                  <el-tag
                    v-for="port in row.listeningPorts.slice(0, 5)"
                    :key="port"
                    size="small"
                    type="success"
                    class="port-tag"
                    >{{ port }}</el-tag
                  >
                  <span v-if="row.listeningPorts.length > 5" class="port-more">
                    +{{ row.listeningPorts.length - 5 }}
                  </span>
                </template>
                <span v-else class="text-dim">-</span>
              </template>
            </el-table-column>
            <el-table-column label="连接数" width="90">
              <template #default="{ row }">
                <span class="mono">{{ row.connectionCount }}</span>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="160" fixed="right">
              <template #default="{ row }">
                <el-space>
                  <el-button size="small" @click="showProcessDetail(row.pid)">详情</el-button>
                  <el-button size="small" type="danger" plain @click="killProcess(row.pid)"
                    >结束</el-button
                  >
                </el-space>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </el-tab-pane>

      <!-- Tab 3: 连接明细 -->
      <el-tab-pane>
        <template #label>
          连接明细
          <el-badge
            v-if="portConnectionRows.length > 0"
            :value="
              truncatedCount > 0 ? portConnectionRows.length + '+' : portConnectionRows.length
            "
            class="tab-badge"
          />
        </template>
        <div class="tab-content">
          <el-alert
            v-if="truncatedCount > 0"
            :title="`数据量过大，已截断显示前 ${MAX_CONNECTIONS} 条，共 ${portConnectionRows.length + truncatedCount} 条`"
            type="warning"
            show-icon
            :closable="false"
            class="truncate-alert"
          />
          <el-input
            v-model="connectionFilter"
            placeholder="按协议/PID/应用/地址/状态过滤"
            clearable
            class="tab-filter"
          />
          <el-table :data="filteredConnectionRows" border max-height="420">
            <el-table-column label="协议" width="80">
              <template #default="{ row }">
                <el-tag
                  size="small"
                  :type="row.protocol === 'TCP' ? 'primary' : 'warning'"
                  effect="dark"
                  >{{ row.protocol }}</el-tag
                >
              </template>
            </el-table-column>
            <el-table-column label="PID" width="80">
              <template #default="{ row }">
                <span class="mono">{{ row.pid }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="processName" label="应用" min-width="150" />
            <el-table-column label="本地地址" min-width="190">
              <template #default="{ row }">
                <span class="mono addr">{{ row.localAddress }}</span>
              </template>
            </el-table-column>
            <el-table-column label="远端地址" min-width="190">
              <template #default="{ row }">
                <span class="mono addr">{{ row.remoteAddress }}</span>
              </template>
            </el-table-column>
            <el-table-column label="状态" width="130">
              <template #default="{ row }">
                <el-tag size="small" :type="stateTagType(row.state)" effect="light">{{
                  row.state
                }}</el-tag>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </el-tab-pane>
    </el-tabs>

    <!-- 进程详情 Drawer -->
    <el-drawer v-model="detailVisible" title="进程详情" size="50%">
      <div v-if="processDetail" class="process-detail">
        <div class="detail-item">
          <span class="detail-item__label">PID</span>
          <span class="detail-item__value mono">{{ processDetail.pid }}</span>
        </div>
        <div class="detail-item">
          <span class="detail-item__label">名称</span>
          <span class="detail-item__value">{{ processDetail.name }}</span>
        </div>
        <div class="detail-item">
          <span class="detail-item__label">路径</span>
          <span class="detail-item__value mono detail-item__value--break">{{
            processDetail.path || "-"
          }}</span>
        </div>
        <div class="detail-item">
          <span class="detail-item__label">启动命令</span>
          <span class="detail-item__value mono detail-item__value--break">{{
            processDetail.commandLine || "-"
          }}</span>
        </div>
        <div class="detail-item">
          <span class="detail-item__label">启动时间</span>
          <span class="detail-item__value">{{ processDetail.startTime || "-" }}</span>
        </div>
      </div>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  PortUsageResponse,
  PortUsageSummary,
  PortUsageStateRow,
  PortUsageProcessRow,
  PortUsageConnectionRow,
  PortProcessDetailResponse,
} from "../types";

const MAX_CONNECTIONS = 1200;

const loading = ref(false);
const truncatedCount = ref(0);

const portUsageSummary = ref<PortUsageSummary>({ total: 0, tcp: 0, udp: 0 });
const portUsageStateRows = ref<PortUsageStateRow[]>([]);
const portProcessRows = ref<PortUsageProcessRow[]>([]);
const portConnectionRows = ref<PortUsageConnectionRow[]>([]);
const portFilter = ref("");
const connectionFilter = ref("");

const detailVisible = ref(false);
const processDetail = ref<PortProcessDetailResponse | null>(null);

onMounted(loadPortUsage);

// TCP/UDP 占比（百分比，保留一位小数）
const protocolRatio = computed(() => {
  const total = portUsageSummary.value.total;
  if (total === 0) return { tcp: 0, udp: 0 };
  return {
    tcp: Math.round((portUsageSummary.value.tcp / total) * 1000) / 10,
    udp: Math.round((portUsageSummary.value.udp / total) * 1000) / 10,
  };
});

// 热门端口 Top 10
const topPorts = computed(() => {
  const counts: Record<string, number> = {};
  for (const row of portConnectionRows.value) {
    const port = row.localAddress.split(":").pop() ?? "";
    if (port && /^\d+$/.test(port)) {
      counts[port] = (counts[port] ?? 0) + 1;
    }
  }
  const total = portConnectionRows.value.length || 1;
  return Object.entries(counts)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 10)
    .map(([port, count]) => ({ port, count, pct: Math.round((count / total) * 100) }));
});

// 监听地址类型分布
const bindingTypeStats = computed(() => {
  const listeningRows = portConnectionRows.value.filter(
    (r) => r.state === "LISTENING" || (r.protocol === "UDP" && r.remoteAddress === "*:*"),
  );
  let allInterfaces = 0,
    loopback = 0,
    specific = 0;
  for (const row of listeningRows) {
    const ip = row.localAddress.includes(":")
      ? row.localAddress.substring(0, row.localAddress.lastIndexOf(":"))
      : row.localAddress;
    if (ip === "0.0.0.0" || ip === "[::]" || ip === "::") allInterfaces++;
    else if (ip === "127.0.0.1" || ip === "::1") loopback++;
    else specific++;
  }
  const total = listeningRows.length || 1;
  return [
    { label: "全网卡", count: allInterfaces, pct: Math.round((allInterfaces / total) * 100) },
    { label: "仅本机", count: loopback, pct: Math.round((loopback / total) * 100) },
    { label: "指定网卡", count: specific, pct: Math.round((specific / total) * 100) },
  ];
});

// 活跃进程 Top 5
const topProcesses = computed(() => {
  const total = portUsageSummary.value.total || 1;
  return [...portProcessRows.value]
    .sort((a, b) => b.connectionCount - a.connectionCount)
    .slice(0, 5)
    .map((p) => ({ ...p, pct: Math.round((p.connectionCount / total) * 100) }));
});

const filteredPortProcessRows = computed(() => {
  const needle = portFilter.value.trim().toLowerCase();
  if (!needle) return portProcessRows.value;
  return portProcessRows.value.filter((row) => {
    return (
      row.listeningPorts.some((port) => port.includes(needle)) ||
      row.processName.toLowerCase().includes(needle) ||
      String(row.pid).includes(needle)
    );
  });
});

const filteredConnectionRows = computed(() => {
  const needle = connectionFilter.value.trim().toLowerCase();
  if (!needle) return portConnectionRows.value;
  return portConnectionRows.value.filter((row) => {
    return (
      row.protocol.toLowerCase().includes(needle) ||
      String(row.pid).includes(needle) ||
      row.processName.toLowerCase().includes(needle) ||
      row.localAddress.toLowerCase().includes(needle) ||
      row.remoteAddress.toLowerCase().includes(needle) ||
      row.state.toLowerCase().includes(needle)
    );
  });
});

function stateTagType(state: string): "success" | "primary" | "warning" | "danger" | "info" | "" {
  const s = (state || "").toUpperCase();
  if (s === "LISTENING") return "success";
  if (s === "ESTABLISHED") return "primary";
  if (s.includes("WAIT") || s.includes("FIN") || s.includes("SYN")) return "warning";
  if (s === "CLOSED") return "info";
  return "";
}

function stateChipClass(state: string): string {
  const s = (state || "").toUpperCase();
  if (s === "LISTENING") return "state-chip--listening";
  if (s === "ESTABLISHED") return "state-chip--established";
  if (s.includes("WAIT") || s.includes("FIN") || s.includes("SYN")) return "state-chip--wait";
  if (s === "CLOSED") return "state-chip--closed";
  return "";
}

async function loadPortUsage() {
  loading.value = true;
  truncatedCount.value = 0;
  try {
    const data = await invokeToolByChannel("tool:port:usage", {});
    const payload = (data ?? {}) as PortUsageResponse;

    const summary = payload.summary ?? { total: 0, tcp: 0, udp: 0 };
    const stateCounts = payload.stateCounts ?? {};
    const processSummaries = Array.isArray(payload.processSummaries)
      ? payload.processSummaries
      : [];
    const connections = Array.isArray(payload.connections) ? payload.connections : [];

    portUsageSummary.value = {
      total: summary.total ?? connections.length,
      tcp: summary.tcp ?? 0,
      udp: summary.udp ?? 0,
    };

    portUsageStateRows.value = Object.entries(stateCounts)
      .map(([state, count]) => ({ state, count }))
      .sort((a, b) => b.count - a.count);

    portProcessRows.value = processSummaries.map((item) => ({
      pid: item.pid ?? 0,
      processName: item.processName ?? "UNKNOWN",
      listeningPorts: item.listeningPorts ?? [],
      listeningPortsText: (item.listeningPorts ?? []).join(", ") || "-",
      connectionCount: item.connectionCount ?? 0,
    }));

    if (connections.length > MAX_CONNECTIONS) {
      truncatedCount.value = connections.length - MAX_CONNECTIONS;
    }

    portConnectionRows.value = connections.slice(0, MAX_CONNECTIONS).map((item) => ({
      protocol: item.protocol ?? "",
      pid: item.pid ?? 0,
      processName: item.processName ?? "UNKNOWN",
      localAddress: item.localAddress ?? "",
      remoteAddress: item.remoteAddress ?? "",
      state: item.state ?? "-",
    }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    loading.value = false;
  }
}

async function showProcessDetail(pid: number) {
  try {
    const data = (await invokeToolByChannel("tool:port:process-detail", {
      pid,
    })) as PortProcessDetailResponse;
    processDetail.value = data;
    detailVisible.value = true;
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function killProcess(pid: number) {
  try {
    await ElMessageBox.confirm(`确定结束进程 PID=${pid} 吗？`, "结束进程", {
      type: "warning",
      confirmButtonText: "结束",
      cancelButtonText: "取消",
    });
    await invokeToolByChannel("tool:port:kill", { pid, force: true });
    ElMessage.success(`已结束进程 ${pid}`);
    await loadPortUsage();
  } catch (error) {
    const message = (error as Error).message;
    if (message && !message.toLowerCase().includes("cancel")) {
      ElMessage.error(message);
    }
  }
}
</script>

<style scoped>
/* 概览卡片 */
.stat-cards {
  display: flex;
  gap: 12px;
}

.stat-card {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 12px 16px;
  border: 1px solid var(--el-border-color);
  border-radius: var(--el-border-radius-base);
  background: var(--el-fill-color-extra-light);
}

.stat-card--tcp {
  border-color: var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
}

.stat-card--udp {
  border-color: var(--el-color-warning-light-5);
  background: var(--el-color-warning-light-9);
}

.stat-card__label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-weight: 500;
  letter-spacing: 0.03em;
}

.stat-card__value {
  font-size: 26px;
  font-weight: 700;
  color: var(--el-text-color-primary);
  font-family: "Cascadia Code", "Consolas", "Courier New", monospace;
  line-height: 1;
}

.stat-card--tcp .stat-card__value {
  color: var(--el-color-primary);
}

.stat-card--udp .stat-card__value {
  color: var(--el-color-warning);
}

/* 状态分布 */
.state-distribution {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.section-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.state-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.state-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 10px 3px 8px;
  border-radius: 12px;
  border: 1px solid var(--el-border-color);
  background: var(--el-fill-color-light);
  font-size: 12px;
  line-height: 1.6;
}

.state-chip--listening {
  border-color: var(--el-color-success-light-5);
  background: var(--el-color-success-light-9);
  color: var(--el-color-success-dark-2);
}

.state-chip--established {
  border-color: var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary-dark-2);
}

.state-chip--wait {
  border-color: var(--el-color-warning-light-5);
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning-dark-2);
}

.state-chip--closed {
  border-color: var(--el-border-color);
  background: var(--el-fill-color);
  color: var(--el-text-color-placeholder);
}

.state-chip__name {
  font-weight: 500;
}

.state-chip__count {
  font-family: "Cascadia Code", "Consolas", "Courier New", monospace;
  font-weight: 700;
  font-size: 13px;
  min-width: 1.5em;
  text-align: right;
}

/* Tabs */
.ports-tabs {
  width: 100%;
}

.tab-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-top: 4px;
}

.tab-filter {
  width: 100%;
}

.tab-badge {
  margin-left: 6px;
}

.truncate-alert {
  padding: 6px 12px;
}

.empty-hint {
  padding: 32px 0;
  text-align: center;
  color: var(--el-text-color-placeholder);
  font-size: 13px;
}

/* 数据字体 */
.mono {
  font-family: "Cascadia Code", "Consolas", "Courier New", monospace;
  font-size: 13px;
}

.addr {
  font-size: 12px;
  color: var(--el-text-color-regular);
}

.text-dim {
  color: var(--el-text-color-placeholder);
}

/* 端口标签 */
.port-tag {
  margin-right: 4px;
  margin-bottom: 2px;
  font-family: "Cascadia Code", "Consolas", "Courier New", monospace;
}

.port-more {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  margin-left: 2px;
}

/* 统计区块 */
.stats-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ratio-bar-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ratio-label {
  width: 80px;
  font-size: 12px;
  color: var(--el-text-color-regular);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 0;
}

.ratio-label--process {
  width: 120px;
}

.ratio-track {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  background: var(--el-fill-color);
  overflow: hidden;
}

.ratio-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.4s ease;
  min-width: 2px;
}

.ratio-fill--tcp {
  background: var(--el-color-primary);
}

.ratio-fill--udp {
  background: var(--el-color-warning);
}

.ratio-fill--success {
  background: var(--el-color-success);
}

.ratio-fill--warning {
  background: var(--el-color-warning);
}

.ratio-value {
  width: 44px;
  text-align: right;
  font-size: 12px;
  font-family: "Cascadia Code", "Consolas", "Courier New", monospace;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}

/* 进程详情 Drawer */
.process-detail {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.detail-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 0;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.detail-item:last-child {
  border-bottom: none;
}

.detail-item__label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--el-text-color-placeholder);
}

.detail-item__value {
  font-size: 14px;
  color: var(--el-text-color-primary);
  line-height: 1.5;
}

.detail-item__value--break {
  word-break: break-all;
  font-size: 12px;
}
</style>
