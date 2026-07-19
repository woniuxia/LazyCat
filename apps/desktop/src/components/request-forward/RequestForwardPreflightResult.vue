<script setup lang="ts">
import type {
  RequestForwardPreflightCheckKind,
  RequestForwardPreflightResult,
} from "../../types/request-forward";

defineProps<{
  result: RequestForwardPreflightResult;
  disabled: boolean;
}>();

const emit = defineEmits<{
  "apply-suggested-port": [port: number];
}>();

const checkLabels: Record<RequestForwardPreflightCheckKind, string> = {
  listener: "监听端口",
  dns: "目标解析",
  connect: "目标连接",
  tls: "TLS 校验",
};

const stateLabels = {
  passed: "通过",
  failed: "失败",
  warning: "注意",
} as const;
</script>

<template>
  <section
    class="preflight-result"
    :class="result.ready ? 'is-ready' : 'is-blocked'"
    role="status"
    aria-live="polite"
    aria-label="配置预检结果"
  >
    <header class="preflight-result__summary">
      <div>
        <span class="preflight-result__eyebrow">配置预检</span>
        <strong>{{ result.ready ? "检测时配置可用" : "检测发现阻断项" }}</strong>
      </div>
      <span class="preflight-result__verdict">
        {{ result.ready ? "可继续" : "请先处理" }}
      </span>
    </header>

    <ul class="preflight-result__checks">
      <li
        v-for="check in result.checks"
        :key="check.kind"
        class="preflight-check"
        :class="`is-${check.state}`"
      >
        <span class="preflight-check__mark" aria-hidden="true" />
        <div>
          <span class="preflight-check__heading">
            <strong>{{ checkLabels[check.kind] }}</strong>
            <em>{{ stateLabels[check.state] }}</em>
          </span>
          <p v-if="check.state === 'failed'" role="alert">{{ check.message }}</p>
          <p v-else>{{ check.message }}</p>
        </div>
      </li>
    </ul>

    <div
      v-if="result.suggestedListenPort != null"
      class="preflight-result__suggestion"
      role="status"
    >
      <span>检测到可尝试的监听端口 {{ result.suggestedListenPort }}</span>
      <el-button
        size="small"
        :disabled="disabled"
        @click="emit('apply-suggested-port', result.suggestedListenPort)"
      >
        使用建议端口 {{ result.suggestedListenPort }}
      </el-button>
    </div>
  </section>
</template>

<style scoped>
.preflight-result {
  margin-top: 14px;
  border: 1px solid #d9e0e7;
  border-radius: 7px;
  background: #fbfcfd;
  overflow: hidden;
}

.preflight-result__summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 11px 13px;
  border-bottom: 1px solid #e3e8ed;
  background: #f4f7f9;
}

.preflight-result__summary > div { display: grid; gap: 2px; }
.preflight-result__eyebrow {
  color: #667486;
  font-size: 11px;
  font-weight: 800;
  letter-spacing: .12em;
}
.preflight-result__summary strong { color: #26364a; font-size: 16px; }
.preflight-result__verdict {
  flex: none;
  border-radius: 999px;
  padding: 3px 9px;
  background: #e6f4ee;
  color: #176b4a;
  font-size: 12px;
  font-weight: 700;
}
.is-blocked .preflight-result__verdict { background: #fbe8e6; color: #9f302b; }

.preflight-result__checks { display: grid; margin: 0; padding: 0; list-style: none; }
.preflight-check {
  display: grid;
  grid-template-columns: 10px minmax(0, 1fr);
  gap: 10px;
  padding: 10px 13px;
  border-bottom: 1px solid #edf0f3;
}
.preflight-check:last-child { border-bottom: 0; }
.preflight-check__mark {
  width: 8px;
  height: 8px;
  margin-top: 5px;
  border-radius: 50%;
  background: #2b9368;
  box-shadow: 0 0 0 3px #e3f3ec;
}
.preflight-check.is-warning .preflight-check__mark {
  background: #b87916;
  box-shadow: 0 0 0 3px #fff1d9;
}
.preflight-check.is-failed .preflight-check__mark {
  background: #c5453e;
  box-shadow: 0 0 0 3px #fbe6e4;
}
.preflight-check__heading { display: flex; align-items: baseline; gap: 8px; }
.preflight-check__heading strong { color: #314156; font-size: 14px; }
.preflight-check__heading em {
  color: #2b7d5e;
  font-size: 12px;
  font-style: normal;
  font-weight: 700;
}
.is-warning .preflight-check__heading em { color: #9a650e; }
.is-failed .preflight-check__heading em { color: #a73731; }
.preflight-check p {
  margin: 3px 0 0;
  color: #647184;
  font-size: 13px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.preflight-result__suggestion {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 9px 13px;
  border-top: 1px solid #ecd6a9;
  background: #fffaf0;
  color: #78591e;
  font-size: 13px;
}

@media (max-width: 560px) {
  .preflight-result__summary,
  .preflight-result__suggestion { align-items: flex-start; flex-direction: column; }
}
</style>
