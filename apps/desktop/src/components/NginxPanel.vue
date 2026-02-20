<template>
  <div class="panel-grid">
    <el-input v-model="serverName" placeholder="server_name，例如 api.example.com" />
    <el-input v-model="rootDir" placeholder="静态资源目录，例如 /var/www/app/dist" />
    <el-input v-model="apiPrefix" placeholder="API 前缀，例如 /api/" />
    <el-input v-model="apiUpstream" placeholder="API upstream，例如 http://127.0.0.1:8080" />
    <div class="panel-grid-full">
      <el-space>
        <el-switch v-model="enableSpaFallback" active-text="SPA 回退" />
        <el-switch v-model="enableGzip" active-text="Gzip" />
      </el-space>
    </div>
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="generateConfig">生成配置</el-button>
        <el-button @click="lintConfig">校验配置</el-button>
      </el-space>
    </div>
    <el-input
      v-model="configOutput"
      class="panel-grid-full"
      type="textarea"
      :rows="12"
      placeholder="Nginx 配置输出"
    />
    <el-table
      v-if="issues.length"
      class="panel-grid-full"
      :data="issues"
      border
      max-height="220"
    >
      <el-table-column prop="line" label="行号" width="90" />
      <el-table-column prop="level" label="级别" width="90" />
      <el-table-column prop="message" label="问题" min-width="420" />
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const serverName = ref("localhost");
const rootDir = ref("/var/www/app/dist");
const apiPrefix = ref("/api/");
const apiUpstream = ref("http://127.0.0.1:8080");
const enableSpaFallback = ref(true);
const enableGzip = ref(true);
const configOutput = ref("");
const issues = ref<Array<{ line: number; level: string; message: string }>>([]);

async function generateConfig() {
  try {
    const data = (await invokeToolByChannel("tool:nginx:generate", {
      serverName: serverName.value,
      root: rootDir.value,
      apiPrefix: apiPrefix.value,
      apiUpstream: apiUpstream.value,
      listen: 80,
      index: "index.html",
      enableSpaFallback: enableSpaFallback.value,
      enableGzip: enableGzip.value,
    })) as { config?: string };
    configOutput.value = data?.config ?? "";
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function lintConfig() {
  try {
    const data = (await invokeToolByChannel("tool:nginx:lint", {
      config: configOutput.value,
    })) as { issues?: Array<{ line: number; level: string; message: string }> };
    issues.value = Array.isArray(data?.issues) ? data.issues : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>
