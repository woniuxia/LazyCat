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

    <div class="spotlight-settings-actions">
      <el-button @click="resetToDefault">恢复默认</el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { ElMessage } from "element-plus";
import * as configStore from "../../spotlight/config-store";
import { listDescriptors } from "../../spotlight/registry";
import { QUICK_COMMAND_DESCRIPTORS } from "../../spotlight/quick-commands";
// 确保所有 provider 都已注册(若 SettingsPanel 先于 SpotlightPanel 打开)
import "../../spotlight/providers/tool";
import "../../spotlight/providers/vault";
import "../../spotlight/providers/hosts";
import "../../spotlight/providers/todo";
import "../../spotlight/providers/pm";
import "../../spotlight/providers/suggestion";
import "../../spotlight/providers/launcher";
import type {
  ProviderDescriptor,
  QuickCommandDescriptor,
  QuickCommandId,
  SpotlightConfig,
  SpotlightProviderId,
} from "../../spotlight/types";

const editableProviders = ref<ProviderDescriptor[]>([]);
const quickCommands: QuickCommandDescriptor[] = QUICK_COMMAND_DESCRIPTORS;
const config = ref<SpotlightConfig>(configStore.buildDefaultConfig());
const aliasInputs = reactive<Record<string, string>>({});
const aliasErrors = reactive<Record<string, string>>({});
const loadError = ref<string | null>(null);

let unsub: (() => void) | null = null;

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

async function persist(next: SpotlightConfig) {
  try {
    await configStore.saveConfig(next);
  } catch (err) {
    ElMessage.error(`保存失败:${err instanceof Error ? err.message : String(err)}`);
    syncFromStore();
  }
}

async function onToggleProvider(id: SpotlightProviderId, value: boolean) {
  const next: SpotlightConfig = JSON.parse(JSON.stringify(config.value));
  next.providers[id] = { ...(next.providers[id] ?? {}), enabled: value };
  await persist(next);
}

async function onToggleQuickCommand(id: QuickCommandId, value: boolean) {
  const next: SpotlightConfig = JSON.parse(JSON.stringify(config.value));
  next.quickCommands[id] = { enabled: value };
  await persist(next);
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
  const next: SpotlightConfig = JSON.parse(JSON.stringify(config.value));
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
