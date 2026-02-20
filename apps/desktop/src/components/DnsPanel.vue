<template>
  <div class="dns-panel">
    <div class="dns-controls">
      <div class="input-row">
        <el-input
          v-model="domain"
          placeholder="输入域名，如 example.com"
          style="flex: 1; min-width: 220px;"
          clearable
          @keyup.enter="runQuery"
        />
        <el-input
          v-model="dnsServer"
          placeholder="DNS 服务器（留空使用系统DNS）"
          style="width: 240px;"
          clearable
        />
        <el-button type="primary" :loading="loading" @click="runQuery">查询</el-button>
      </div>
    </div>

    <template v-if="result">
      <div class="result-meta">
        <el-tag size="small" type="info">DNS: {{ result.server }}</el-tag>
        <el-tag size="small" type="info">耗时: {{ result.elapsed_ms }} ms</el-tag>
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
          </el-table>
        </div>
      </template>

      <div v-if="noRecords" class="empty-hint">
        未查询到任何 DNS 记录
      </div>
    </template>

    <div v-else-if="!loading" class="empty-hint">
      输入域名后点击"查询"，获取 A/AAAA/CNAME/MX/NS/TXT/SOA/SRV 记录
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

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
];

const domain = ref("");
const dnsServer = ref("");
const loading = ref(false);
const result = ref<DnsResult | null>(null);

const noRecords = computed(() => {
  if (!result.value) return false;
  return RECORD_TYPES.every((rt) => getRecords(rt.type).length === 0);
});

function getRecords(type: string): Record<string, unknown>[] {
  if (!result.value) return [];
  return result.value.records[type] ?? [];
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
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}
</script>

<style scoped>
.dns-panel {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.input-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.result-meta {
  display: flex;
  gap: 8px;
}

.record-section {
  margin-bottom: 4px;
}

.empty-hint {
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  text-align: center;
  padding: 32px 0;
}
</style>
