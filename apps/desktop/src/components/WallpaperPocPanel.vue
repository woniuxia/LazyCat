<template>
  <!-- B1 Spike: Wallpaper PoC 控制台。仅 dev 阶段，验证 WebView2 CapturePreview 链路 -->
  <div class="poc-panel">
    <h2>Wallpaper PoC · WebView2 CapturePreview</h2>
    <p class="hint">
      验证 hidden Tauri WebView 能否通过 ICoreWebView2::CapturePreview 输出非黑帧。
      关联文档：<code>docs/superpowers/spikes/2026-05-05-wallpaper-screenshot.md</code>
    </p>

    <div class="row">
      <el-button type="primary" :loading="busy" @click="onOpen(false)">
        打开 hidden 窗口
      </el-button>
      <el-button :loading="busy" @click="onOpen(true)">
        打开 visible 窗口（对照）
      </el-button>
      <el-button :loading="busy" @click="onShow(true)">显示</el-button>
      <el-button :loading="busy" @click="onShow(false)">隐藏</el-button>
      <el-button :loading="busy" type="danger" plain @click="onClose">
        关闭窗口
      </el-button>
    </div>

    <div class="row">
      <el-button :loading="busy" @click="captureOnce">截图 1 次</el-button>
      <el-button :loading="busy" @click="captureN(10)">截图 10 次循环</el-button>
      <el-button :loading="busy" plain @click="results = []">清空结果</el-button>
    </div>

    <div class="status">
      <strong>状态：</strong>{{ status }}
    </div>

    <table v-if="results.length > 0" class="results">
      <thead>
        <tr>
          <th>#</th>
          <th>耗时 (ms)</th>
          <th>大小 (KB)</th>
          <th>OK</th>
          <th>路径 / 错误</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(r, i) in results" :key="i" :class="{ fail: !r.ok }">
          <td>{{ i + 1 }}</td>
          <td>{{ r.elapsed_ms }}</td>
          <td>{{ (r.bytes / 1024).toFixed(1) }}</td>
          <td>{{ r.ok ? "✓" : "✗" }}</td>
          <td>{{ r.error ?? r.path ?? "-" }}</td>
        </tr>
      </tbody>
    </table>

    <div v-if="stats" class="stats">
      <div><strong>首次：</strong>{{ stats.first }} ms</div>
      <div><strong>后续平均（去掉首次）：</strong>{{ stats.avgRest }} ms</div>
      <div><strong>最快 / 最慢：</strong>{{ stats.min }} / {{ stats.max }} ms</div>
      <div><strong>成功率：</strong>{{ stats.successRate }}</div>
    </div>

    <div v-if="lastImagePath" class="preview">
      <h3>最后一张截图预览（路径）：</h3>
      <code>{{ lastImagePath }}</code>
      <p class="hint">用资源管理器打开该路径验证 P1（非黑帧）和 P4（DPI 缩放下尺寸）</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ElButton, ElMessage } from "element-plus";

interface CaptureResult {
  ok: boolean;
  elapsed_ms: number;
  bytes: number;
  path?: string | null;
  error?: string | null;
}

const busy = ref(false);
const status = ref("未打开");
const results = ref<CaptureResult[]>([]);
const lastImagePath = ref<string>("");

const stats = computed(() => {
  if (results.value.length === 0) return null;
  const ok = results.value.filter((r) => r.ok);
  if (ok.length === 0) return null;
  const ms = ok.map((r) => r.elapsed_ms);
  const first = ms[0];
  const rest = ms.slice(1);
  const avgRest =
    rest.length > 0
      ? Math.round(rest.reduce((a, b) => a + b, 0) / rest.length)
      : first;
  return {
    first,
    avgRest,
    min: Math.min(...ms),
    max: Math.max(...ms),
    successRate: `${ok.length}/${results.value.length}`,
  };
});

async function onOpen(visible: boolean) {
  busy.value = true;
  try {
    const msg = await invoke<string>("wallpaper_poc_open", { visible });
    status.value = msg;
  } catch (e) {
    ElMessage.error(String(e));
    status.value = `打开失败：${e}`;
  } finally {
    busy.value = false;
  }
}

async function onShow(visible: boolean) {
  busy.value = true;
  try {
    await invoke("wallpaper_poc_show", { visible });
    status.value = visible ? "已显示" : "已隐藏";
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    busy.value = false;
  }
}

async function onClose() {
  busy.value = true;
  try {
    await invoke("wallpaper_poc_close");
    status.value = "已关闭";
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    busy.value = false;
  }
}

function nextOutputPath(idx: number) {
  // PoC 输出目录：固定写到 Public Documents，方便资源管理器快速验证
  const ts = Date.now();
  return `C:\\Users\\Public\\Documents\\wallpaper-poc-${ts}-${idx}.png`;
}

async function captureOnce() {
  busy.value = true;
  try {
    const out = nextOutputPath(results.value.length);
    const r = await invoke<CaptureResult>("wallpaper_poc_capture", {
      outputPath: out,
    });
    results.value.push(r);
    if (r.ok && r.path) lastImagePath.value = r.path;
    if (!r.ok) ElMessage.error(`截图失败：${r.error}`);
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    busy.value = false;
  }
}

async function captureN(n: number) {
  busy.value = true;
  try {
    for (let i = 0; i < n; i++) {
      const out = nextOutputPath(results.value.length);
      const r = await invoke<CaptureResult>("wallpaper_poc_capture", {
        outputPath: out,
      });
      results.value.push(r);
      if (r.ok && r.path) lastImagePath.value = r.path;
      if (!r.ok) {
        ElMessage.error(`第 ${i + 1} 次失败：${r.error}`);
        break;
      }
    }
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    busy.value = false;
  }
}
</script>

<style scoped>
.poc-panel {
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-width: 900px;
}

.hint {
  font-size: 12px;
  color: var(--el-text-color-secondary, #666);
  margin: 0;
}

.row {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.status {
  padding: 8px 12px;
  background: var(--el-bg-color-page, #f5f5f5);
  border-radius: 4px;
  font-size: 13px;
}

.results {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.results th,
.results td {
  border: 1px solid var(--el-border-color-lighter, #e4e7ed);
  padding: 6px 10px;
  text-align: left;
}

.results th {
  background: var(--el-bg-color-page, #f5f5f5);
}

.results tr.fail {
  background: rgba(239, 68, 68, 0.08);
}

.stats {
  padding: 12px;
  background: var(--el-bg-color-page, #f5f5f5);
  border-radius: 6px;
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
  font-size: 13px;
}

.preview {
  padding: 12px;
  background: var(--el-bg-color-page, #f5f5f5);
  border-radius: 6px;
  font-size: 13px;
}

.preview code {
  display: block;
  margin: 6px 0;
  word-break: break-all;
}
</style>
