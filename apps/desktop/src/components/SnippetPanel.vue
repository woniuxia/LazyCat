<template>
  <div class="snippet-panel">
    <!-- 左栏：导航 -->
    <aside class="snippet-nav">
      <div class="nav-section">
        <div class="nav-item" :class="{ active: navMode === 'all' }" @click="setNav('all')">
          全部片段 <span class="count">({{ totalCount }})</span>
        </div>
        <div class="nav-item" :class="{ active: navMode === 'favorite' }" @click="setNav('favorite')">
          收藏 <span class="count">({{ favoriteCount }})</span>
        </div>
      </div>

      <div class="nav-section">
        <div class="nav-section-header">
          <span>文件夹</span>
          <el-button text size="small" @click="createFolder">+</el-button>
        </div>
        <el-tree
          ref="folderTreeRef"
          :data="folderTree"
          node-key="id"
          :props="{ label: 'name', children: 'children' }"
          :highlight-current="true"
          :expand-on-click-node="false"
          @node-click="onFolderClick"
          @node-contextmenu="onFolderContext"
        >
          <template #default="{ data }">
            <span class="folder-node">
              <span>{{ data.name }}</span>
              <span class="count">({{ data.snippetCount ?? 0 }})</span>
            </span>
          </template>
        </el-tree>
      </div>

      <div class="nav-section">
        <div class="nav-section-header"><span>标签</span></div>
        <div
          v-for="t in tagList"
          :key="t.tag"
          class="nav-item tag-item"
          :class="{ active: navMode === 'tag' && activeTag === t.tag }"
          @click="setNav('tag', t.tag)"
        >
          {{ t.tag }} <span class="count">({{ t.count }})</span>
        </div>
<!-- PLACEHOLDER_TEMPLATE_CONTINUE -->
        <div v-if="tagList.length === 0" class="nav-empty">暂无标签</div>
      </div>
    </aside>

    <!-- 中栏：片段列表 -->
    <section class="snippet-list">
      <div class="list-toolbar">
        <el-input v-model="searchKeyword" placeholder="搜索..." clearable size="small" @input="onSearchDebounced" />
        <el-select v-model="filterLanguage" placeholder="语言" clearable size="small" style="width: 110px; margin-left: 6px" @change="loadSnippets">
          <el-option v-for="lang in sortedLanguages" :key="lang" :label="lang" :value="lang" />
        </el-select>
      </div>
      <div class="list-sort">
        <el-radio-group v-model="sortBy" size="small" @change="loadSnippets">
          <el-radio-button value="updated_at">最近修改</el-radio-button>
          <el-radio-button value="created_at">最近创建</el-radio-button>
          <el-radio-button value="title">标题</el-radio-button>
        </el-radio-group>
      </div>
      <div class="list-items">
        <div
          v-for="s in snippetList"
          :key="s.id"
          class="snippet-card"
          :class="{ active: selectedId === s.id }"
          @click="selectSnippet(s.id)"
        >
          <div class="card-title">
            <span>{{ s.title }}</span>
            <span v-if="s.isFavorite" class="fav-icon">&#9733;</span>
          </div>
          <div class="card-meta">
            <span class="lang-badge">{{ s.language }}</span>
            <span class="time">{{ formatTime(s.updatedAt) }}</span>
          </div>
        </div>
        <div v-if="snippetList.length === 0" class="list-empty">暂无片段</div>
      </div>
      <div class="list-footer">
        <el-button type="primary" size="small" @click="createSnippet">新建片段</el-button>
      </div>
    </section>
<!-- PLACEHOLDER_EDITOR_SECTION -->
    <!-- 右栏：编辑器 -->
    <section v-if="current" class="snippet-editor">
      <div class="editor-header">
        <el-input v-model="current.title" placeholder="片段标题" @input="autoSave" />
        <div class="editor-actions">
          <el-button text :type="current.isFavorite ? 'warning' : 'info'" @click="toggleFav">
            {{ current.isFavorite ? "取消收藏" : "收藏" }}
          </el-button>
          <el-button text type="primary" @click="copyCode">复制代码</el-button>
          <el-popconfirm title="确定删除此片段？" @confirm="deleteSnippet">
            <template #reference><el-button text type="danger">删除</el-button></template>
          </el-popconfirm>
        </div>
      </div>

      <el-input v-model="current.description" type="textarea" :rows="2" placeholder="描述（可选）" @input="autoSave" style="margin-bottom: 8px" />

      <div class="tag-editor">
        <el-tag v-for="tag in current.tags" :key="tag" closable size="small" @close="removeTag(tag)" style="margin-right: 4px">{{ tag }}</el-tag>
        <el-input
          v-if="tagInputVisible"
          ref="tagInputRef"
          v-model="tagInputValue"
          size="small"
          style="width: 80px"
          @keyup.enter="confirmTag"
          @blur="confirmTag"
        />
        <el-button v-else text size="small" @click="showTagInput">+ 标签</el-button>
      </div>

      <div class="fragment-tabs">
        <el-tabs v-model="activeFragIdx" type="card" editable @edit="onFragTabEdit">
          <el-tab-pane
            v-for="(frag, idx) in current.fragments"
            :key="idx"
            :name="String(idx)"
          >
            <template #label>
              <span v-if="renamingFragIdx !== idx" @dblclick.stop="startFragRename(idx)">{{ frag.label }}</span>
              <el-input
                v-else
                v-model="fragRenameValue"
                size="small"
                style="width: 80px"
                @keyup.enter="confirmFragRename"
                @blur="confirmFragRename"
                @click.stop
                :ref="(el: any) => { if (el) fragRenameInputRef = el; }"
              />
            </template>
          </el-tab-pane>
        </el-tabs>
        <el-select
          v-if="activeFragment"
          v-model="activeFragment.language"
          size="small"
          style="width: 140px"
          filterable
          @change="onLanguageChange"
        >
          <el-option v-for="lang in sortedLanguages" :key="lang" :label="lang" :value="lang" />
        </el-select>
      </div>

      <div class="editor-body">
        <MonacoPane
          v-if="activeFragment"
          :key="selectedId + '-' + activeFragIdx"
          v-model="activeFragment.code"
          :language="activeFragment.language"
          @update:model-value="autoSave"
        />
      </div>
    </section>

    <section v-else class="snippet-editor snippet-empty-hint">
      <div class="empty-text">选择或新建一个片段</div>
    </section>

    <!-- 文件夹右键菜单 -->
    <teleport to="body">
      <div v-if="folderCtx.visible" class="ctx-menu" :style="{ left: folderCtx.x + 'px', top: folderCtx.y + 'px' }">
        <div class="ctx-item" @click="renameFolderPrompt">重命名</div>
        <div class="ctx-item" @click="addSubFolder">新建子文件夹</div>
        <div class="ctx-item danger" @click="deleteFolderConfirm">删除</div>
      </div>
    </teleport>
  </div>
</template>
<!-- PLACEHOLDER_SCRIPT -->
<script setup lang="ts">
import { ref, reactive, computed, onMounted, onBeforeUnmount, nextTick } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";
import { ElMessage, ElMessageBox } from "element-plus";
import MonacoPane from "./MonacoPane.vue";

interface Fragment { id?: number; label: string; language: string; code: string; sortOrder: number }
interface Snippet {
  id: number; title: string; description: string; folderId: number | null;
  isFavorite: boolean; createdAt: string; updatedAt: string;
  tags: string[]; fragments: Fragment[]; language?: string;
}
interface Folder { id: number; name: string; parentId: number | null; snippetCount: number; children?: Folder[] }
interface TagInfo { tag: string; count: number }

const defaultLanguages = [
  "javascript","typescript","python","java","go","rust","sql","html","css",
  "json","xml","yaml","bash","shell","markdown","plaintext","c","cpp","csharp",
  "php","ruby","swift","kotlin","scala","lua","r","dart","dockerfile","graphql","toml",
];

interface LangStat { language: string; count: number }
const langStats = ref<LangStat[]>([]);

// Languages sorted by usage count, unused ones appended in default order
const sortedLanguages = computed(() => {
  const usedSet = new Set(langStats.value.map(s => s.language));
  const used = langStats.value.map(s => s.language);
  const unused = defaultLanguages.filter(l => !usedSet.has(l));
  return [...used, ...unused];
});

// --- State ---
const navMode = ref<"all" | "favorite" | "folder" | "tag">("all");
const activeFolderId = ref<number | null>(null);
const activeTag = ref("");
const searchKeyword = ref("");
const filterLanguage = ref("");
const sortBy = ref("updated_at");
const snippetList = ref<Snippet[]>([]);
const selectedId = ref<number | null>(null);
const current = ref<Snippet | null>(null);
const folderList = ref<Folder[]>([]);
const tagList = ref<TagInfo[]>([]);
const activeFragIdx = ref("0");
const tagInputVisible = ref(false);
const tagInputValue = ref("");
const tagInputRef = ref<HTMLInputElement | null>(null);
const folderTreeRef = ref<InstanceType<any> | null>(null);
const renamingFragIdx = ref<number | null>(null);
const fragRenameValue = ref("");
let fragRenameInputRef: any = null;

let saveTimer: ReturnType<typeof setTimeout> | null = null;
let searchTimer: ReturnType<typeof setTimeout> | null = null;

const totalCount = computed(() => snippetList.value.length);
const favoriteCount = computed(() => snippetList.value.filter(s => s.isFavorite).length);

const activeFragment = computed(() => {
  if (!current.value) return null;
  const idx = parseInt(activeFragIdx.value, 10);
  return current.value.fragments[idx] ?? null;
});

// --- Folder tree builder ---
const folderTree = computed(() => {
  const map = new Map<number, Folder & { children: Folder[] }>();
  for (const f of folderList.value) {
    map.set(f.id, { ...f, children: [] });
  }
  const roots: Folder[] = [];
  for (const f of map.values()) {
    if (f.parentId && map.has(f.parentId)) {
      map.get(f.parentId)!.children!.push(f);
    } else {
      roots.push(f);
    }
  }
  return roots;
});

// --- Context menu ---
const folderCtx = reactive({ visible: false, x: 0, y: 0, folderId: 0, folderName: "" });
function closeFolderCtx() { folderCtx.visible = false; }
// PLACEHOLDER_SCRIPT_METHODS

// --- IPC helpers ---
async function ipc(channel: string, payload: Record<string, unknown> = {}) {
  return invokeToolByChannel(`tool:snippets:${channel}`, payload);
}

// --- Data loading ---
async function loadSnippets() {
  try {
    if (searchKeyword.value) {
      snippetList.value = (await ipc("search", { keyword: searchKeyword.value })) as Snippet[];
    } else {
      const params: Record<string, unknown> = { sort_by: sortBy.value };
      if (navMode.value === "favorite") params.is_favorite = true;
      if (navMode.value === "folder" && activeFolderId.value !== null) params.folder_id = activeFolderId.value;
      if (navMode.value === "tag" && activeTag.value) params.tag = activeTag.value;
      if (filterLanguage.value) params.language = filterLanguage.value;
      snippetList.value = (await ipc("list", params)) as Snippet[];
    }
  } catch (e: any) {
    ElMessage.error(e.message ?? "加载片段失败");
  }
}

async function loadFolders() {
  try {
    folderList.value = (await ipc("folder-list")) as Folder[];
  } catch { /* ignore */ }
}

async function loadTags() {
  try {
    tagList.value = (await ipc("tags")) as TagInfo[];
  } catch { /* ignore */ }
}

async function loadLangStats() {
  try {
    langStats.value = (await ipc("language-stats")) as LangStat[];
  } catch { /* ignore */ }
}

async function loadAll() {
  await Promise.all([loadSnippets(), loadFolders(), loadTags(), loadLangStats()]);
}

// --- Navigation ---
function setNav(mode: "all" | "favorite" | "tag", tag?: string) {
  navMode.value = mode;
  activeFolderId.value = null;
  activeTag.value = tag ?? "";
  if (folderTreeRef.value) folderTreeRef.value.setCurrentKey(null);
  loadSnippets();
}

function onFolderClick(data: Folder) {
  navMode.value = "folder";
  activeFolderId.value = data.id;
  activeTag.value = "";
  loadSnippets();
}

// --- Snippet selection ---
async function selectSnippet(id: number) {
  try {
    selectedId.value = id;
    const data = (await ipc("get", { id })) as Snippet;
    current.value = data;
    activeFragIdx.value = "0";
  } catch (e: any) {
    ElMessage.error(e.message ?? "加载片段失败");
  }
}

// --- CRUD ---
async function createSnippet() {
  try {
    const params: Record<string, unknown> = { title: "未命名片段" };
    if (navMode.value === "folder" && activeFolderId.value !== null) {
      params.folderId = activeFolderId.value;
    }
    const data = (await ipc("create", params)) as Snippet;
    await loadAll();
    selectSnippet(data.id);
  } catch (e: any) {
    ElMessage.error(e.message ?? "创建失败");
  }
}

function autoSave() {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(doSave, 800);
}

async function doSave() {
  if (!current.value) return;
  try {
    await ipc("update", {
      id: current.value.id,
      title: current.value.title,
      description: current.value.description,
      folderId: current.value.folderId,
      tags: current.value.tags,
      fragments: current.value.fragments.map((f, i) => ({
        label: f.label, language: f.language, code: f.code, sortOrder: i,
      })),
    });
    // Refresh list metadata without losing selection
    await Promise.all([loadSnippets(), loadTags(), loadLangStats()]);
  } catch { /* silent */ }
}

async function deleteSnippet() {
  if (!current.value) return;
  try {
    await ipc("delete", { id: current.value.id });
    current.value = null;
    selectedId.value = null;
    await loadAll();
  } catch (e: any) {
    ElMessage.error(e.message ?? "删除失败");
  }
}

async function toggleFav() {
  if (!current.value) return;
  try {
    const res = (await ipc("toggle-favorite", { id: current.value.id })) as { isFavorite: boolean };
    current.value.isFavorite = res.isFavorite;
    await loadSnippets();
  } catch { /* ignore */ }
}
// PLACEHOLDER_SCRIPT_REST

// --- Copy ---
function copyCode() {
  if (!activeFragment.value) return;
  navigator.clipboard.writeText(activeFragment.value.code).then(
    () => ElMessage.success("已复制"),
    () => ElMessage.error("复制失败"),
  );
}

// --- Tags ---
function removeTag(tag: string) {
  if (!current.value) return;
  current.value.tags = current.value.tags.filter(t => t !== tag);
  autoSave();
}
function showTagInput() {
  tagInputVisible.value = true;
  nextTick(() => { (tagInputRef.value as any)?.focus?.(); });
}
function confirmTag() {
  const v = tagInputValue.value.trim();
  if (v && current.value && !current.value.tags.includes(v)) {
    current.value.tags.push(v);
    autoSave();
  }
  tagInputVisible.value = false;
  tagInputValue.value = "";
}

// --- Fragment tabs ---
function onFragTabEdit(targetName: string | undefined, action: "add" | "remove") {
  if (!current.value) return;
  if (action === "add") {
    current.value.fragments.push({ label: `tab${current.value.fragments.length + 1}`, language: "plaintext", code: "", sortOrder: current.value.fragments.length });
    activeFragIdx.value = String(current.value.fragments.length - 1);
    autoSave();
  } else if (action === "remove" && targetName !== undefined) {
    const idx = parseInt(targetName, 10);
    if (current.value.fragments.length <= 1) return;
    current.value.fragments.splice(idx, 1);
    if (parseInt(activeFragIdx.value, 10) >= current.value.fragments.length) {
      activeFragIdx.value = String(current.value.fragments.length - 1);
    }
    autoSave();
  }
}

function startFragRename(idx: number) {
  if (!current.value) return;
  renamingFragIdx.value = idx;
  fragRenameValue.value = current.value.fragments[idx].label;
  nextTick(() => { fragRenameInputRef?.focus?.(); });
}

function confirmFragRename() {
  if (renamingFragIdx.value === null || !current.value) return;
  const v = fragRenameValue.value.trim();
  if (v) {
    current.value.fragments[renamingFragIdx.value].label = v;
    autoSave();
  }
  renamingFragIdx.value = null;
  fragRenameValue.value = "";
}

const langExtMap: Record<string, string> = {
  javascript: ".js", typescript: ".ts", python: ".py", java: ".java", go: ".go",
  rust: ".rs", sql: ".sql", html: ".html", css: ".css", json: ".json",
  xml: ".xml", yaml: ".yml", bash: ".sh", shell: ".sh", markdown: ".md",
  plaintext: ".txt", c: ".c", cpp: ".cpp", csharp: ".cs", php: ".php",
  ruby: ".rb", swift: ".swift", kotlin: ".kt", scala: ".scala", lua: ".lua",
  r: ".r", dart: ".dart", dockerfile: "", graphql: ".graphql", toml: ".toml",
  vue: ".vue", jsx: ".jsx", tsx: ".tsx", scss: ".scss", less: ".less",
  powershell: ".ps1", perl: ".pl",
};

function onLanguageChange(lang: string) {
  if (!current.value) return;
  const frag = activeFragment.value;
  if (!frag) return;
  const ext = langExtMap[lang];
  // Only auto-append if label has no extension (no dot) and ext is non-empty
  if (ext && !frag.label.includes(".")) {
    frag.label = frag.label + ext;
  }
  autoSave();
}

// --- Folder operations ---
async function createFolder() {
  try {
    const { value } = await ElMessageBox.prompt("文件夹名称", "新建文件夹", { inputValue: "新建文件夹", confirmButtonText: "创建", cancelButtonText: "取消" });
    if (value) {
      await ipc("folder-create", { name: value });
      await loadFolders();
    }
  } catch { /* cancelled */ }
}

function onFolderContext(ev: MouseEvent, data: Folder) {
  ev.preventDefault();
  folderCtx.visible = true;
  folderCtx.x = ev.clientX;
  folderCtx.y = ev.clientY;
  folderCtx.folderId = data.id;
  folderCtx.folderName = data.name;
}

async function renameFolderPrompt() {
  closeFolderCtx();
  try {
    const { value } = await ElMessageBox.prompt("新名称", "重命名文件夹", { inputValue: folderCtx.folderName, confirmButtonText: "确定", cancelButtonText: "取消" });
    if (value) {
      await ipc("folder-update", { id: folderCtx.folderId, name: value });
      await loadFolders();
    }
  } catch { /* cancelled */ }
}

async function addSubFolder() {
  closeFolderCtx();
  try {
    const { value } = await ElMessageBox.prompt("子文件夹名称", "新建子文件夹", { inputValue: "子文件夹", confirmButtonText: "创建", cancelButtonText: "取消" });
    if (value) {
      await ipc("folder-create", { name: value, parentId: folderCtx.folderId });
      await loadFolders();
    }
  } catch { /* cancelled */ }
}

async function deleteFolderConfirm() {
  closeFolderCtx();
  try {
    await ElMessageBox.confirm(`删除文件夹「${folderCtx.folderName}」？其中的片段不会被删除。`, "确认删除", { confirmButtonText: "删除", cancelButtonText: "取消", type: "warning" });
    await ipc("folder-delete", { id: folderCtx.folderId });
    if (activeFolderId.value === folderCtx.folderId) {
      navMode.value = "all";
      activeFolderId.value = null;
    }
    await loadAll();
  } catch { /* cancelled */ }
}

// --- Search debounce ---
function onSearchDebounced() {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(loadSnippets, 300);
}

// --- Time format ---
function formatTime(iso: string): string {
  if (!iso) return "";
  const d = new Date(iso.replace(" ", "T"));
  const now = new Date();
  if (d.toDateString() === now.toDateString()) {
    return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString("zh-CN", { month: "2-digit", day: "2-digit" });
}

// --- Lifecycle ---
onMounted(() => {
  loadAll();
  document.addEventListener("click", closeFolderCtx);
});
onBeforeUnmount(() => {
  if (saveTimer) clearTimeout(saveTimer);
  if (searchTimer) clearTimeout(searchTimer);
  document.removeEventListener("click", closeFolderCtx);
});
</script>
<!-- PLACEHOLDER_STYLE -->
<style scoped>
.snippet-panel {
  display: flex;
  height: 100%;
  gap: 0;
  overflow: hidden;
}

/* --- Left nav --- */
.snippet-nav {
  width: 200px;
  min-width: 200px;
  border-right: 1px solid var(--lc-border);
  overflow-y: auto;
  padding: 8px 0;
}
.nav-section { padding: 4px 0; }
.nav-section + .nav-section { border-top: 1px solid var(--lc-border); }
.nav-section-header {
  display: flex; justify-content: space-between; align-items: center;
  padding: 4px 12px; font-size: 12px; color: var(--lc-text-muted);
}
.nav-item {
  padding: 6px 12px; cursor: pointer; font-size: 13px;
  color: var(--lc-text-secondary); display: flex; justify-content: space-between;
}
.nav-item:hover { color: var(--lc-text); background: var(--lc-surface-2); }
.nav-item.active { color: var(--lc-accent-light); background: var(--lc-accent-dim); }
.nav-empty { padding: 6px 12px; font-size: 12px; color: var(--lc-text-muted); }
.count { font-size: 11px; color: var(--lc-text-muted); }
.folder-node { display: flex; gap: 4px; align-items: center; font-size: 13px; }
.tag-item { font-size: 12px; }

/* --- Middle list --- */
.snippet-list {
  width: 260px; min-width: 260px;
  border-right: 1px solid var(--lc-border);
  display: flex; flex-direction: column; overflow: hidden;
}
.list-toolbar { display: flex; padding: 8px; }
.list-sort { padding: 0 8px 6px; }
.list-items { flex: 1; overflow-y: auto; }
.snippet-card {
  padding: 8px 12px; cursor: pointer;
  border-bottom: 1px solid var(--lc-border-subtle);
}
.snippet-card:hover { background: var(--lc-surface-2); }
.snippet-card.active { background: var(--lc-accent-dim); }
.card-title {
  display: flex; justify-content: space-between; align-items: center;
  font-size: 13px; color: var(--lc-text);
}
.card-title .fav-icon { color: #e6a23c; font-size: 14px; }
.card-meta { display: flex; gap: 8px; margin-top: 4px; font-size: 11px; color: var(--lc-text-muted); }
.lang-badge {
  background: var(--lc-surface-3); padding: 1px 6px; border-radius: 3px; font-size: 10px;
}
.list-empty { padding: 24px; text-align: center; color: var(--lc-text-muted); font-size: 13px; }
.list-footer { padding: 8px; border-top: 1px solid var(--lc-border); }
/* PLACEHOLDER_STYLE_REST */

/* --- Right editor --- */
.snippet-editor {
  flex: 1; display: flex; flex-direction: column; overflow: hidden; padding: 12px;
}
.snippet-empty-hint {
  align-items: center; justify-content: center;
}
.empty-text { color: var(--lc-text-muted); font-size: 14px; }
.editor-header {
  display: flex; justify-content: space-between; align-items: center;
  margin-bottom: 8px; gap: 8px;
}
.editor-actions { display: flex; gap: 4px; flex-shrink: 0; }
.tag-editor { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; margin-bottom: 8px; }
.fragment-tabs {
  display: flex; align-items: center; gap: 8px; margin-bottom: 4px;
}
.fragment-tabs .el-tabs { flex: 1; }
.fragment-tabs :deep(.el-tabs__header) { margin-bottom: 0; }
.editor-body { flex: 1; min-height: 0; }

/* --- Context menu --- */
.ctx-menu {
  position: fixed; z-index: 9999;
  background: var(--lc-surface-1); border: 1px solid var(--lc-border);
  border-radius: 6px; padding: 4px 0; min-width: 120px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.3);
}
.ctx-item {
  padding: 6px 16px; font-size: 13px; cursor: pointer;
  color: var(--lc-text-secondary);
}
.ctx-item:hover { background: var(--lc-surface-2); color: var(--lc-text); }
.ctx-item.danger { color: #f56c6c; }
.ctx-item.danger:hover { background: rgba(245,108,108,0.1); }

/* el-tree overrides */
.snippet-nav :deep(.el-tree) { background: transparent; }
.snippet-nav :deep(.el-tree-node__content) { height: 28px; }
.snippet-nav :deep(.el-tree-node.is-current > .el-tree-node__content) {
  background: var(--lc-accent-dim); color: var(--lc-accent-light);
}
</style>
