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
      placeholder="按端口/进程名/PID 过滤，例如 5173 或 java 或 1234"
      clearable
    />
    <el-table class="panel-grid-full" :data="filteredPortProcessRows" border max-height="320">
      <el-table-column prop="processName" label="应用" min-width="180" />
      <el-table-column prop="pid" label="PID" width="100" />
      <el-table-column prop="listeningPortsText" label="监听端口" min-width="220" />
      <el-table-column prop="connectionCount" label="连接数" width="110" />
      <el-table-column label="操作" width="180" fixed="right">
        <template #default="{ row }">
          <el-space>
            <el-button size="small" @click="showProcessDetail(row.pid)">详情</el-button>
            <el-button size="small" type="danger" @click="killProcess(row.pid)">结束</el-button>
          </el-space>
        </template>
      </el-table-column>
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

    <el-drawer v-model="detailVisible" title="进程详情" size="50%">
      <el-descriptions v-if="processDetail" :column="1" border>
        <el-descriptions-item label="PID">{{ processDetail.pid }}</el-descriptions-item>
        <el-descriptions-item label="名称">{{ processDetail.name }}</el-descriptions-item>
        <el-descriptions-item label="路径">{{ processDetail.path || "-" }}</el-descriptions-item>
        <el-descriptions-item label="启动命令">{{ processDetail.commandLine || "-" }}</el-descriptions-item>
        <el-descriptions-item label="启动时间">{{ processDetail.startTime || "-" }}</el-descriptions-item>
      </el-descriptions>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
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

const detailVisible = ref(false);
const processDetail = ref<{
  pid: number;
  name: string;
  path: string;
  commandLine: string;
  startTime: string;
} | null>(null);

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

    portConnectionRows.value = connections.slice(0, 1200).map((item) => ({
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

async function showProcessDetail(pid: number) {
  try {
    const data = (await invokeToolByChannel("tool:port:process-detail", {
      pid,
    })) as {
      pid: number;
      name: string;
      path: string;
      commandLine: string;
      startTime: string;
    };
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
