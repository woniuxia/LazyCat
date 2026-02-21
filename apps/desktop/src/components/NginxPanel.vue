<template>
  <div class="panel-grid">
    <div class="setting-item">
      <div class="setting-label">服务域名（server_name）</div>
      <el-input v-model="serverName" placeholder="例如 api.example.com" />
      <div class="setting-hint">用于匹配请求域名，支持多个域名（空格分隔）。</div>
    </div>
    <div class="setting-item">
      <div class="setting-label">站点根目录（root）</div>
      <el-input v-model="rootDir" placeholder="例如 /var/www/app/dist" />
      <div class="setting-hint">静态文件目录，通常是前端构建产物目录。</div>
    </div>
    <div class="setting-item">
      <div class="setting-label">API 路径前缀（location）</div>
      <el-input v-model="apiPrefix" placeholder="例如 /api/" />
      <div class="setting-hint">用于反向代理匹配，自动规范为 /xxx/ 格式。</div>
    </div>
    <div class="setting-item">
      <div class="setting-label">API 上游地址（proxy_pass）</div>
      <el-input v-model="apiUpstream" placeholder="例如 http://127.0.0.1:8080" />
      <div class="setting-hint">后端服务地址，建议带协议头（http/https）。</div>
    </div>

    <div class="setting-item">
      <div class="setting-label">请求体大小（client_max_body_size）</div>
      <el-input v-model="clientMaxBodySize" placeholder="例如 20m" />
      <div class="setting-hint">限制上传大小，避免大文件请求被直接拒绝。</div>
    </div>
    <div class="setting-item">
      <div class="setting-label">监听端口（listen）</div>
      <el-input-number v-model="listenPort" :min="1" :max="65535" placeholder="监听端口" />
      <div class="setting-hint">HTTP 常用 80，HTTPS 常用 443。</div>
    </div>
    <div class="setting-item">
      <div class="setting-label">首页文件（index）</div>
      <el-input v-model="indexFile" placeholder="例如 index.html" />
      <div class="setting-hint">站点默认入口文件。</div>
    </div>
    <div class="setting-item">
      <div class="setting-label">SPA 回退</div>
      <el-switch v-model="enableSpaFallback" active-text="开启" inactive-text="关闭" />
      <div class="setting-hint">开启后使用 try_files 回退到 /index.html（前端路由必备）。</div>
    </div>

    <div class="panel-grid-full">
      <el-divider content-position="left">高级选项</el-divider>
    </div>
    <div class="panel-grid-full nginx-switch-row">
      <el-space wrap>
        <el-switch v-model="enableAccessLog" active-text="访问日志" />
        <el-switch v-model="enableGzip" active-text="GZIP 压缩" />
        <el-switch v-model="enableHttps" active-text="HTTPS 开启" />
        <el-switch v-model="enableHttp2" :disabled="!enableHttps" active-text="HTTP/2" />
        <el-switch v-model="enableWebsocket" active-text="WebSocket 透传" />
      </el-space>
    </div>
    <div v-if="enableAccessLog" class="setting-item panel-grid-full">
      <div class="setting-label">日志详细配置</div>
      <div class="log-row">
        <el-switch v-model="generateAccessLog" active-text="Access 日志" />
        <el-select v-model="accessLogFormat" :disabled="!generateAccessLog" style="width: 140px;">
          <el-option label="main" value="main" />
          <el-option label="combined" value="combined" />
          <el-option label="json" value="json" />
        </el-select>
        <el-input
          v-model="accessLogPath"
          :disabled="!generateAccessLog"
          placeholder="例如 /var/log/nginx/access.log"
        />
      </div>
      <div class="log-row">
        <el-switch v-model="generateErrorLog" active-text="Error 日志" />
        <el-select v-model="errorLogLevel" :disabled="!generateErrorLog" style="width: 140px;">
          <el-option label="debug" value="debug" />
          <el-option label="info" value="info" />
          <el-option label="notice" value="notice" />
          <el-option label="warn" value="warn" />
          <el-option label="error" value="error" />
          <el-option label="crit" value="crit" />
        </el-select>
        <el-input
          v-model="errorLogPath"
          :disabled="!generateErrorLog"
          placeholder="例如 /var/log/nginx/error.log"
        />
      </div>
      <div class="setting-hint">
        关闭访问日志时将生成 <code>access_log off;</code>，并不再输出自定义 access/error 日志项。
      </div>
      <div class="setting-hint">
        使用 <code>json</code> 格式时，请在 nginx 的 <code>http {}</code> 块预先定义 <code>log_format json ...;</code>。
      </div>
      <div v-if="generateAccessLog || generateErrorLog" class="log-preview">
        <div class="setting-label">样例日志</div>
        <template v-if="generateAccessLog">
          <div class="log-sample-title">Access 样例</div>
          <pre class="log-sample">{{ accessLogSample }}</pre>
        </template>
        <template v-if="generateErrorLog">
          <div class="log-sample-title">Error 样例</div>
          <pre class="log-sample">{{ errorLogSample }}</pre>
        </template>
      </div>
    </div>
    <div v-if="enableHttps" class="setting-item">
      <div class="setting-label">SSL 证书路径（ssl_certificate）</div>
      <el-input v-model="sslCertPath" placeholder="例如 /etc/nginx/certs/fullchain.pem" />
      <div class="setting-hint">仅在 HTTPS 开启时生效。</div>
    </div>
    <div v-if="enableHttps" class="setting-item">
      <div class="setting-label">SSL 私钥路径（ssl_certificate_key）</div>
      <el-input v-model="sslKeyPath" placeholder="例如 /etc/nginx/certs/privkey.pem" />
      <div class="setting-hint">证书与私钥需配套使用。</div>
    </div>

    <div class="panel-grid-full">
      <el-space wrap>
        <el-button type="primary" :loading="loadingGenerate" @click="generateConfig">生成配置</el-button>
        <el-button :loading="loadingLint" @click="lintConfig">校验配置</el-button>
        <el-button :loading="loadingGenerateAndLint" @click="generateAndLint">一键生成并校验</el-button>
        <el-button :disabled="!configOutput.trim()" @click="copyConfig">复制配置</el-button>
      </el-space>
    </div>

    <el-input
      v-model="configOutput"
      class="panel-grid-full"
      type="textarea"
      :rows="14"
      placeholder="Nginx 配置输出"
    />

    <div v-if="hints.length" class="panel-grid-full">
      <el-alert type="info" show-icon :closable="false">
        <template #title>生成建议</template>
        <div v-for="(hint, idx) in hints" :key="idx" class="nginx-hint-line">
          {{ idx + 1 }}. {{ hint }}
        </div>
      </el-alert>
    </div>

    <div class="panel-grid-full">
      <el-descriptions :column="3" border size="small">
        <el-descriptions-item label="校验状态">
          <el-tag :type="lintValid ? 'success' : 'danger'">{{ lintValid ? "通过" : "未通过" }}</el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="错误数">{{ lintErrorCount }}</el-descriptions-item>
        <el-descriptions-item label="警告数">{{ lintWarnCount }}</el-descriptions-item>
      </el-descriptions>
    </div>

    <el-table v-if="issues.length" class="panel-grid-full" :data="issues" border max-height="260">
      <el-table-column prop="line" label="行号" width="90" />
      <el-table-column prop="level" label="级别" width="90">
        <template #default="{ row }">
          <el-tag size="small" :type="row.level === 'error' ? 'danger' : 'warning'">{{ row.level }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="message" label="问题" min-width="500" show-overflow-tooltip />
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

interface NginxIssue {
  line: number;
  level: "error" | "warn" | string;
  message: string;
}

const serverName = ref("localhost");
const rootDir = ref("/var/www/app/dist");
const apiPrefix = ref("/api/");
const apiUpstream = ref("http://127.0.0.1:8080");
const listenPort = ref(80);
const indexFile = ref("index.html");
const clientMaxBodySize = ref("20m");

const enableSpaFallback = ref(true);
const enableGzip = ref(true);
const enableHttps = ref(false);
const enableHttp2 = ref(false);
const enableWebsocket = ref(false);
const enableAccessLog = ref(true);
const accessLogPath = ref("/var/log/nginx/access.log");
const accessLogFormat = ref("main");
const errorLogPath = ref("/var/log/nginx/error.log");
const errorLogLevel = ref("warn");
const generateAccessLog = ref(true);
const generateErrorLog = ref(true);
const sslCertPath = ref("/etc/nginx/certs/fullchain.pem");
const sslKeyPath = ref("/etc/nginx/certs/privkey.pem");

const configOutput = ref("");
const hints = ref<string[]>([]);
const issues = ref<NginxIssue[]>([]);
const lintValid = ref(true);

const loadingGenerate = ref(false);
const loadingLint = ref(false);
const loadingGenerateAndLint = ref(false);

const lintErrorCount = computed(() => issues.value.filter((item) => item.level === "error").length);
const lintWarnCount = computed(() => issues.value.filter((item) => item.level === "warn").length);
const accessLogSample = computed(() => {
  if (accessLogFormat.value === "json") {
    return '{"time":"2026-02-21T11:03:25+08:00","remote_addr":"10.0.0.8","method":"GET","uri":"/api/health","status":200,"request_time":0.012}';
  }
  if (accessLogFormat.value === "combined") {
    return '10.0.0.8 - - [21/Feb/2026:11:03:25 +0800] "GET /api/health HTTP/1.1" 200 72 "-" "curl/8.5.0"';
  }
  return '10.0.0.8 - - [21/Feb/2026:11:03:25 +0800] "GET /api/health HTTP/1.1" 200 72';
});
const errorLogSample = computed(() => {
  return `2026/02/21 11:04:01 [${errorLogLevel.value}] 12345#0: *87 upstream timed out while reading response header from upstream, client: 10.0.0.8, server: localhost, request: "GET /api/slow HTTP/1.1", upstream: "http://127.0.0.1:8080/api/slow", host: "localhost"`;
});

function ensureApiPrefix(raw: string): string {
  const x = raw.trim();
  if (!x) return "/api/";
  const withLeading = x.startsWith("/") ? x : `/${x}`;
  return withLeading.endsWith("/") ? withLeading : `${withLeading}/`;
}

function buildPayload() {
  return {
    serverName: serverName.value.trim(),
    root: rootDir.value.trim(),
    apiPrefix: ensureApiPrefix(apiPrefix.value),
    apiUpstream: apiUpstream.value.trim(),
    listen: listenPort.value,
    index: indexFile.value.trim() || "index.html",
    clientMaxBodySize: clientMaxBodySize.value.trim() || "20m",
    enableSpaFallback: enableSpaFallback.value,
    enableGzip: enableGzip.value,
    enableHttps: enableHttps.value,
    enableHttp2: enableHttp2.value,
    enableWebsocket: enableWebsocket.value,
    enableAccessLog: enableAccessLog.value,
    accessLogPath: accessLogPath.value.trim(),
    accessLogFormat: accessLogFormat.value,
    errorLogPath: errorLogPath.value.trim(),
    errorLogLevel: errorLogLevel.value,
    generateAccessLog: generateAccessLog.value,
    generateErrorLog: generateErrorLog.value,
    sslCert: sslCertPath.value.trim(),
    sslKey: sslKeyPath.value.trim(),
  };
}

async function generateConfig() {
  loadingGenerate.value = true;
  try {
    const data = (await invokeToolByChannel("tool:nginx:generate", buildPayload())) as {
      config?: string;
      hints?: string[];
    };
    configOutput.value = data?.config ?? "";
    hints.value = Array.isArray(data?.hints) ? data.hints : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    loadingGenerate.value = false;
  }
}

async function lintConfig() {
  loadingLint.value = true;
  try {
    const data = (await invokeToolByChannel("tool:nginx:lint", {
      config: configOutput.value,
    })) as { valid?: boolean; issues?: NginxIssue[] };
    issues.value = Array.isArray(data?.issues) ? data.issues : [];
    lintValid.value = data?.valid ?? issues.value.every((item) => item.level !== "error");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    loadingLint.value = false;
  }
}

async function generateAndLint() {
  loadingGenerateAndLint.value = true;
  try {
    await generateConfig();
    await lintConfig();
    ElMessage.success("生成并校验完成");
  } finally {
    loadingGenerateAndLint.value = false;
  }
}

async function copyConfig() {
  try {
    await navigator.clipboard.writeText(configOutput.value);
    ElMessage.success("配置已复制");
  } catch (error) {
    ElMessage.error(`复制失败: ${(error as Error).message}`);
  }
}
</script>

<style scoped>
.setting-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.setting-label {
  font-size: 13px;
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.setting-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.45;
}

.nginx-switch-row {
  margin-top: -6px;
}

.nginx-hint-line {
  line-height: 1.8;
}

.log-row {
  display: grid;
  grid-template-columns: 170px 140px minmax(240px, 1fr);
  gap: 10px;
  align-items: center;
}

.log-preview {
  margin-top: 8px;
}

.log-sample-title {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 6px;
}

.log-sample {
  margin: 4px 0 0;
  padding: 8px 10px;
  background: var(--el-fill-color-light);
  border-radius: 6px;
  font-size: 12px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
