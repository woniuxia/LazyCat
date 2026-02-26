<template>
  <el-dialog
    v-model="visible"
    title="配置菜单显示与搜索别名"
    width="860px"
    :close-on-click-modal="false"
    @open="onOpen"
  >
    <div class="menu-dialog">
      <p class="menu-dialog-tip">
        可配置每个功能的别名、缩写和描述。隐藏菜单项后，仍可通过首页搜索访问。
      </p>
      <el-input
        v-model="query"
        placeholder="搜索功能名、分组、别名、缩写或描述"
        clearable
      />
      <div class="menu-list">
        <div
          v-for="tool in filteredTools"
          :key="tool.id"
          class="tool-row"
        >
          <div class="tool-row-head">
            <div class="tool-main">
              <span class="tool-name">{{ tool.name }}</span>
              <span class="tool-group">{{ tool.groupName }}</span>
            </div>
            <el-switch
              :model-value="isVisible(tool.id)"
              @change="(v) => setVisible(tool.id, !!v)"
            />
          </div>

          <div class="tool-row-fields">
            <div class="field">
              <span class="label">别名</span>
              <div class="alias-editor">
                <el-tag
                  v-for="alias in aliasesDraft[tool.id] ?? []"
                  :key="alias"
                  closable
                  @close="removeAlias(tool.id, alias)"
                >
                  {{ alias }}
                </el-tag>
                <el-input
                  v-model="aliasInput[tool.id]"
                  size="small"
                  placeholder="回车添加，支持逗号分隔"
                  @keydown.enter.prevent="appendAliasInput(tool.id)"
                  @blur="appendAliasInput(tool.id)"
                />
              </div>
            </div>

            <div class="field field-compact">
              <span class="label">缩写</span>
              <el-input
                v-model="abbreviationDraft[tool.id]"
                size="small"
                placeholder="如 jp / cr / mdn"
              />
            </div>

            <div class="field">
              <span class="label">描述</span>
              <el-input
                v-model="descriptionDraft[tool.id]"
                size="small"
                placeholder="补充用于搜索的描述词"
              />
            </div>
          </div>
        </div>
      </div>
    </div>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" @click="onSave">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import type { SidebarItem, ToolSearchMetaMap } from "../types";

interface ToolRow {
  id: string;
  name: string;
  groupName: string;
}

const props = defineProps<{
  sidebarItems: SidebarItem[];
  getHiddenIds: () => string[];
  setHiddenIds: (ids: string[]) => void;
  getToolSearchMetaMap: () => ToolSearchMetaMap;
  setToolSearchMetaMap: (map: ToolSearchMetaMap) => void;
}>();

const visible = ref(false);
const query = ref("");
const hiddenSet = ref<Set<string>>(new Set());
const aliasesDraft = ref<Record<string, string[]>>({});
const abbreviationDraft = ref<Record<string, string>>({});
const descriptionDraft = ref<Record<string, string>>({});
const aliasInput = ref<Record<string, string>>({});

const tools = computed<ToolRow[]>(() => {
  const rows: ToolRow[] = [];
  for (const item of props.sidebarItems) {
    if (item.kind === "tool") {
      rows.push({
        id: item.tool.id,
        name: item.tool.name,
        groupName: "工具",
      });
      continue;
    }
    for (const tool of item.group.tools) {
      rows.push({
        id: tool.id,
        name: tool.name,
        groupName: item.group.name,
      });
    }
  }
  return rows;
});

const filteredTools = computed<ToolRow[]>(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return tools.value;
  return tools.value.filter((tool) => {
    const aliases = aliasesDraft.value[tool.id] ?? [];
    const abbreviation = abbreviationDraft.value[tool.id] ?? "";
    const description = descriptionDraft.value[tool.id] ?? "";
    return (
      tool.name.toLowerCase().includes(q) ||
      tool.groupName.toLowerCase().includes(q) ||
      aliases.some((alias) => alias.toLowerCase().includes(q)) ||
      abbreviation.toLowerCase().includes(q) ||
      description.toLowerCase().includes(q)
    );
  });
});

function normalizeAliasList(values: string[]): string[] {
  const uniq = new Set<string>();
  for (const value of values) {
    const trimmed = value.trim();
    if (!trimmed) continue;
    uniq.add(trimmed);
  }
  return [...uniq];
}

function allToolIds(): string[] {
  return tools.value.map((tool) => tool.id);
}

function isVisible(toolId: string): boolean {
  return !hiddenSet.value.has(toolId);
}

function setVisible(toolId: string, visibleFlag: boolean) {
  const next = new Set(hiddenSet.value);
  if (visibleFlag) next.delete(toolId);
  else next.add(toolId);
  hiddenSet.value = next;
}

function appendAliasInput(toolId: string) {
  const raw = aliasInput.value[toolId] ?? "";
  if (!raw.trim()) return;
  const chunks = raw.split(",").map((it) => it.trim()).filter(Boolean);
  if (chunks.length === 0) return;

  const current = aliasesDraft.value[toolId] ?? [];
  aliasesDraft.value[toolId] = normalizeAliasList([...current, ...chunks]);
  aliasInput.value[toolId] = "";
}

function removeAlias(toolId: string, alias: string) {
  aliasesDraft.value[toolId] = (aliasesDraft.value[toolId] ?? []).filter((item) => item !== alias);
}

function onOpen() {
  query.value = "";
  hiddenSet.value = new Set(props.getHiddenIds());

  const sourceMeta = props.getToolSearchMetaMap() ?? {};
  const aliases: Record<string, string[]> = {};
  const abbreviation: Record<string, string> = {};
  const description: Record<string, string> = {};
  const aliasInputMap: Record<string, string> = {};

  for (const tool of tools.value) {
    const meta = sourceMeta[tool.id];
    aliases[tool.id] = normalizeAliasList(meta?.aliases ?? []);
    abbreviation[tool.id] = meta?.abbreviation ?? "";
    description[tool.id] = meta?.description ?? "";
    aliasInputMap[tool.id] = "";
  }

  aliasesDraft.value = aliases;
  abbreviationDraft.value = abbreviation;
  descriptionDraft.value = description;
  aliasInput.value = aliasInputMap;
}

function onSave() {
  const visibleCount = allToolIds().filter((id) => !hiddenSet.value.has(id)).length;
  if (visibleCount === 0) {
    ElMessage.warning("至少保留一个可见功能");
    return;
  }

  const metaMap: ToolSearchMetaMap = {};
  for (const tool of tools.value) {
    const aliases = normalizeAliasList(aliasesDraft.value[tool.id] ?? []);
    const abbreviation = (abbreviationDraft.value[tool.id] ?? "").trim();
    const description = (descriptionDraft.value[tool.id] ?? "").trim();
    metaMap[tool.id] = {
      aliases,
      abbreviation,
      description,
    };
  }

  props.setHiddenIds([...hiddenSet.value]);
  props.setToolSearchMetaMap(metaMap);
  visible.value = false;
  ElMessage.success("菜单与搜索别名配置已保存");
}

function show() {
  visible.value = true;
}

defineExpose({ show });
</script>

<style scoped>
.menu-dialog {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.menu-dialog-tip {
  margin: 0;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}

.menu-list {
  max-height: 520px;
  overflow: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
}

.tool-row {
  padding: 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.tool-row:last-child {
  border-bottom: none;
}

.tool-row-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.tool-main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.tool-name {
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.tool-group {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.tool-row-fields {
  display: grid;
  grid-template-columns: 2fr 1fr 2fr;
  gap: 8px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.alias-editor {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.alias-editor :deep(.el-input) {
  width: 180px;
}

@media (max-width: 900px) {
  .tool-row-fields {
    grid-template-columns: 1fr;
  }

  .alias-editor :deep(.el-input) {
    width: 100%;
  }
}
</style>
