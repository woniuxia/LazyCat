<template>
  <div>
    <!-- Base64 -->
    <div v-if="activeTool === 'base64'" class="panel-grid">
      <div class="panel-grid-full">
        <el-radio-group v-model="base64UrlSafe" size="small">
          <el-radio-button :value="false">Standard</el-radio-button>
          <el-radio-button :value="true">URL-safe</el-radio-button>
        </el-radio-group>
      </div>
      <div class="textarea-wrap">
        <el-input v-model="base64Input" type="textarea" :rows="10" placeholder="输入文本" />
        <span class="char-count">{{ base64Input.length }} 字符</span>
      </div>
      <div class="textarea-wrap">
        <el-input :model-value="base64Output" type="textarea" :rows="10" readonly placeholder="结果" />
        <span class="char-count">{{ base64Output.length }} 字符</span>
      </div>
      <div class="panel-grid-full">
        <el-space>
          <el-button type="primary" @click="runBase64('encode')">Base64 编码</el-button>
          <el-button @click="runBase64('decode')">Base64 解码</el-button>
          <el-button @click="swapBase64">互换</el-button>
          <el-button @click="copyOutput(base64Output)">复制结果</el-button>
          <el-button @click="clearBase64">清空</el-button>
        </el-space>
      </div>
    </div>

    <!-- URL -->
    <div v-else-if="activeTool === 'url'" class="panel-grid">
      <div class="textarea-wrap">
        <el-input v-model="urlInput" type="textarea" :rows="10" placeholder="输入 URL 文本" />
        <span class="char-count">{{ urlInput.length }} 字符</span>
      </div>
      <div class="textarea-wrap">
        <el-input :model-value="urlOutput" type="textarea" :rows="10" readonly placeholder="结果" />
        <span class="char-count">{{ urlOutput.length }} 字符</span>
      </div>
      <div class="panel-grid-full">
        <el-space>
          <el-button type="primary" @click="runUrl('encode')">URL 编码</el-button>
          <el-button @click="runUrl('decode')">URL 解码</el-button>
          <el-button @click="swapUrl">互换</el-button>
          <el-button @click="copyOutput(urlOutput)">复制结果</el-button>
          <el-button @click="clearUrl">清空</el-button>
        </el-space>
      </div>
    </div>

    <!-- MD5 -->
    <div v-else-if="activeTool === 'md5'" class="panel-grid">
      <div class="textarea-wrap">
        <el-input v-model="md5Input" type="textarea" :rows="10" placeholder="输入文本" />
        <span class="char-count">{{ md5Input.length }} 字符</span>
      </div>
      <div class="textarea-wrap">
        <el-input :model-value="md5Output" type="textarea" :rows="10" readonly placeholder="MD5 结果" />
        <span class="char-count">{{ md5Output.length }} 字符</span>
      </div>
      <div class="panel-grid-full">
        <el-space>
          <el-button type="primary" @click="runMd5">计算 MD5</el-button>
          <el-button @click="copyOutput(md5Output)">复制结果</el-button>
          <el-button @click="clearMd5">清空</el-button>
        </el-space>
      </div>
    </div>

    <!-- QR -->
    <div v-else-if="activeTool === 'qr'" class="qr-layout">
      <div class="textarea-wrap qr-layout-full">
        <el-input v-model="qrInput" type="textarea" :rows="7" placeholder="输入文本并生成二维码" />
        <span class="char-count">{{ qrInput.length }} 字符</span>
      </div>
      <el-input
        class="qr-layout-full"
        :model-value="qrDataUrl"
        type="textarea"
        :rows="4"
        readonly
        placeholder="Base64 图片（Data URL）"
      />
      <div class="qr-layout-action">
        <el-space>
          <el-button type="primary" @click="generateQr">生成二维码</el-button>
          <el-button @click="copyOutput(qrDataUrl)">复制 Data URL</el-button>
          <el-button @click="clearQr">清空</el-button>
        </el-space>
      </div>
      <div class="qr-preview">
        <img v-if="qrDataUrl" :src="qrDataUrl" alt="QR code" class="qr-image" />
        <el-empty v-else description="尚未生成二维码" />
      </div>
    </div>

    <!-- Hash (SHA/HMAC) -->
    <div v-else-if="activeTool === 'hash'" class="panel-grid">
      <div class="panel-grid-full">
        <el-radio-group v-model="hashAlgo" size="small">
          <el-radio-button value="sha1">SHA-1</el-radio-button>
          <el-radio-button value="sha256">SHA-256</el-radio-button>
          <el-radio-button value="sha512">SHA-512</el-radio-button>
          <el-radio-button value="hmac-sha256">HMAC-SHA256</el-radio-button>
        </el-radio-group>
      </div>
      <div class="textarea-wrap">
        <el-input v-model="hashInput" type="textarea" :rows="8" placeholder="输入文本" />
        <span class="char-count">{{ hashInput.length }} 字符</span>
      </div>
      <div v-if="hashAlgo === 'hmac-sha256'" class="panel-grid-full">
        <el-input v-model="hmacKey" placeholder="HMAC 密钥" />
      </div>
      <div class="textarea-wrap">
        <el-input :model-value="hashOutput" type="textarea" :rows="4" readonly placeholder="散列结果" />
        <span class="char-count">{{ hashOutput.length }} 字符</span>
      </div>
      <div class="panel-grid-full">
        <el-space>
          <el-button type="primary" @click="runHash">计算散列</el-button>
          <el-button @click="copyOutput(hashOutput)">复制结果</el-button>
          <el-button @click="clearHash">清空</el-button>
        </el-space>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

defineProps<{ activeTool: string }>();

const base64Input = ref("");
const base64Output = ref("");
const base64UrlSafe = ref(false);

const urlInput = ref("");
const urlOutput = ref("");

const md5Input = ref("");
const md5Output = ref("");

const qrInput = ref("");
const qrDataUrl = ref("");

const hashAlgo = ref<"sha1" | "sha256" | "sha512" | "hmac-sha256">("sha256");
const hashInput = ref("");
const hashOutput = ref("");
const hmacKey = ref("");

async function call(channel: string, payload: Record<string, unknown>): Promise<string> {
  const data = await invokeToolByChannel(channel, payload);
  return typeof data === "string" ? data : JSON.stringify(data, null, 2);
}

async function runBase64(mode: "encode" | "decode") {
  try {
    const channel = mode === "encode"
      ? (base64UrlSafe.value ? "tool:encode:base64-url-encode" : "tool:encode:base64-encode")
      : (base64UrlSafe.value ? "tool:encode:base64-url-decode" : "tool:encode:base64-decode");
    base64Output.value = await call(channel, { input: base64Input.value });
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function swapBase64() {
  base64Input.value = base64Output.value;
  base64Output.value = "";
}

function clearBase64() {
  base64Input.value = "";
  base64Output.value = "";
}

async function runUrl(mode: "encode" | "decode") {
  try {
    const channel = mode === "encode" ? "tool:encode:url-encode" : "tool:encode:url-decode";
    urlOutput.value = await call(channel, { input: urlInput.value });
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function swapUrl() {
  urlInput.value = urlOutput.value;
  urlOutput.value = "";
}

function clearUrl() {
  urlInput.value = "";
  urlOutput.value = "";
}

async function runMd5() {
  try {
    md5Output.value = await call("tool:encode:md5", { input: md5Input.value });
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function clearMd5() {
  md5Input.value = "";
  md5Output.value = "";
}

async function generateQr() {
  try {
    qrDataUrl.value = await call("tool:encode:qr", { input: qrInput.value });
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function clearQr() {
  qrInput.value = "";
  qrDataUrl.value = "";
}

async function runHash() {
  try {
    const channelMap: Record<string, string> = {
      sha1: "tool:encode:sha1",
      sha256: "tool:encode:sha256",
      sha512: "tool:encode:sha512",
      "hmac-sha256": "tool:encode:hmac-sha256",
    };
    const payload: Record<string, unknown> = { input: hashInput.value };
    if (hashAlgo.value === "hmac-sha256") {
      payload.key = hmacKey.value;
    }
    hashOutput.value = await call(channelMap[hashAlgo.value], payload);
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function clearHash() {
  hashInput.value = "";
  hashOutput.value = "";
  hmacKey.value = "";
}

async function copyOutput(text: string) {
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}
</script>

<style scoped>
.textarea-wrap {
  position: relative;
}
.char-count {
  position: absolute;
  bottom: 6px;
  right: 10px;
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  pointer-events: none;
}
</style>
