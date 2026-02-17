<template>
  <div class="panel-grid">
    <div class="panel-grid-full">
      <el-button type="primary" @click="emit('load')">查询端口占用</el-button>
    </div>
    <el-divider class="panel-grid-full" content-position="left">概览</el-divider>
    <el-descriptions class="panel-grid-full" :column="3" border>
      <el-descriptions-item label="总连接">{{ summary.total }}</el-descriptions-item>
      <el-descriptions-item label="TCP">{{ summary.tcp }}</el-descriptions-item>
      <el-descriptions-item label="UDP">{{ summary.udp }}</el-descriptions-item>
    </el-descriptions>
    <el-table class="panel-grid-full" :data="stateRows" border>
      <el-table-column prop="state" label="状态" />
      <el-table-column prop="count" label="数量" width="120" />
    </el-table>
    <el-divider class="panel-grid-full" content-position="left">按应用汇总</el-divider>
    <el-input
      class="panel-grid-full"
      :model-value="filter"
      placeholder="按端口筛选应用汇总，例如 5173"
      clearable
      @update:model-value="emit('update:filter', String($event ?? ''))"
    />
    <el-table class="panel-grid-full" :data="filteredProcessRows" border max-height="280">
      <el-table-column prop="processName" label="应用" min-width="180" />
      <el-table-column prop="pid" label="PID" width="100" />
      <el-table-column prop="listeningPortsText" label="监听端口" min-width="220" />
      <el-table-column prop="connectionCount" label="连接数" width="120" />
    </el-table>
    <el-divider class="panel-grid-full" content-position="left">连接明细</el-divider>
    <el-table class="panel-grid-full" :data="connectionRows" border max-height="360">
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
interface PortUsageSummary {
  total: number;
  tcp: number;
  udp: number;
}

interface PortUsageStateRow {
  state: string;
  count: number;
}

interface PortUsageProcessRow {
  pid: number;
  processName: string;
  listeningPorts: string[];
  listeningPortsText: string;
  connectionCount: number;
}

interface PortUsageConnectionRow {
  protocol: string;
  pid: number;
  processName: string;
  localAddress: string;
  remoteAddress: string;
  state: string;
}

defineProps<{
  summary: PortUsageSummary;
  stateRows: PortUsageStateRow[];
  filter: string;
  filteredProcessRows: PortUsageProcessRow[];
  connectionRows: PortUsageConnectionRow[];
}>();

const emit = defineEmits<{
  (event: "load"): void;
  (event: "update:filter", value: string): void;
}>();
</script>
