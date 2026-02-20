<template>
  <div class="panel-grid">
    <div class="panel-grid-full">
      <el-input
        v-model="token"
        type="textarea"
        :rows="4"
        placeholder="粘贴 JWT Token"
      />
    </div>
    <div v-if="decoded" class="panel-grid-full jwt-sections">
      <div class="jwt-section">
        <div class="jwt-section-label" style="color: var(--el-color-primary)">Header</div>
        <pre class="jwt-section-content jwt-header">{{ decoded.header }}</pre>
      </div>
      <div class="jwt-section">
        <div class="jwt-section-label" style="color: var(--el-color-success)">Payload</div>
        <pre class="jwt-section-content jwt-payload">{{ decoded.payload }}</pre>
        <div v-if="decoded.expInfo" style="margin-top: 8px">
          <el-tag :type="decoded.expired ? 'danger' : 'success'" size="small">
            {{ decoded.expired ? "已过期" : "未过期" }}
          </el-tag>
          <span style="margin-left: 8px; font-size: 13px; color: var(--lc-text-secondary)">
            {{ decoded.expInfo }}
          </span>
        </div>
      </div>
      <div class="jwt-section">
        <div class="jwt-section-label" style="color: var(--el-color-danger)">Signature</div>
        <pre class="jwt-section-content jwt-signature">{{ decoded.signature }}</pre>
      </div>
    </div>
    <div v-if="error" class="panel-grid-full">
      <el-alert :title="error" type="error" :closable="false" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";

const token = ref("");
const decoded = ref<{
  header: string;
  payload: string;
  signature: string;
  expInfo?: string;
  expired?: boolean;
} | null>(null);
const error = ref("");

let timer: ReturnType<typeof setTimeout> | null = null;
watch(token, (val) => {
  if (timer) clearTimeout(timer);
  if (!val.trim()) {
    decoded.value = null;
    error.value = "";
    return;
  }
  timer = setTimeout(async () => {
    try {
      const data = (await invokeToolByChannel("tool:jwt:decode", {
        token: val.trim(),
      })) as Record<string, unknown>;
      decoded.value = {
        header: JSON.stringify(data.header, null, 2),
        payload: JSON.stringify(data.payload, null, 2),
        signature: data.signature as string,
        expInfo: data.exp_readable as string | undefined,
        expired: data.expired as boolean | undefined,
      };
      error.value = "";
    } catch (e) {
      decoded.value = null;
      error.value = (e as Error).message;
    }
  }, 200);
});
</script>

<style scoped>
.jwt-sections {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.jwt-section {
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-sm);
  padding: 12px;
  background: var(--lc-surface-1);
}

.jwt-section-label {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
}

.jwt-section-content {
  font-family: var(--lc-font-mono);
  font-size: 13px;
  line-height: 1.5;
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.jwt-header { color: var(--el-color-primary); }
.jwt-payload { color: var(--el-color-success); }
.jwt-signature { color: var(--el-color-danger); }
</style>
