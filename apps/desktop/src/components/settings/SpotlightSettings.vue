<template>
  <div class="spotlight-settings">
    <div v-if="loadError" class="spotlight-settings-error">
      配置加载失败,已使用默认值。详情:{{ loadError }}
    </div>

    <div class="spotlight-settings-group">
      <div class="group-title">数据源</div>
      <div
        v-for="p in editableProviders"
        :key="p.id"
        class="provider-row"
      >
        <el-switch
          :model-value="resolveEnabled(p.id, p.defaultEnabled)"
          @update:model-value="(v: boolean) => onToggleProvider(p.id, v)"
        />
        <div class="provider-meta">
          <div class="provider-name">{{ p.name }}</div>
          <div class="provider-desc">{{ p.description }}</div>
        </div>
        <div class="provider-aliases">
          <div class="alias-label">scope 别名(逗号分隔)</div>
          <el-input
            :model-value="aliasInputs[p.id]"
            placeholder="例如:t, todo"
            @update:model-value="(v: string) => (aliasInputs[p.id] = v)"
            @blur="commitAliases(p.id)"
          />
          <div v-if="aliasErrors[p.id]" class="alias-error">
            {{ aliasErrors[p.id] }}
          </div>
          <div class="alias-default-hint">
            默认:{{ p.defaultAliases.join(", ") || "(无)" }}
          </div>
        </div>
      </div>
    </div>

    <div class="spotlight-settings-group">
      <div class="group-title">快速命令</div>
      <div
        v-for="qc in quickCommands"
        :key="qc.id"
        class="quick-command-row"
      >
        <el-switch
          :model-value="resolveQuickEnabled(qc.id, qc.defaultEnabled)"
          @update:model-value="(v: boolean) => onToggleQuickCommand(qc.id, v)"
        />
        <div class="quick-command-meta">
          <div class="quick-command-name">{{ qc.name }}</div>
          <div class="quick-command-desc">{{ qc.description }}</div>
        </div>
      </div>
    </div>

    <div class="spotlight-settings-group">
      <div class="group-title-row">
        <span class="group-title">关键字命令(; 前缀)</span>
        <el-button size="small" type="primary" plain @click="onAddCustom">
          + 添加
        </el-button>
      </div>

      <div class="kw-section-label">内置命令</div>
      <div
        v-for="b in builtinList"
        :key="b.id"
        class="quick-command-row"
      >
        <el-switch
          :model-value="resolveBuiltinEnabled(b.id, b.defaultEnabled)"
          @update:model-value="(v: boolean) => onToggleBuiltin(b.id, v)"
        />
        <div class="quick-command-meta">
          <div class="quick-command-name">; {{ b.keyword }} — {{ b.name }}</div>
          <div class="quick-command-desc">{{ b.description }}</div>
        </div>
        <span class="kw-tag kw-tag-builtin">内置</span>
      </div>

      <div class="kw-section-label">自定义命令</div>
      <div v-if="customList.length === 0" class="kw-empty">
        还没有自定义命令。点击右上角"+ 添加"创建一个,例如 <code>;wifi</code> 列出 Vault 中 tag=wifi 的密码。
      </div>
      <div
        v-for="c in customList"
        :key="c.id"
        class="quick-command-row"
      >
        <el-switch
          :model-value="c.enabled"
          @update:model-value="(v: boolean) => onToggleCustom(c.id, v)"
        />
        <div class="quick-command-meta">
          <div class="quick-command-name">
            ; {{ c.keyword }} — {{ c.name || "(未命名)" }}
          </div>
          <div class="quick-command-desc">
            {{ describeCustom(c) }}
          </div>
        </div>
        <el-button size="small" link @click="onEditCustom(c)">编辑</el-button>
        <el-button size="small" link type="danger" @click="onDeleteCustom(c.id)">
          删除
        </el-button>
      </div>
    </div>

    <div class="spotlight-settings-actions">
      <el-button @click="resetToDefault">恢复默认</el-button>
    </div>

    <KeywordCommandEditor
      :open="editorOpen"
      :initial="editorInitial"
      :existing-custom="customList"
      @close="editorOpen = false"
      @save="onSaveCustom"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import * as configStore from "../../spotlight/config-store";
import { listDescriptors } from "../../spotlight/registry";
import { QUICK_COMMAND_DESCRIPTORS } from "../../spotlight/quick-commands";
import { BUILTIN_KEYWORD_COMMANDS } from "../../spotlight/keyword-commands";
import KeywordCommandEditor from "./KeywordCommandEditor.vue";
// 确保所有 provider 都已注册(若 SettingsPanel 先于 SpotlightPanel 打开)
import "../../spotlight/providers/tool";
import "../../spotlight/providers/vault";
import "../../spotlight/providers/hosts";
import "../../spotlight/providers/todo";
import "../../spotlight/providers/pm";
import "../../spotlight/providers/suggestion";
import "../../spotlight/providers/launcher";
import type {
  KeywordCommandCustom,
  ProviderDescriptor,
  QuickCommandDescriptor,
  QuickCommandId,
  SpotlightConfig,
  SpotlightProviderId,
} from "../../spotlight/types";

const editableProviders = ref<ProviderDescriptor[]>([]);
const quickCommands: QuickCommandDescriptor[] = QUICK_COMMAND_DESCRIPTORS;
const builtinList = BUILTIN_KEYWORD_COMMANDS;
const config = ref<SpotlightConfig>(configStore.buildDefaultConfig());
const aliasInputs = reactive<Record<string, string>>({});
const aliasErrors = reactive<Record<string, string>>({});
const loadError = ref<string | null>(null);

const editorOpen = ref(false);
const editorInitial = ref<KeywordCommandCustom | null>(null);

let unsub: (() => void) | null = null;

const customList = computed<KeywordCommandCustom[]>(() => {
  return config.value.keywordCommands?.custom ?? [];
});

function syncFromStore() {
  config.value = JSON.parse(JSON.stringify(configStore.getConfig())) as SpotlightConfig;
  editableProviders.value = listDescriptors();
  for (const p of editableProviders.value) {
    const override = config.value.providers[p.id];
    const aliases = override?.aliases ?? p.defaultAliases;
    aliasInputs[p.id] = aliases.join(", ");
    aliasErrors[p.id] = "";
  }
  loadError.value = configStore.getLastLoadError();
}

onMounted(async () => {
  try {
    await configStore.ensureLoaded();
  } catch (err) {
    loadError.value = err instanceof Error ? err.message : String(err);
  }
  syncFromStore();
  unsub = configStore.subscribe(() => syncFromStore());
  void configStore.startListening();
});

onBeforeUnmount(() => {
  unsub?.();
  unsub = null;
});

function resolveEnabled(id: SpotlightProviderId, defaultEnabled: boolean): boolean {
  return config.value.providers[id]?.enabled ?? defaultEnabled;
}

function resolveQuickEnabled(id: QuickCommandId, defaultEnabled: boolean): boolean {
  return config.value.quickCommands[id]?.enabled ?? defaultEnabled;
}

function resolveBuiltinEnabled(id: string, defaultEnabled: boolean): boolean {
  return config.value.keywordCommands?.builtins?.[id]?.enabled ?? defaultEnabled;
}

async function persist(next: SpotlightConfig) {
  try {
    await configStore.saveConfig(next);
  } catch (err) {
    ElMessage.error(`保存失败:${err instanceof Error ? err.message : String(err)}`);
    syncFromStore();
  }
}

function cloneConfig(): SpotlightConfig {
  return JSON.parse(JSON.stringify(config.value)) as SpotlightConfig;
}

async function onToggleProvider(id: SpotlightProviderId, value: boolean) {
  const next = cloneConfig();
  next.providers[id] = { ...(next.providers[id] ?? {}), enabled: value };
  await persist(next);
}

async function onToggleQuickCommand(id: QuickCommandId, value: boolean) {
  const next = cloneConfig();
  next.quickCommands[id] = { enabled: value };
  await persist(next);
}

async function onToggleBuiltin(id: string, value: boolean) {
  const next = cloneConfig();
  const kw = (next.keywordCommands ??= {});
  const builtins = (kw.builtins ??= {});
  builtins[id] = { enabled: value };
  await persist(next);
}

async function onToggleCustom(id: string, value: boolean) {
  const next = cloneConfig();
  const kw = (next.keywordCommands ??= {});
  const custom = (kw.custom ??= []);
  const target = custom.find((c) => c.id === id);
  if (!target) return;
  target.enabled = value;
  await persist(next);
}

function onAddCustom() {
  editorInitial.value = null;
  editorOpen.value = true;
}

function onEditCustom(item: KeywordCommandCustom) {
  editorInitial.value = JSON.parse(JSON.stringify(item)) as KeywordCommandCustom;
  editorOpen.value = true;
}

async function onSaveCustom(custom: KeywordCommandCustom) {
  const next = cloneConfig();
  const kw = (next.keywordCommands ??= {});
  const list = (kw.custom ??= []);
  const idx = list.findIndex((c) => c.id === custom.id);
  if (idx >= 0) {
    list[idx] = custom;
  } else {
    list.push(custom);
  }
  await persist(next);
  editorOpen.value = false;
  ElMessage.success(idx >= 0 ? "已更新关键字命令" : "已添加关键字命令");
}

async function onDeleteCustom(id: string) {
  try {
    await ElMessageBox.confirm("确定要删除这个关键字命令吗?", "删除确认", {
      type: "warning",
    });
  } catch {
    return;
  }
  const next = cloneConfig();
  const list = next.keywordCommands?.custom;
  if (!list) return;
  next.keywordCommands = {
    ...next.keywordCommands,
    custom: list.filter((c) => c.id !== id),
  };
  await persist(next);
  ElMessage.success("已删除关键字命令");
}

function describeCustom(c: KeywordCommandCustom): string {
  if (c.kind === "open-tool") {
    const args = c.forwardArgs === false ? "(不透传参数)" : "(透传参数)";
    return `直达工具 → ${c.toolId || "(未选)"} ${args}`;
  }
  if (c.kind === "vault-tag") {
    return `查 Vault Tag → #${c.targetTag || "(未填)"}`;
  }
  if (c.kind === "snippet-tag") {
    return `查 Snippet Tag → #${c.targetTag || "(未填)"}`;
  }
  return c.description || "";
}

async function commitAliases(id: SpotlightProviderId) {
  const raw = aliasInputs[id] ?? "";
  const aliases = raw
    .split(/[,，]/)
    .map((s) => s.trim())
    .filter(Boolean);
  const result = configStore.validateAliases(aliases, id);
  if (!result.ok) {
    aliasErrors[id] = result.conflicts
      .map((c) => `「${c.alias}」${c.reason}`)
      .join(";");
    return;
  }
  aliasErrors[id] = "";
  const next = cloneConfig();
  next.providers[id] = { ...(next.providers[id] ?? {}), aliases: result.normalized };
  await persist(next);
}

async function resetToDefault() {
  await persist(configStore.buildDefaultConfig());
  ElMessage.success("已恢复 Spotlight 默认配置");
}
</script>

<style scoped>
.spotlight-settings {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.spotlight-settings-error {
  padding: 10px 12px;
  border-radius: 8px;
  background: rgba(245, 108, 108, 0.08);
  color: #c45656;
  font-size: 12px;
}

.spotlight-settings-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.group-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.group-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.kw-section-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
}

.kw-empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  padding: 12px 14px;
  background: var(--el-fill-color-blank);
  border-radius: 8px;
}

.kw-empty code {
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
  font-family: var(--el-font-family);
}

.kw-tag {
  flex-shrink: 0;
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 6px;
  white-space: nowrap;
}

.kw-tag-builtin {
  background: rgba(144, 147, 153, 0.14);
  color: #606266;
}

.provider-row {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  padding: 12px 14px;
  background: var(--el-fill-color-blank);
  border-radius: 8px;
}

.provider-meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.provider-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.provider-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.provider-aliases {
  flex: 1.4;
  min-width: 200px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.alias-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.alias-default-hint {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
}

.alias-error {
  font-size: 12px;
  color: #c45656;
}

.quick-command-row {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 14px;
  background: var(--el-fill-color-blank);
  border-radius: 8px;
}

.quick-command-meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.quick-command-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.quick-command-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.spotlight-settings-actions {
  display: flex;
  justify-content: flex-end;
}
</style>
