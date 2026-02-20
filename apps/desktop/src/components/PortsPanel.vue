<template>
  <div class="panel-grid">
    <div class="panel-grid-full">
      <el-button type="primary" @click="loadPortUsage">查询端口占用</el-button>
    </div>
    <el-divider class="panel-grid-full" content-position="left">概览</el-divider>
    <el-descriptions class="panel-grid-full" :column="3" border>
      <el-descriptions-item label="总连接">{{ portUsageSummary.total }}</el-descriptions-item>
      <el-descriptions-item label="TCP">{{ portUsageSummary.tcp }}</el-descriptions-item>
      <el-descriptions-item label="UDP">{{ portUsageSummary.udp }}</el-descriptions-item>
    </el-descriptions>
    <el-table class="panel-grid-full" :data="portUsageStateRows" border>
      <el-table-column prop="state" label="状态" />
      <el-table-column prop="count" label="数量" width="120" />
    </el-table>
    <el-divider class="panel-grid-full" content-position="left">按应用汇总</el-divider>
    <el-input
      class="panel-grid-full"
      v-model="portFilter"
      placeholder="按端口筛选应用汇总，例如 5173"
      clearable
    />
    <el-table class="panel-grid-full" :data="filteredPortProcessRows" border max-height="280">
      <el-table-column prop="processName" label="应用" min-width="180" />
      <el-table-column prop="pid" label="PID" width="100" />
      <el-table-column prop="listeningPortsText" label="监听端口" min-width="220" />
      <el-table-column prop="connectionCount" label="连接数" width="120" />
    </el-table>
    <el-divider class="panel-grid-full" content-position="left">连接明细</el-divider>
    <el-table class="panel-grid-full" :data="portConnectionRows" border max-height="360">
      <el-table-column prop="protocol" label="协议" width="90" />
      <el-table-column prop="pid" label="PID" width="90" />
      <el-table-column prop="processName" label="应用" min-width="180" />
      <el-table-column prop="localAddress" label="本地地址" min-width="220" />
      <el-table-column prop="remoteAddress" label="远端地址" min-width="220" />
      <el-table-column prop="state" label="状态" width="130" />
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  PortUsageSummary,
  PortUsageStateRow,
  PortUsageProcessRow,
  PortUsageConnectionRow,
} from "../types";

const portUsageSummary = ref<PortUsageSummary>({ total: 0, tcp: 0, udp: 0 });
const portUsageStateRows = ref<PortUsageStateRow[]>([]);
const portProcessRows = ref<PortUsageProcessRow[]>([]);
const portConnectionRows = ref<PortUsageConnectionRow[]>([]);
const portFilter = ref("");

const filteredPortProcessRows = computed(() => {
  const needle = portFilter.value.trim();
  if (!needle) return portProcessRows.value;
  return portProcessRows.value.filter((row) =>
    row.listeningPorts.some((port) => port.includes(needle)),
  );
});

async function loadPortUsage() {
  try {
    const data = await invokeToolByChannel("tool:port:usage", {});
    const payload = (data ?? {}) as {
      summary?: { total?: number; tcp?: number; udp?: number };
      stateCounts?: Record<string, number>;
      processSummaries?: Array<{
        pid?: number;
        processName?: string;
        listeningPorts?: string[];
        connectionCount?: number;
      }>;
      connections?: Array<{
        protocol?: string;
        pid?: number;
        processName?: string;
        localAddress?: string;
        remoteAddress?: string;
        state?: string | null;
      }>;
    };
    const summary = payload.summary ?? {};
    const stateCounts = payload.stateCounts ?? {};
    const processSummaries = Array.isArray(payload.processSummaries) ? payload.processSummaries : [];
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
    portConnectionRows.value = connections.slice(0, 1000).map((item) => ({
      protocol: item.protocol ?? "",
      pid: item.pid ?? 0,
      processName: item.processName ?? "UNKNOWN",
      localAddress: item.localAddress ?? "",
      remoteAddress: item.remoteAddress ?? "",
      state: item.state ?? "-",
    }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>
