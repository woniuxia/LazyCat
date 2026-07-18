<script setup lang="ts">
import { computed } from "vue";
import { QuestionFilled } from "@element-plus/icons-vue";
import type { RequestForwardRuleForm } from "../../types/request-forward";
import { isExposedForwardBindHost } from "../../utils/requestForward";

const props = defineProps<{
  modelValue: RequestForwardRuleForm;
  readonly: boolean;
  disabled: boolean;
  persisted: boolean;
  errors?: Partial<Record<keyof RequestForwardRuleForm, string>>;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: RequestForwardRuleForm];
}>();

const exposedListener = computed(() => isExposedForwardBindHost(props.modelValue.bindHost));
const protocolTip = computed(() => props.persisted
  ? "协议在规则创建后不可修改。"
  : "HTTP 会按请求转发；TCP 和 UDP 会按连接或数据报转发。");

function update<K extends keyof RequestForwardRuleForm>(
  key: K,
  value: RequestForwardRuleForm[K],
) {
  emit("update:modelValue", { ...props.modelValue, [key]: value });
}
</script>

<template>
  <el-form class="rule-form" label-position="top" @submit.prevent>
    <section class="form-section">
      <div class="form-section__heading">
        <span>01</span>
        <div>
          <h3>规则标识</h3>
          <p>名称用于在规则列表中快速定位。</p>
        </div>
      </div>
      <div class="form-grid form-grid--identity">
        <el-form-item :error="errors?.name">
          <template #label>
            <span class="field-label">规则名称
              <el-tooltip content="用于在左侧规则列表中快速定位，最多 80 个字符。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="规则名称提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input
            :model-value="modelValue.name"
            :disabled="readonly || disabled"
            maxlength="80"
            show-word-limit
            placeholder="例如：本地 API 转发"
            @update:model-value="update('name', $event)"
          />
        </el-form-item>
        <el-form-item>
          <template #label>
            <span class="field-label">协议
              <el-tooltip :content="protocolTip" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="协议提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-select
            :model-value="modelValue.protocol"
            :disabled="persisted || readonly || disabled"
            @update:model-value="update('protocol', $event)"
          >
            <el-option label="HTTP / HTTPS" value="http" />
            <el-option label="TCP" value="tcp" />
            <el-option label="UDP" value="udp" />
          </el-select>
        </el-form-item>
      </div>
    </section>

    <section class="form-section">
      <div class="form-section__heading">
        <span>02</span>
        <div>
          <h3>本地监听</h3>
          <p>指定 LazyCat 接收流量的本地地址与端口。</p>
        </div>
      </div>
      <div class="form-grid">
        <el-form-item :error="errors?.bindHost">
          <template #label>
            <span class="field-label">监听地址
              <el-tooltip content="LazyCat 接收流量的本地 IP。使用 127.0.0.1 或 ::1 时仅允许本机访问。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="监听地址提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input
            :model-value="modelValue.bindHost"
            :disabled="readonly || disabled"
            placeholder="127.0.0.1 或 ::1"
            @update:model-value="update('bindHost', $event)"
          />
        </el-form-item>
        <el-form-item :error="errors?.listenPort">
          <template #label>
            <span class="field-label">监听端口
              <el-tooltip content="LazyCat 在本机占用并接收流量的端口，范围为 1 到 65535。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="监听端口提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input-number
            :model-value="modelValue.listenPort"
            :disabled="readonly || disabled"
            :min="1"
            :max="65535"
            controls-position="right"
            @update:model-value="update('listenPort', $event ?? 0)"
          />
        </el-form-item>
      </div>
      <div v-if="exposedListener" class="exposure-warning" role="alert">
        <strong>当前监听地址可被其他设备访问</strong>
        <span>请确认所在网络可信，并在系统防火墙中限制不必要的入站访问。</span>
      </div>
    </section>

    <section class="form-section">
      <div class="form-section__heading">
        <span>03</span>
        <div>
          <h3>转发目标</h3>
          <p>目标字段会随协议切换，保存时只提交当前协议需要的字段。</p>
        </div>
      </div>
      <el-form-item
        v-if="modelValue.protocol === 'http'"
        :error="errors?.targetUrl"
      >
        <template #label>
          <span class="field-label">目标 URL
            <el-tooltip content="仅支持 HTTP/HTTPS 基础地址，不包含查询参数或片段。请求路径会追加到该地址。" placement="top">
              <el-icon class="field-tip" tabindex="0" aria-label="目标 URL 提示"><QuestionFilled /></el-icon>
            </el-tooltip>
          </span>
        </template>
        <el-input
          :model-value="modelValue.targetUrl ?? ''"
          :disabled="readonly || disabled"
          placeholder="https://example.com/api"
          @update:model-value="update('targetUrl', $event)"
        />
      </el-form-item>
      <div v-else class="form-grid">
        <el-form-item :error="errors?.targetHost">
          <template #label>
            <span class="field-label">目标主机
              <el-tooltip content="接收转发流量的目标 IP 或域名，不包含端口。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="目标主机提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input
            :model-value="modelValue.targetHost ?? ''"
            :disabled="readonly || disabled"
            placeholder="192.168.1.10 或 db.internal"
            @update:model-value="update('targetHost', $event)"
          />
        </el-form-item>
        <el-form-item :error="errors?.targetPort">
          <template #label>
            <span class="field-label">目标端口
              <el-tooltip content="目标服务实际监听的端口，范围为 1 到 65535。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="目标端口提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input-number
            :model-value="modelValue.targetPort"
            :disabled="readonly || disabled"
            :min="1"
            :max="65535"
            controls-position="right"
            @update:model-value="update('targetPort', $event)"
          />
        </el-form-item>
      </div>
    </section>

    <section v-if="modelValue.protocol === 'http'" class="form-section">
      <div class="form-section__heading">
        <span>04</span>
        <div>
          <h3>HTTP 采集</h3>
          <p>控制后续日志可查看的请求信息范围。</p>
        </div>
      </div>
      <div class="capture-options">
        <el-checkbox
          :model-value="modelValue.captureHttpHeaders"
          :disabled="readonly || disabled"
          @update:model-value="update('captureHttpHeaders', Boolean($event))"
        >
          <span class="capture-option-label">采集请求与响应头
            <el-tooltip content="在日志详情中保留脱敏后的 HTTP 请求头和响应头。" placement="top">
              <el-icon class="field-tip" tabindex="0" aria-label="HTTP 头采集提示" @click.stop><QuestionFilled /></el-icon>
            </el-tooltip>
          </span>
        </el-checkbox>
        <el-checkbox
          :model-value="modelValue.captureHttpBody"
          :disabled="readonly || disabled"
          @update:model-value="update('captureHttpBody', Boolean($event))"
        >
          <span class="capture-option-label">采集请求与响应正文预览
            <el-tooltip content="在日志详情中保留有限长度的正文预览，可能包含业务数据，请按需开启。" placement="top">
              <el-icon class="field-tip" tabindex="0" aria-label="HTTP 正文采集提示" @click.stop><QuestionFilled /></el-icon>
            </el-tooltip>
          </span>
        </el-checkbox>
      </div>
    </section>
  </el-form>
</template>

<style scoped>
.rule-form { display: grid; gap: 10px; }

.form-section {
  padding: 12px 14px 2px;
  border: 1px solid #e1e5ea;
  border-radius: 7px;
  background: #fff;
}

.form-section__heading {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}

.form-section__heading > span {
  color: var(--el-color-primary, #409eff);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
}

.form-section__heading h3 {
  margin: 0;
  color: var(--text-primary, #1f2937);
  font-size: 18px;
}

.form-section__heading p {
  margin: 2px 0 0;
  color: var(--text-secondary, #64748b);
  font-size: 14px;
  line-height: 1.4;
}

.form-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 180px;
  gap: 12px;
}

.form-grid--identity { grid-template-columns: minmax(0, 1fr) 220px; }
.rule-form :deep(.el-form-item__label),
.rule-form :deep(.el-input__inner),
.rule-form :deep(.el-select__placeholder),
.rule-form :deep(.el-input-number .el-input__inner),
.rule-form :deep(.el-checkbox__label) { font-size: 16px; }
.rule-form :deep(.el-select),
.rule-form :deep(.el-input-number) { width: 100%; }
.field-label,
.capture-option-label { display: inline-flex; align-items: center; gap: 5px; }
.field-tip { color: #657386; cursor: help; font-size: 16px; }
.field-tip:hover { color: var(--el-color-primary, #409eff); }
.field-tip:focus-visible { border-radius: 50%; outline: 2px solid var(--el-color-primary, #409eff); outline-offset: 1px; }

.exposure-warning {
  display: grid;
  gap: 3px;
  margin: -1px 0 12px;
  padding: 8px 10px;
  border-left: 3px solid #d58a16;
  background: #fff8e8;
  color: #70490b;
  font-size: 14px;
  line-height: 1.45;
}

.capture-options {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 20px;
  margin-bottom: 10px;
}

@media (max-width: 680px) {
  .form-grid,
  .form-grid--identity { grid-template-columns: minmax(0, 1fr); gap: 0; }
  .form-section { padding-inline: 12px; }
}
</style>
