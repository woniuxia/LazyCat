<template>
  <div class="snippet-v2">
    <aside class="left-pane">
      <div class="left-header">代码片段</div>
      <el-input
        v-model="keyword"
        class="left-search"
        placeholder="搜索标题、内容、标签"
        clearable
        @input="onSearchInput"
      />

      <div class="filter-group">
        <button class="filter-chip" :class="{ active: viewPreset === 'all' }" @click="setPreset('all')">全部</button>
        <button class="filter-chip" :class="{ active: viewPreset === 'favorite' }" @click="setPreset('favorite')">收藏</button>
        <button class="filter-chip" :class="{ active: viewPreset === 'recent7' }" @click="setPreset('recent7')">最近 7 天</button>
        <button class="filter-chip" :class="{ active: viewPreset === 'untagged' }" @click="setPreset('untagged')">无标签</button>
      </div>

      <section class="left-section">
        <div class="section-title">标签</div>
        <div class="tag-list">
          <button
            class="tag-item"
            :class="{ active: selectedTag === '' }"
            @click="selectedTag = ''; loadSnippets()"
          >
            全部标签
          </button>
          <button
            v-for="item in tagStats"
            :key="item.tag"
            class="tag-item"
            :class="{ active: selectedTag === item.tag }"
            @click="selectedTag = item.tag; loadSnippets()"
          >
            <span>{{ item.tag }}</span>
            <span class="count">{{ item.count }}</span>
          </button>
        </div>
      </section>

    </aside>

    <section class="middle-pane">
      <header class="middle-header">
        <div class="middle-header-left">
          <div class="middle-header-title-row">
            <h2>片段列表</h2>
            <el-button type="primary" size="small" @click="createSnippet">新建片段</el-button>
          </div>
          <p>{{ listSubTitle }}</p>
        </div>
        <div class="middle-actions">
          <el-select v-model="sortBy" size="small" style="width: 140px" @change="loadSnippets">
            <el-option label="最近使用" value="last_used" />
            <el-option label="最近修改" value="updated_at" />
            <el-option label="最近创建" value="created_at" />
            <el-option label="标题" value="title" />
          </el-select>
        </div>
      </header>

      <div class="snippet-list">
        <button
          v-for="item in snippets"
          :key="item.id"
          class="snippet-item"
          :class="{ active: selectedId === item.id }"
          @click="selectSnippet(item.id)"
        >
          <div class="snippet-item-head">
            <span class="snippet-title">{{ item.title || "（空白标题）" }}</span>
            <span class="snippet-meta">{{ item.primaryLanguage }}</span>
          </div>
          <div class="snippet-item-desc">{{ item.description || '暂无描述' }}</div>
          <div class="snippet-item-footer">
            <span>{{ formatTime(item.lastUsedAt || item.updatedAt) }}</span>
            <span>{{ item.fragmentCount }} 段</span>
          </div>
          <div class="snippet-tags">
            <span v-for="tag in item.tags" :key="tag" class="list-tag">{{ tag }}</span>
          </div>
        </button>
        <div v-if="!snippets.length" class="empty">暂无结果</div>
      </div>
    </section>

    <section class="right-pane">
      <template v-if="current">
        <header class="editor-header">
          <el-input v-model="current.title" placeholder="片段标题" @input="scheduleSave" />
          <div class="editor-actions">
            <el-button text :type="current.isFavorite ? 'warning' : 'info'" @click="toggleFavorite">
              {{ current.isFavorite ? '取消收藏' : '收藏' }}
            </el-button>
            <el-button text type="primary" @click="copyCurrentCode">复制代码</el-button>
            <el-popconfirm title="确定删除该片段？" @confirm="deleteSnippet">
              <template #reference><el-button text type="danger">删除</el-button></template>
            </el-popconfirm>
          </div>
        </header>

        <el-input
          v-model="current.description"
          type="textarea"
          :rows="2"
          placeholder="描述（可选）"
          @input="scheduleSave"
        />

        <div class="meta-row">
          <div class="tag-block">
            <div class="tag-block-header">
              <span class="tag-block-title">标签</span>
              <el-autocomplete
                v-if="tagInputVisible"
                ref="tagInputRef"
                v-model="tagInput"
                size="small"
                style="width: 120px"
                :fetch-suggestions="queryTagSuggestions"
                placeholder="输入标签"
                @keyup.enter="confirmTag"
                @select="onSelectTagSuggestion"
                @blur="confirmTag"
              />
              <el-button v-else text size="small" @click="showTagInput">+标签</el-button>
            </div>
            <div class="tags-editor">
              <el-tag
                v-for="tag in current.tags"
                :key="tag"
                size="small"
                closable
                @close="removeTag(tag)"
              >{{ tag }}</el-tag>
            </div>
          </div>
        </div>

        <div class="fragment-row">
          <div class="fragment-tabs-inline">
            <el-tabs
              v-model="activeFragmentName"
              class="fragment-tabs"
              type="card"
              :addable="false"
              :closable="true"
              @tab-remove="onTabRemove"
            >
              <el-tab-pane v-for="(frag, idx) in current.fragments" :key="idx" :name="String(idx)">
                <template #label>
                  <span v-if="renamingIdx !== idx" @dblclick.stop="startRename(idx)">{{ frag.label }}</span>
                  <el-input
                    v-else
                    v-model="renameValue"
                    size="small"
                    style="width: 96px"
                    @keyup.enter="confirmRename"
                    @blur="confirmRename"
                  />
                </template>
              </el-tab-pane>
            </el-tabs>
            <el-button class="add-fragment-btn" text size="small" @click="addFragment">+片段</el-button>
          </div>
          <el-select
            v-if="activeFragment"
            v-model="activeFragment.language"
            size="small"
            style="width: 160px"
            filterable
            @change="onFragmentLanguageChange"
          >
            <el-option v-for="lang in languageOptions" :key="lang" :label="lang" :value="lang" />
          </el-select>
        </div>

        <div class="editor-body">
          <MonacoPane
            v-if="activeFragment"
            :key="current.id + '-' + activeFragmentName"
            v-model="activeFragment.code"
            :language="activeFragment.language"
            @update:model-value="scheduleSave"
          />
        </div>
      </template>
      <div v-else class="empty">请选择或创建片段</div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import MonacoPane from "./MonacoPane.vue";
import { invokeToolByChannel } from "../bridge/tauri";

interface SnippetSummary {
  id: number;
  title: string;
  description: string;
  isFavorite: boolean;
  primaryLanguage: string;
  createdAt: string;
  updatedAt: string;
  lastUsedAt: string;
  useCount: number;
  fragmentCount: number;
  tags: string[];
}

interface Fragment {
  id?: number;
  label: string;
  language: string;
  code: string;
  sortOrder: number;
}

interface SnippetDetail {
  id: number;
  title: string;
  description: string;
  isFavorite: boolean;
  fragments: Fragment[];
  tags: string[];
}

interface TagStat {
  tag: string;
  count: number;
}

const defaultLanguages = [
  "javascript", "typescript", "python", "java", "go", "rust", "sql", "html", "css",
  "json", "xml", "yaml", "bash", "shell", "markdown", "plaintext", "c", "cpp", "csharp",
  "php", "ruby", "swift", "kotlin", "scala", "lua", "r", "dart", "dockerfile", "graphql", "toml"
];

const languageExtensionMap: Record<string, string> = {
  javascript: "js",
  typescript: "ts",
  python: "py",
  java: "java",
  go: "go",
  rust: "rs",
  sql: "sql",
  html: "html",
  css: "css",
  json: "json",
  xml: "xml",
  yaml: "yml",
  bash: "sh",
  shell: "sh",
  markdown: "md",
  plaintext: "txt",
  c: "c",
  cpp: "cpp",
  csharp: "cs",
  php: "php",
  ruby: "rb",
  swift: "swift",
  kotlin: "kt",
  scala: "scala",
  lua: "lua",
  r: "r",
  dart: "dart",
  dockerfile: "dockerfile",
  graphql: "graphql",
  toml: "toml",
};

const keyword = ref("");
const selectedTag = ref("");
const sortBy = ref<"last_used" | "updated_at" | "created_at" | "title">("last_used");
const viewPreset = ref<"all" | "favorite" | "recent7" | "untagged">("all");
const snippets = ref<SnippetSummary[]>([]);
const current = ref<SnippetDetail | null>(null);
const selectedId = ref<number | null>(null);
const tagStats = ref<TagStat[]>([]);
const activeFragmentName = ref("0");
const tagInputVisible = ref(false);
const tagInput = ref("");
const tagInputRef = ref();
const renamingIdx = ref<number | null>(null);
const renameValue = ref("");
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let searchTimer: ReturnType<typeof setTimeout> | null = null;

const listSubTitle = computed(() => `共 ${snippets.value.length} 条结果`);

const activeFragment = computed(() => {
  if (!current.value) return null;
  const idx = Number(activeFragmentName.value);
  return current.value.fragments[idx] ?? null;
});

const languageOptions = computed(() => {
  const used = new Set<string>();
  if (current.value) {
    for (const frag of current.value.fragments) used.add(frag.language);
  }
  return [...Array.from(used), ...defaultLanguages.filter((l) => !used.has(l))];
});

async function ipc<T>(channel: string, payload: Record<string, unknown> = {}): Promise<T> {
  return (await invokeToolByChannel(channel, payload)) as T;
}

async function ensureInitialized() {
  const check = await ipc<{ initialized: boolean; requiresConfirm: boolean }>("tool:snippets:v2:init", { confirm: false });
  if (check.initialized) return true;

  const { value } = await ElMessageBox.prompt(
    "首次进入代码片段工作区将清空旧片段数据。请输入 DELETE 继续。",
    "危险操作确认",
    {
      inputPlaceholder: "DELETE",
      confirmButtonText: "确认清空",
      cancelButtonText: "取消",
    }
  );

  if ((value ?? "").trim().toUpperCase() !== "DELETE") {
    ElMessage.warning("未输入 DELETE，已取消初始化");
    return false;
  }

  await ipc("tool:snippets:v2:init", { confirm: true });
  ElMessage.success("代码片段工作区已重建");
  return true;
}

function buildQuery() {
  return {
    tag: selectedTag.value,
    sort_by: sortBy.value,
    favorite_only: viewPreset.value === "favorite",
    untagged_only: viewPreset.value === "untagged",
    recent_days: viewPreset.value === "recent7" ? 7 : 0,
  };
}

async function loadSnippets() {
  const query = buildQuery();
  const data = keyword.value.trim()
    ? await ipc<SnippetSummary[]>("tool:snippets:v2:search", { ...query, keyword: keyword.value.trim() })
    : await ipc<SnippetSummary[]>("tool:snippets:v2:list", query);

  snippets.value = data;
  if (selectedId.value && !data.some((item) => item.id === selectedId.value)) {
    selectedId.value = null;
    current.value = null;
  }
}

async function loadMeta() {
  tagStats.value = await ipc<TagStat[]>("tool:snippets:v2:tag-stats");
}

async function selectSnippet(id: number) {
  selectedId.value = id;
  current.value = await ipc<SnippetDetail>("tool:snippets:v2:get", { id });
  activeFragmentName.value = "0";
  await ipc("tool:snippets:v2:mark-used", { id });
  void loadSnippets();
}

async function createSnippet() {
  const created = await ipc<SnippetDetail>("tool:snippets:v2:create", {
    title: "",
    description: "",
    tags: selectedTag.value ? [selectedTag.value] : [],
    fragments: [{ label: "main", language: "plaintext", code: "" }],
  });
  await loadMeta();
  await loadSnippets();
  await selectSnippet(created.id);
}

function scheduleSave() {
  if (!current.value) return;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    if (!current.value) return;
    await ipc("tool:snippets:v2:update", {
      id: current.value.id,
      title: current.value.title,
      description: current.value.description,
      isFavorite: current.value.isFavorite,
      tags: current.value.tags,
      fragments: current.value.fragments,
    });
    await loadSnippets();
    await loadMeta();
  }, 420);
}

async function deleteSnippet() {
  if (!current.value) return;
  await ipc("tool:snippets:v2:delete", { id: current.value.id });
  current.value = null;
  selectedId.value = null;
  await loadSnippets();
  await loadMeta();
}

function setPreset(preset: "all" | "favorite" | "recent7" | "untagged") {
  viewPreset.value = preset;
  void loadSnippets();
}

function onSearchInput() {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    void loadSnippets();
  }, 260);
}

function showTagInput() {
  tagInputVisible.value = true;
  void nextTick(() => tagInputRef.value?.focus?.());
}

function confirmTag() {
  const val = tagInput.value.trim();
  if (current.value && val && !current.value.tags.includes(val)) {
    current.value.tags.push(val);
    scheduleSave();
  }
  tagInput.value = "";
  tagInputVisible.value = false;
}

function queryTagSuggestions(queryString: string, cb: (items: Array<{ value: string }>) => void) {
  const existed = new Set((current.value?.tags ?? []).map((tag) => tag.toLowerCase()));
  const normalizedQuery = queryString.trim().toLowerCase();
  const candidates = tagStats.value
    .map((item) => item.tag)
    .filter((tag) => !existed.has(tag.toLowerCase()));

  const filtered = normalizedQuery
    ? candidates.filter((tag) => tag.toLowerCase().includes(normalizedQuery))
    : candidates;

  cb(filtered.slice(0, 8).map((tag) => ({ value: tag })));
}

function onSelectTagSuggestion(item: { value: string }) {
  tagInput.value = item.value;
  confirmTag();
}

function removeTag(tag: string) {
  if (!current.value) return;
  current.value.tags = current.value.tags.filter((t) => t !== tag);
  scheduleSave();
}

function startRename(index: number) {
  if (!current.value) return;
  renamingIdx.value = index;
  renameValue.value = current.value.fragments[index]?.label ?? "";
}

function confirmRename() {
  if (!current.value || renamingIdx.value === null) return;
  const idx = renamingIdx.value;
  const value = renameValue.value.trim();
  current.value.fragments[idx].label = value || `片段 ${idx + 1}`;
  renamingIdx.value = null;
  scheduleSave();
}

function addFragment() {
  if (!current.value) return;
  const idx = current.value.fragments.length;
  current.value.fragments.push({
    label: `片段 ${idx + 1}`,
    language: "plaintext",
    code: "",
    sortOrder: idx,
  });
  activeFragmentName.value = String(idx);
  scheduleSave();
}

function onTabRemove(targetName: string | number) {
  if (!current.value) return;
  const idx = Number(targetName);
  if (!Number.isFinite(idx) || current.value.fragments.length <= 1) return;
  current.value.fragments.splice(idx, 1);
  current.value.fragments.forEach((frag, i) => { frag.sortOrder = i; });
  activeFragmentName.value = String(Math.max(0, idx - 1));
  scheduleSave();
}

function onFragmentLanguageChange() {
  if (!activeFragment.value) return;
  const fragment = activeFragment.value;
  const currentLabel = fragment.label.trim();

  if (currentLabel && !/\.[a-z0-9]+$/i.test(currentLabel)) {
    const ext = languageExtensionMap[fragment.language.toLowerCase()] ?? fragment.language.toLowerCase();
    fragment.label = `${currentLabel}.${ext}`;
  }

  scheduleSave();
}

function formatTime(value: string): string {
  if (!value) return "";
  const date = new Date(value.replace(" ", "T"));
  const now = new Date();
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  }
  return date.toLocaleDateString("zh-CN", { month: "2-digit", day: "2-digit" });
}

async function copyCurrentCode() {
  if (!activeFragment.value) return;
  await navigator.clipboard.writeText(activeFragment.value.code || "");
  ElMessage.success("代码已复制");
  if (current.value) {
    await ipc("tool:snippets:v2:mark-used", { id: current.value.id });
    await loadSnippets();
  }
}

async function toggleFavorite() {
  if (!current.value) return;
  current.value.isFavorite = !current.value.isFavorite;
  scheduleSave();
}

onMounted(async () => {
  try {
    const ready = await ensureInitialized();
    if (!ready) return;
    await loadMeta();
    await loadSnippets();
  } catch (error) {
    ElMessage.error((error as Error).message || "代码片段工作区初始化失败");
  }
});

onBeforeUnmount(() => {
  if (saveTimer) clearTimeout(saveTimer);
  if (searchTimer) clearTimeout(searchTimer);
});
</script>

<style scoped>
.snippet-v2 {
  display: grid;
  grid-template-columns: 260px 320px 1fr;
  height: 100%;
  overflow: hidden;
  background: var(--lc-surface-0);
}

.left-pane,
.middle-pane,
.right-pane {
  min-height: 0;
}

.left-pane {
  border-right: 1px solid var(--lc-border);
  padding: 14px 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
}

.left-header {
  font-size: 16px;
  font-weight: 700;
  color: var(--lc-text);
}

.filter-group {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.filter-chip {
  border: 1px solid var(--lc-border);
  background: var(--lc-surface-1);
  color: var(--lc-text-secondary);
  border-radius: 8px;
  padding: 6px 8px;
  font-size: 12px;
  cursor: pointer;
}

.filter-chip.active {
  border-color: var(--lc-accent);
  color: var(--lc-accent);
  background: var(--lc-accent-dim);
}

.left-section {
  border-top: 1px solid var(--lc-border-subtle);
  padding-top: 10px;
}

.section-title {
  font-size: 12px;
  color: var(--lc-text-muted);
  margin-bottom: 6px;
}

.tag-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.tag-item {
  border: none;
  background: transparent;
  color: var(--lc-text-secondary);
  border-radius: 8px;
  text-align: left;
  padding: 6px 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.tag-item:hover {
  background: var(--lc-surface-2);
  color: var(--lc-text);
}

.tag-item.active {
  background: var(--lc-accent-dim);
  color: var(--lc-accent);
}

.count {
  font-size: 11px;
  color: var(--lc-text-muted);
}

.middle-pane {
  border-right: 1px solid var(--lc-border);
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.middle-header {
  padding: 14px 14px 10px;
  border-bottom: 1px solid var(--lc-border-subtle);
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 8px;
}

.middle-header-left {
  min-width: 0;
}

.middle-header-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.middle-header h2 {
  margin: 0;
  font-size: 15px;
}

.middle-header p {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--lc-text-muted);
}

.middle-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.snippet-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

.snippet-item {
  width: 100%;
  border: 1px solid var(--lc-border-subtle);
  background: var(--lc-surface-1);
  border-radius: 10px;
  padding: 10px;
  margin-bottom: 8px;
  text-align: left;
  cursor: pointer;
}

.snippet-item.active {
  border-color: var(--lc-accent);
  box-shadow: inset 0 0 0 1px var(--lc-accent-dim);
}

.snippet-item-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}

.snippet-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--lc-text);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.snippet-meta,
.snippet-item-footer {
  font-size: 11px;
  color: var(--lc-text-muted);
}

.snippet-item-desc {
  margin-top: 6px;
  color: var(--lc-text-secondary);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.snippet-item-footer {
  margin-top: 8px;
  display: grid;
  grid-template-columns: 1fr auto;
  align-items: center;
  column-gap: 8px;
}

.snippet-tags {
  margin-top: 6px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.list-tag {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 99px;
  background: var(--lc-surface-2);
  color: var(--lc-text-muted);
}

.right-pane {
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 14px;
  gap: 10px;
}

.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.editor-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.meta-row {
  display: flex;
  align-items: flex-start;
  justify-content: flex-start;
  gap: 10px;
}

.tag-block {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
}

.tag-block-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tag-block-title {
  font-size: 12px;
  color: var(--lc-text-muted);
}

.tags-editor {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.fragment-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.fragment-tabs-inline {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  min-width: 0;
}

.fragment-tabs {
  flex: 0 1 auto;
  min-width: 0;
}

.fragment-tabs-inline :deep(.el-tabs__header) {
  margin: 0;
}

.add-fragment-btn {
  flex-shrink: 0;
}

.editor-body {
  flex: 1;
  min-height: 0;
  border: 1px solid var(--lc-border-subtle);
  border-radius: 10px;
  overflow: hidden;
}

.empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--lc-text-muted);
  font-size: 13px;
}

@media (max-width: 1200px) {
  .snippet-v2 {
    grid-template-columns: 220px 280px 1fr;
  }
}

@media (max-width: 960px) {
  .snippet-v2 {
    grid-template-columns: 1fr;
    grid-template-rows: 260px 1fr 1fr;
  }

  .left-pane,
  .middle-pane {
    border-right: none;
    border-bottom: 1px solid var(--lc-border);
  }
}
</style>
