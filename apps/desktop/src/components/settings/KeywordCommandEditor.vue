<template>
  <el-dialog
    v-model="visible"
    :title="isEdit ? '编辑关键字命令' : '添加关键字命令'"
    width="540px"
    :close-on-click-modal="false"
    append-to-body
    @close="onCancel"
  >
    <el-form label-position="top" class="kw-editor-form">
      <el-form-item label="关键字(不含 ; 前缀)" required>
        <el-input
          :model-value="form.keyword"
          placeholder="例如:myapi"
          maxlength="24"
          @update:model-value="(v: string) => (form.keyword = v)"
          @blur="validateKeyword"
        />
        <div v-if="keywordError" class="kw-editor-error">{{ keywordError }}</div>
        <div v-else class="kw-editor-hint">触发输入:; {{ form.keyword || "<keyword>" }}</div>
      </el-form-item>

      <el-form-item label="显示名" required>
        <el-input
          :model-value="form.name"
          placeholder="例如:跳到 API 片段"
          maxlength="40"
          @update:model-value="(v: string) => (form.name = v)"
        />
      </el-form-item>

      <el-form-item label="描述">
        <el-input
          :model-value="form.description"
          placeholder="可选,用于设置面板与命令列表展示"
          maxlength="80"
          @update:model-value="(v: string) => (form.description = v)"
        />
      </el-form-item>

      <el-form-item label="类型" required>
        <el-radio-group
          :model-value="form.kind"
          @update:model-value="(v: KeywordCommandCustom['kind']) => onChangeKind(v)"
        >
          <el-radio-button value="open-tool">直达工具</el-radio-button>
          <el-radio-button value="vault-tag">查 Vault Tag</el-radio-button>
          <el-radio-button value="snippet-tag">查 Snippet Tag</el-radio-button>
        </el-radio-group>
      </el-form-item>

      <el-form-item v-if="form.kind === 'open-tool'" label="目标工具" required>
        <el-select
          :model-value="form.toolId"
          placeholder="选择目标工具"
          filterable
          style="width: 100%"
          @update:model-value="(v: string) => (form.toolId = v)"
        >
          <el-option
            v-for="t in toolOptions"
            :key="t.id"
            :label="`${t.name} · ${t.desc}`"
            :value="t.id"
          />
        </el-select>
      </el-form-item>

      <el-form-item v-if="form.kind === 'open-tool'">
        <el-switch
          :model-value="form.forwardArgs"
          @update:model-value="(v: boolean) => (form.forwardArgs = v)"
        />
        <span class="kw-editor-inline">透传参数(把 ; 后面的文本预填到工具输入框)</span>
      </el-form-item>

      <el-form-item
        v-if="form.kind === 'vault-tag' || form.kind === 'snippet-tag'"
        label="目标 Tag"
        required
      >
        <el-input
          :model-value="form.targetTag"
          :placeholder="form.kind === 'vault-tag' ? '例如:wifi' : '例如:api'"
          maxlength="40"
          @update:model-value="(v: string) => (form.targetTag = v)"
        />
        <div class="kw-editor-hint">
          将列出含此 Tag 的{{ form.kind === "vault-tag" ? "凭据" : "代码片段" }}条目
        </div>
      </el-form-item>

      <el-form-item>
        <el-switch
          :model-value="form.enabled"
          @update:model-value="(v: boolean) => (form.enabled = v)"
        />
        <span class="kw-editor-inline">启用此关键字</span>
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="onCancel">取消</el-button>
      <el-button type="primary" :disabled="!canSave" @click="onSave">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { validateCustomKeyword, generateCustomKeywordId } from "../../spotlight/keyword-commands";
import { getAllTools, isRealToolId } from "../../composables/toolCatalog";
import type { KeywordCommandCustom } from "../../spotlight/types";

interface ToolOption {
  id: string;
  name: string;
  desc: string;
}

const props = defineProps<{
  open: boolean;
  initial: KeywordCommandCustom | null;
  existingCustom: KeywordCommandCustom[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "save", custom: KeywordCommandCustom): void;
}>();

const visible = ref(false);
const keywordError = ref<string | null>(null);

interface FormState {
  id: string;
  keyword: string;
  name: string;
  description: string;
  kind: KeywordCommandCustom["kind"];
  toolId: string;
  forwardArgs: boolean;
  targetTag: string;
  enabled: boolean;
}

function emptyForm(): FormState {
  return {
    id: "",
    keyword: "",
    name: "",
    description: "",
    kind: "open-tool",
    toolId: "",
    forwardArgs: true,
    targetTag: "",
    enabled: true,
  };
}

const form = reactive<FormState>(emptyForm());

const isEdit = computed(() => !!form.id);

const toolOptions = computed<ToolOption[]>(() =>
  getAllTools()
    .filter((t) => isRealToolId(t.id))
    .map((t) => ({ id: t.id, name: t.name, desc: t.desc })),
);

watch(
  () => props.open,
  (next) => {
    visible.value = next;
    if (next) reset(props.initial);
  },
  { immediate: true },
);

function reset(initial: KeywordCommandCustom | null) {
  const base = emptyForm();
  if (initial) {
    base.id = initial.id;
    base.keyword = initial.keyword;
    base.name = initial.name;
    base.description = initial.description;
    base.kind = initial.kind;
    base.toolId = initial.toolId ?? "";
    base.forwardArgs = initial.forwardArgs ?? true;
    base.targetTag = initial.targetTag ?? "";
    base.enabled = initial.enabled;
  }
  Object.assign(form, base);
  keywordError.value = null;
}

function onChangeKind(kind: KeywordCommandCustom["kind"]) {
  form.kind = kind;
  if (kind !== "open-tool") {
    form.toolId = "";
  }
  if (kind === "open-tool") {
    form.targetTag = "";
  }
}

function validateKeyword(): boolean {
  const result = validateCustomKeyword(form.keyword, {
    selfId: form.id || undefined,
    existingCustom: props.existingCustom,
  });
  if (!result.ok) {
    keywordError.value = result.error ?? "无效关键字";
    return false;
  }
  form.keyword = result.normalized;
  keywordError.value = null;
  return true;
}

const canSave = computed(() => {
  if (!form.keyword.trim() || keywordError.value) return false;
  if (!form.name.trim()) return false;
  if (form.kind === "open-tool" && !form.toolId) return false;
  if ((form.kind === "vault-tag" || form.kind === "snippet-tag") && !form.targetTag.trim()) {
    return false;
  }
  return true;
});

function onSave() {
  if (!validateKeyword()) return;
  if (!canSave.value) return;
  const next: KeywordCommandCustom = {
    id: form.id || generateCustomKeywordId(),
    keyword: form.keyword.trim().toLowerCase(),
    name: form.name.trim(),
    description: form.description.trim(),
    kind: form.kind,
    enabled: form.enabled,
  };
  if (form.kind === "open-tool") {
    next.toolId = form.toolId;
    next.forwardArgs = form.forwardArgs;
  }
  if (form.kind === "vault-tag" || form.kind === "snippet-tag") {
    next.targetTag = form.targetTag.trim();
  }
  emit("save", next);
}

function onCancel() {
  emit("close");
}
</script>

<style scoped>
.kw-editor-form :deep(.el-form-item) {
  margin-bottom: 16px;
}

.kw-editor-hint {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  margin-top: 4px;
}

.kw-editor-error {
  font-size: 12px;
  color: #c45656;
  margin-top: 4px;
}

.kw-editor-inline {
  margin-left: 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>
