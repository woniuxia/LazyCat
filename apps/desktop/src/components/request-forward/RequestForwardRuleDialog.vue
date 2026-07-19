<script setup lang="ts">
import { computed } from "vue";
import { Delete } from "@element-plus/icons-vue";
import type {
  RequestForwardPreflightResult,
  RequestForwardRuleForm,
} from "../../types/request-forward";
import RequestForwardPreflightResultView from "./RequestForwardPreflightResult.vue";
import RequestForwardRuleFormEditor from "./RequestForwardRuleForm.vue";

const props = defineProps<{
  visible: boolean;
  mode: "create" | "edit" | null;
  form: RequestForwardRuleForm;
  errors: Partial<Record<keyof RequestForwardRuleForm, string>>;
  readonly: boolean;
  persisted: boolean;
  disabled: boolean;
  saving: boolean;
  operating: boolean;
  preflightResult: RequestForwardPreflightResult | null;
  preflighting: boolean;
}>();

const emit = defineEmits<{
  "update:form": [value: RequestForwardRuleForm];
  "request-close": [];
  save: [];
  "save-and-start": [autoStart: boolean];
  preflight: [];
  "preflight-and-start": [autoStart: boolean];
  "apply-suggested-port": [port: number];
  "stop-and-edit": [];
  delete: [];
}>();

const title = computed(() => props.mode === "create" ? "新建转发规则" : "编辑转发规则");

function handleBeforeClose() {
  emit("request-close");
}

function handleSaveCommand(command: "save" | "start-once" | "start-auto") {
  if (command === "save") emit("save");
  else emit("save-and-start", command === "start-auto");
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :title="title"
    width="min(760px, 92vw)"
    class="request-forward-rule-dialog"
    :close-on-click-modal="false"
    :before-close="handleBeforeClose"
  >
    <div v-if="readonly" class="readonly-banner" role="status">
      <div>
        <strong>规则正在运行，配置已锁定</strong>
        <span>停止成功后才会解除只读，避免运行配置与持久化配置不一致。</span>
      </div>
      <el-button
        :disabled="disabled"
        :loading="operating"
        @click="emit('stop-and-edit')"
      >
        停止并编辑
      </el-button>
    </div>

    <div class="dialog-scroll">
      <RequestForwardRuleFormEditor
        :model-value="form"
        :readonly="readonly"
        :disabled="disabled"
        :persisted="persisted"
        :errors="errors"
        @update:model-value="emit('update:form', $event)"
      />
      <RequestForwardPreflightResultView
        v-if="preflightResult"
        :result="preflightResult"
        :disabled="disabled"
        @apply-suggested-port="emit('apply-suggested-port', $event)"
      />
    </div>

    <template #footer>
      <div class="dialog-footer">
        <el-button
          v-if="persisted"
          type="danger"
          text
          :icon="Delete"
          :disabled="readonly || disabled"
          @click="emit('delete')"
        >
          删除规则
        </el-button>
        <span class="dialog-footer__spacer" />
        <el-button :disabled="disabled" @click="emit('request-close')">取消</el-button>
        <el-dropdown
          :disabled="readonly || disabled"
          trigger="click"
          @command="handleSaveCommand"
        >
          <el-button :disabled="readonly || disabled">保存选项</el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="save">仅保存</el-dropdown-item>
              <el-dropdown-item command="start-once">保存并仅本次启动</el-dropdown-item>
              <el-dropdown-item command="start-auto">保存并启动且自动恢复</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button
          :disabled="readonly || disabled"
          :loading="preflighting"
          @click="emit('preflight')"
        >
          检测配置
        </el-button>
        <el-dropdown
          :disabled="readonly || disabled"
          trigger="click"
          @command="(command: 'once' | 'auto') => emit('preflight-and-start', command === 'auto')"
        >
          <el-button
            type="primary"
            :disabled="readonly || disabled"
            :loading="preflighting || saving || operating"
          >
            检测并启动
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="once">检测并仅本次启动</el-dropdown-item>
              <el-dropdown-item command="auto">检测并启动且自动恢复</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.readonly-banner {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 12px;
  padding: 9px 10px;
  border: 1px solid #ecd6a9;
  border-radius: 6px;
  background: #fffaf0;
}
.readonly-banner > div { display: grid; min-width: 0; flex: 1; gap: 3px; }
.readonly-banner strong { color: #65450d; font-size: 16px; }
.readonly-banner span { color: #85672f; font-size: 14px; line-height: 1.5; }
.dialog-scroll { max-height: min(68vh, 720px); overflow-y: auto; padding-right: 2px; }
.dialog-footer { display: flex; align-items: center; gap: 8px; }
.dialog-footer__spacer { flex: 1; }
:global(.request-forward-rule-dialog) { --el-font-size-base: 16px; }
:global(.request-forward-rule-dialog .el-dialog__title) { font-size: 20px; }
:global(.request-forward-rule-dialog .el-button) { font-size: 16px; }

@media (max-width: 560px) {
  .readonly-banner { align-items: flex-start; flex-direction: column; }
  .dialog-footer { flex-wrap: wrap; }
  .dialog-footer__spacer { width: 100%; flex-basis: 100%; }
}
</style>
