<template>
  <div class="network-panel">
    <div class="network-controls">
      <el-radio-group v-model="protocol" class="protocol-group">
        <el-radio-button value="tcp">TCP</el-radio-button>
        <el-radio-button value="http">HTTP</el-radio-button>
        <el-radio-button value="https">HTTPS</el-radio-button>
      </el-radio-group>

      <div v-if="protocol === 'tcp'" class="input-row">
        <el-input v-model="host" placeholder="主机地址" style="width: 220px;" />
        <el-input-number v-model="port" :min="1" :max="65535" placeholder="端口" style="width: 120px;" />
        <el-input-number v-model="timeoutMs" :min="100" :max="30000" :step="500" placeholder="超时(ms)" style="width: 140px;" />
        <el-button type="primary" :loading="loading" @click="runTest">测试连通性</el-button>
      </div>

      <div v-else class="input-row">
        <el-input v-model="httpUrl" :placeholder="protocol === 'https' ? 'https://example.com' : 'http://example.com'" style="flex: 1; min-width: 280px;" />
        <el-input-number v-model="timeoutMs" :min="100" :max="30000" :step="500" placeholder="超时(ms)" style="width: 140px;" />
        <el-button type="primary" :loading="loading" @click="runTest">测试连通性</el-button>
      </div>
    </div>

    <div v-if="result" class="result-card" :class="result.reachable ? 'result-ok' : 'result-fail'">
      <div class="result-header">
        <span class="result-icon">{{ result.reachable ? '✓' : '✗' }}</span>
        <span class="result-status">{{ result.reachable ? '可达' : '不可达' }}</span>
        <el-tag size="small" :type="result.reachable ? 'success' : 'danger'" style="margin-left: 8px;">
          {{ result.latencyMs }} ms
        </el-tag>
        <el-tag v-if="result.statusCode != null" size="small" :type="statusTagType(result.statusCode)" style="margin-left: 6px;">
          HTTP {{ result.statusCode }}
        </el-tag>
      </div>

      <el-descriptions :column="1" size="small" border style="margin-top: 12px;">
        <el-descriptions-item v-if="protocol === 'tcp'" label="主机">{{ result.host }}</el-descriptions-item>
        <el-descriptions-item v-if="protocol === 'tcp'" label="端口">{{ result.port }}</el-descriptions-item>
        <el-descriptions-item v-if="protocol !== 'tcp'" label="URL">{{ result.url }}</el-descriptions-item>
        <el-descriptions-item label="延迟">{{ result.latencyMs }} ms</el-descriptions-item>
        <el-descriptions-item v-if="result.statusCode != null" label="状态码">{{ result.statusCode }}</el-descriptions-item>
        <el-descriptions-item v-if="result.error" label="错误">
          <span style="color: var(--el-color-danger);">{{ result.error }}</span>
        </el-descriptions-item>
      </el-descriptions>
    </div>

    <div v-else class="empty-hint">
      选择协议并输入地址，点击"测试连通性"查看结果
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";

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

const protocol = ref<Protocol>("tcp");
const host = ref("127.0.0.1");
const port = ref(80);
const timeoutMs = ref(2000);
const httpUrl = ref("");
const loading = ref(false);
const result = ref<TestResult | null>(null);

watch(protocol, () => {
  result.value = null;
  if (protocol.value === "http" && !httpUrl.value) {
    httpUrl.value = "http://";
  } else if (protocol.value === "https" && !httpUrl.value.startsWith("https")) {
    httpUrl.value = "https://";
  }
});

async function runTest() {
  result.value = null;
  loading.value = true;
  try {
    if (protocol.value === "tcp") {
      const data = await invokeToolByChannel("tool:network:tcp-test", {
        host: host.value,
        port: port.value,
        timeoutMs: timeoutMs.value
      });
      result.value = data as TestResult;
    } else {
      const url = httpUrl.value.trim();
      const data = await invokeToolByChannel("tool:network:http-test", {
        url,
        timeoutMs: timeoutMs.value
      });
      result.value = data as TestResult;
    }
  } catch (e) {
    result.value = {
      reachable: false,
      latencyMs: 0,
      error: (e as Error).message
    };
  } finally {
    loading.value = false;
  }
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
  gap: 16px;
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

.result-card {
  border-radius: 6px;
  padding: 16px;
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
  gap: 4px;
  font-size: 16px;
  font-weight: 600;
}

.result-icon {
  font-size: 18px;
}

.result-ok .result-icon,
.result-ok .result-status {
  color: var(--el-color-success);
}

.result-fail .result-icon,
.result-fail .result-status {
  color: var(--el-color-danger);
}

.empty-hint {
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  text-align: center;
  padding: 32px 0;
}
</style>
