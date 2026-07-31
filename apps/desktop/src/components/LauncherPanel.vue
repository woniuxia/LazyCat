<template>
  <div class="launcher-panel" @dragover.prevent @dragenter.prevent>
    <div class="launcher-groups">
      <div
        v-for="g in groupList"
        :key="g"
        class="group-item"
        :class="{ active: activeGroup === g }"
        role="button"
        tabindex="0"
        :aria-pressed="activeGroup === g"
        @click="activeGroup = g"
        @keydown.enter.prevent="activeGroup = g"
        @keydown.space.prevent="activeGroup = g"
      >
        {{ g }} ({{ groupCount(g) }})
      </div>
    </div>

    <div class="launcher-main">
      <div class="launcher-toolbar">
        <el-input
          v-model="searchQuery"
          placeholder="搜索应用..."
          clearable
          style="flex: 1; min-width: 160px; max-width: 320px;"
        />
        <div style="display: flex; gap: 8px; align-items: center;">
          <el-button-group>
            <el-button :type="viewType === 'grid' ? 'primary' : ''" size="small" @click="viewType = 'grid'">网格</el-button>
            <el-button :type="viewType === 'list' ? 'primary' : ''" size="small" @click="viewType = 'list'">列表</el-button>
          </el-button-group>
          <el-button @click="settingsDialogVisible = true">设置</el-button>
        </div>
      </div>

      <div class="launcher-content">
        <div v-if="filteredEntries.length === 0" class="launcher-empty">
          <div class="empty-inner">
            <p v-if="debouncedQuery">无匹配结果</p>
            <template v-else>
              <p>暂无应用</p>
              <div class="empty-actions">
                <el-button type="primary" @click="openScanDialog">扫描添加</el-button>
                <el-button @click="handleManualAddFile">添加程序</el-button>
              </div>
            </template>
          </div>
        </div>

        <!-- Grid View -->
        <div v-else-if="viewType === 'grid'" class="launcher-grid">
          <div
            v-for="(entry, idx) in filteredEntries"
            :key="entry.id"
            class="grid-card"
            :class="{ 'drag-over': dragOverIdx === idx, 'is-missing': !entry.path_exists }"
            draggable="true"
            role="button"
            tabindex="0"
            :aria-label="launcherEntryAriaLabel(entry)"
            :title="launcherEntryTitle(entry)"
            @click="launchApp(entry)"
            @keydown.enter.prevent="launchApp(entry)"
            @keydown.space.prevent="launchApp(entry)"
            @keydown.alt.up.prevent="moveEntryByKeyboard(entry, -1)"
            @keydown.alt.down.prevent="moveEntryByKeyboard(entry, 1)"
            @dragstart="onDragStart(idx, $event)"
            @dragover.prevent="onDragOver(idx)"
            @drop="onDrop(idx)"
            @dragend="onDragEnd"
            @contextmenu.prevent="onContextMenu(entry, $event)"
          >
            <img
              v-if="entry.icon_base64"
              :src="'data:image/png;base64,' + entry.icon_base64"
              class="app-icon"
            />
            <img v-else :src="defaultIcon" class="app-icon" />
            <span class="app-name" :title="entry.name">{{ entry.name }}</span>
            <span v-if="!entry.path_exists" class="entry-warning">路径失效</span>
          </div>
        </div>

        <!-- List View -->
        <div v-else class="launcher-list">
          <div
            v-for="(entry, idx) in filteredEntries"
            :key="entry.id"
            class="list-row"
            :class="{ 'drag-over': dragOverIdx === idx, 'is-missing': !entry.path_exists }"
            draggable="true"
            role="button"
            tabindex="0"
            :aria-label="launcherEntryAriaLabel(entry)"
            :title="launcherEntryTitle(entry)"
            @click="launchApp(entry)"
            @keydown.enter.prevent="launchApp(entry)"
            @keydown.space.prevent="launchApp(entry)"
            @keydown.alt.up.prevent="moveEntryByKeyboard(entry, -1)"
            @keydown.alt.down.prevent="moveEntryByKeyboard(entry, 1)"
            @dragstart="onDragStart(idx, $event)"
            @dragover.prevent="onDragOver(idx)"
            @drop="onDrop(idx)"
            @dragend="onDragEnd"
            @contextmenu.prevent="onContextMenu(entry, $event)"
          >
            <img
              v-if="entry.icon_base64"
              :src="'data:image/png;base64,' + entry.icon_base64"
              class="list-icon"
            />
            <img v-else :src="defaultIcon" class="list-icon" />
            <span class="list-name">{{ entry.name }}</span>
            <span class="list-path">{{ entry.exe_path }}</span>
            <el-tag v-if="!entry.path_exists" size="small" type="danger">路径失效</el-tag>
          </div>
        </div>
      </div>
    </div>

    <!-- Context Menu -->
    <teleport to="body">
      <div
        v-if="ctxVisible"
        class="launcher-ctx-menu"
        :style="{ left: ctxX + 'px', top: ctxY + 'px' }"
      >
        <div class="ctx-item" @click="ctxLaunch(false)">启动</div>
        <div class="ctx-item" @click="ctxLaunch(true)">以管理员身份运行</div>
        <div class="ctx-item" @click="ctxOpenFolder">打开所在目录</div>
        <div class="ctx-item" @click="ctxEdit">编辑</div>
        <div class="ctx-item ctx-danger" @click="ctxRemove">删除</div>
      </div>
    </teleport>
    <!-- Scan Dialog -->
    <el-dialog v-model="scanDialogVisible" title="扫描快捷方式" width="680px" :close-on-click-modal="false">
      <div style="margin-bottom: 12px;">
        <el-input v-model="scanSearch" placeholder="过滤扫描结果..." clearable style="width: 260px;" />
      </div>
      <el-table
        :data="filteredScanItems"
        height="400"
        v-loading="scanLoading"
        @selection-change="onScanSelectionChange"
        ref="scanTableRef"
      >
        <el-table-column type="selection" width="45" :selectable="(row: ScanItem) => !row._exists" />
        <el-table-column prop="name" label="名称" min-width="180" />
        <el-table-column prop="exe_path" label="路径" min-width="300" show-overflow-tooltip />
        <el-table-column label="状态" width="80">
          <template #default="{ row }">
            <el-tag v-if="row._exists" type="info" size="small">已添加</el-tag>
          </template>
        </el-table-column>
      </el-table>
      <template #footer>
        <el-button @click="scanDialogVisible = false">取消</el-button>
        <el-button type="primary" :disabled="scanSelection.length === 0" @click="addScanned">
          添加选中 ({{ scanSelection.length }})
        </el-button>
      </template>
    </el-dialog>

    <!-- Edit Dialog -->
    <el-dialog v-model="editDialogVisible" title="编辑应用" width="440px">
      <el-form label-width="80px">
        <el-form-item label="名称">
          <el-input v-model="editForm.name" />
        </el-form-item>
        <el-form-item label="分组">
          <el-select v-model="editForm.group_name" filterable allow-create default-first-option style="width: 100%;">
            <el-option v-for="g in userGroups" :key="g" :label="g" :value="g" />
          </el-select>
        </el-form-item>
        <el-form-item label="程序路径">
          <div class="edit-path-row">
            <el-input v-model="editForm.exe_path" placeholder="可执行文件或文件夹路径" />
            <el-button @click="chooseEditPath">选择</el-button>
          </div>
        </el-form-item>
        <el-form-item label="启动参数">
          <el-input
            v-model="editForm.arguments"
            type="textarea"
            :rows="3"
            placeholder='例如 --profile "work" --safe-mode'
          />
          <div class="form-hint">参数会原样保存；普通启动会按 Windows 命令行引号规则拆分。</div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="saveEdit">保存</el-button>
      </template>
    </el-dialog>

    <!-- Settings Dialog -->
    <el-dialog v-model="settingsDialogVisible" title="快捷启动设置" width="680px" :close-on-click-modal="false">
      <el-tabs v-model="settingsTab">
        <el-tab-pane label="添加应用" name="add">
          <div style="display: flex; flex-direction: column; gap: 16px;">
            <div>
              <h4 style="margin: 0 0 8px;">扫描快捷方式</h4>
              <p style="margin: 0 0 8px; color: var(--el-text-color-secondary); font-size: 13px;">
                自动扫描开始菜单和桌面的快捷方式
              </p>
              <el-button type="primary" @click="openScanDialog">扫描添加</el-button>
            </div>
            <el-divider style="margin: 4px 0;" />
            <div>
              <h4 style="margin: 0 0 8px;">手动添加</h4>
              <p style="margin: 0 0 8px; color: var(--el-text-color-secondary); font-size: 13px;">
                选择可执行文件或文件夹添加到启动列表
              </p>
              <div style="display: flex; gap: 8px;">
                <el-button @click="handleManualAddFile">添加程序</el-button>
                <el-button @click="handleManualAddFolder">添加文件夹</el-button>
              </div>
            </div>
          </div>
        </el-tab-pane>
        <el-tab-pane label="分组管理" name="groups">
          <div style="margin-bottom: 12px;">
            <el-button type="primary" size="small" @click="createGroup">新建分组</el-button>
          </div>
          <div v-if="userGroups.length === 0" style="color: var(--el-text-color-secondary); font-size: 13px; padding: 20px 0; text-align: center;">
            暂无自定义分组
          </div>
          <el-table v-else :data="groupTableData" style="width: 100%;">
            <el-table-column prop="name" label="分组名称" min-width="200" />
            <el-table-column prop="count" label="应用数量" width="100" align="center" />
            <el-table-column label="操作" width="180" align="center">
              <template #default="{ row }">
                <el-button size="small" text type="primary" @click="startRenameGroup(row.name)">重命名</el-button>
                <el-button size="small" text type="danger" @click="deleteGroup(row.name)">删除分组</el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-tab-pane>
      </el-tabs>
    </el-dialog>
  </div>
</template>
<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { useListSearch } from "../composables/useListSearch";
import { useToolInvoke } from "../composables/useToolInvoke";
import defaultIcon from "../assets/icon.png";

interface LauncherEntry {
  id: number;
  name: string;
  exe_path: string;
  arguments: string;
  icon_base64: string;
  group_name: string;
  sort_order: number;
  launch_count: number;
  path_exists: boolean;
}

interface ScanItem {
  name: string;
  exe_path: string;
  arguments: string;
  _exists: boolean;
}

const entries = ref<LauncherEntry[]>([]);
const customGroups = ref<string[]>([]);
const { invokeWithLoading, invokeSilent } = useToolInvoke();
const viewType = ref<"grid" | "list">("grid");
const activeGroup = ref("全部");

// Context menu
const ctxVisible = ref(false);
const ctxX = ref(0);
const ctxY = ref(0);
const ctxEntry = ref<LauncherEntry | null>(null);

// Scan dialog
const scanDialogVisible = ref(false);
const scanLoading = ref(false);
const scanItems = ref<ScanItem[]>([]);
const scanSelection = ref<ScanItem[]>([]);
const scanSearch = ref("");
const scanTableRef = ref();

// Edit dialog
const editDialogVisible = ref(false);
const editForm = ref({ id: 0, name: "", group_name: "", exe_path: "", arguments: "" });

// Settings dialog
const settingsDialogVisible = ref(false);
const settingsTab = ref("add");

// Drag state
const dragIdx = ref(-1);
const dragOverIdx = ref(-1);
const justDragged = ref(false);

// Computed
const groupList = computed(() => {
  const groups = new Set<string>();
  // Add custom groups first
  customGroups.value.forEach((g) => groups.add(g));
  // Add groups from entries
  entries.value.forEach((e) => groups.add(e.group_name || "未分组"));
  return ["全部", ...Array.from(groups).sort()];
});

const userGroups = computed(() => {
  const groups = new Set<string>();
  customGroups.value.forEach((g) => groups.add(g));
  entries.value.map((e) => e.group_name).filter(Boolean).forEach((g) => groups.add(g));
  return Array.from(groups).sort();
});

function groupCount(g: string): number {
  if (g === "全部") return entries.value.length;
  return entries.value.filter((e) => (e.group_name || "未分组") === g).length;
}

const groupFilteredEntries = computed(() => {
  if (activeGroup.value === "全部") return entries.value;
  return entries.value.filter((e) => (e.group_name || "未分组") === activeGroup.value);
});

function matchesLauncherEntry(entry: LauncherEntry, keyword: string) {
  const q = keyword.toLowerCase();
  return entry.name.toLowerCase().includes(q) || entry.exe_path.toLowerCase().includes(q);
}

const {
  keyword: searchQuery,
  debouncedKeyword: debouncedQuery,
  filtered: filteredEntries,
} = useListSearch(() => groupFilteredEntries.value, matchesLauncherEntry);

const filteredScanItems = computed(() => {
  const q = scanSearch.value.trim().toLowerCase();
  if (!q) return scanItems.value;
  return scanItems.value.filter((s) => s.name.toLowerCase().includes(q) || s.exe_path.toLowerCase().includes(q));
});

// Load
async function loadEntries() {
  const res = await invokeWithLoading<{ items: LauncherEntry[] }>(
    "tool:launcher:list",
    {},
    { errorPrefix: "加载失败：" },
  );
  if (res) entries.value = res.items;
}

async function loadGroups() {
  // 分组列表用于补全 UI，加载失败不阻断启动器主体使用。
  const res = await invokeSilent<{ groups: string[] }>("tool:launcher:list-groups", {});
  if (res) customGroups.value = res.groups;
}

onMounted(() => { void loadEntries(); void loadGroups(); document.addEventListener("click", hideCtx); });
onBeforeUnmount(() => { document.removeEventListener("click", hideCtx); });

// Launch
async function launchApp(entry: LauncherEntry, admin = false) {
  if (justDragged.value) { justDragged.value = false; return; }
  if (!entry.path_exists) {
    ElMessage.warning("程序路径已失效，请右键编辑后重新选择");
    return;
  }
  const launched = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:launch",
    {
      exe_path: entry.exe_path,
      arguments: entry.arguments,
      admin,
    },
    { errorPrefix: "启动失败：" },
  );
  if (!launched) return;
  ElMessage.success({ message: '已启动', duration: 1500 });
}

// Scan
async function openScanDialog() {
  scanDialogVisible.value = true;
  scanSearch.value = "";
  scanSelection.value = [];
  scanLoading.value = true;
  try {
    const res = await invokeWithLoading<{ items: ScanItem[] }>(
      "tool:launcher:scan",
      {},
      { errorPrefix: "扫描失败：" },
    );
    if (!res) return;
    const existingPaths = new Set(entries.value.map((e) => e.exe_path.toLowerCase()));
    scanItems.value = res.items.map((s) => ({
      ...s,
      _exists: existingPaths.has(s.exe_path.toLowerCase()),
    }));
  } finally {
    scanLoading.value = false;
  }
}

function onScanSelectionChange(rows: ScanItem[]) {
  scanSelection.value = rows;
}

async function addScanned() {
  const items = scanSelection.value.map((s) => ({
    name: s.name,
    exe_path: s.exe_path,
    arguments: s.arguments,
  }));
  const added = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:add",
    { items },
    { errorPrefix: "添加失败：" },
  );
  if (!added) return;
  ElMessage.success(`已添加 ${items.length} 个应用`);
  scanDialogVisible.value = false;
  await loadEntries();
}

// Manual add
async function handleManualAddFile() {
  try {
    const filePath = await open({
      multiple: false,
      filters: [{ name: "可执行文件", extensions: ["exe"] }],
    });
    if (!filePath) return;
    const added = await invokeWithLoading<Record<string, unknown>>(
      "tool:launcher:add-manual",
      { exe_path: filePath },
      { errorPrefix: "添加失败：" },
    );
    if (!added) return;
    ElMessage.success("已添加");
    await loadEntries();
  } catch (e) {
    ElMessage.error(`添加失败：${(e as Error).message}`);
  }
}

async function handleManualAddFolder() {
  try {
    const dirPath = await open({
      directory: true,
      multiple: false,
      title: "选择文件夹",
    });
    if (!dirPath) return;
    const added = await invokeWithLoading<Record<string, unknown>>(
      "tool:launcher:add-manual",
      { exe_path: dirPath },
      { errorPrefix: "添加失败：" },
    );
    if (!added) return;
    ElMessage.success("已添加");
    await loadEntries();
  } catch (e) {
    ElMessage.error(`添加失败：${(e as Error).message}`);
  }
}
// Context menu
function onContextMenu(entry: LauncherEntry, e: MouseEvent) {
  ctxEntry.value = entry;
  ctxX.value = e.clientX;
  ctxY.value = e.clientY;
  ctxVisible.value = true;
}

function hideCtx() { ctxVisible.value = false; }

function ctxLaunch(admin: boolean) {
  if (ctxEntry.value) launchApp(ctxEntry.value, admin);
  hideCtx();
}

async function ctxOpenFolder() {
  if (!ctxEntry.value) return;
  await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:open-folder",
    { exe_path: ctxEntry.value.exe_path },
    { errorPrefix: "打开目录失败：" },
  );
  hideCtx();
}

function ctxEdit() {
  if (!ctxEntry.value) return;
  editForm.value = {
    id: ctxEntry.value.id,
    name: ctxEntry.value.name,
    group_name: ctxEntry.value.group_name,
    exe_path: ctxEntry.value.exe_path,
    arguments: ctxEntry.value.arguments,
  };
  editDialogVisible.value = true;
  hideCtx();
}

async function chooseEditPath() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "可执行文件", extensions: ["exe"] }],
    });
    if (typeof selected === "string") editForm.value.exe_path = selected;
  } catch (error) {
    ElMessage.error(`选择路径失败：${(error as Error).message}`);
  }
}

async function ctxRemove() {
  if (!ctxEntry.value) return;
  const entry = ctxEntry.value;
  hideCtx();
  try {
    await ElMessageBox.confirm(`确定删除「${entry.name}」？`, "确认删除", { type: "warning" });
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`删除失败：${(e as Error).message}`);
    return;
  }
  const removed = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:remove",
    { id: entry.id },
    { errorPrefix: "删除失败：" },
  );
  if (!removed) return;
  ElMessage.success("已删除");
  await loadEntries();
}

// Edit save
async function saveEdit() {
  const name = editForm.value.name.trim();
  const exePath = editForm.value.exe_path.trim();
  if (!name) {
    ElMessage.warning("名称不能为空");
    return;
  }
  if (!exePath) {
    ElMessage.warning("程序路径不能为空");
    return;
  }
  const saved = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:update",
    {
      id: editForm.value.id,
      name,
      group_name: editForm.value.group_name.trim(),
      exe_path: exePath,
      arguments: editForm.value.arguments,
    },
    { errorPrefix: "保存失败：" },
  );
  if (!saved) return;
  editDialogVisible.value = false;
  ElMessage.success("已保存");
  await loadEntries();
}

function launcherEntryAriaLabel(entry: LauncherEntry): string {
  const status = entry.path_exists ? "" : "，路径已失效";
  return `启动 ${entry.name}${status}。按 Alt 加上下方向键调整顺序`;
}

function launcherEntryTitle(entry: LauncherEntry): string {
  const args = entry.arguments.trim() ? `\n参数：${entry.arguments}` : "";
  const status = entry.path_exists ? "" : "\n路径已失效，请右键编辑";
  return `${entry.exe_path}${args}${status}\nAlt+↑/↓ 调整顺序`;
}

async function moveEntryByKeyboard(entry: LauncherEntry, direction: -1 | 1) {
  const list = [...filteredEntries.value];
  const currentIndex = list.findIndex((item) => item.id === entry.id);
  const targetIndex = currentIndex + direction;
  if (currentIndex < 0 || targetIndex < 0 || targetIndex >= list.length) return;
  const [moved] = list.splice(currentIndex, 1);
  list.splice(targetIndex, 0, moved);
  const reordered = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:reorder",
    { orders: list.map((item, index) => ({ id: item.id, sort_order: index })) },
    { errorPrefix: "排序失败：" },
  );
  if (!reordered) return;
  await loadEntries();
}

// Drag & drop reorder
function onDragStart(idx: number, e: DragEvent) {
  dragIdx.value = idx;
  justDragged.value = true;
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(idx));
  }
}

function onDragOver(idx: number) {
  dragOverIdx.value = idx;
}

function onDragEnd() {
  dragIdx.value = -1;
  dragOverIdx.value = -1;
}

async function onDrop(targetIdx: number) {
  dragOverIdx.value = -1;
  if (dragIdx.value < 0 || dragIdx.value === targetIdx) return;
  const list = [...filteredEntries.value];
  const [moved] = list.splice(dragIdx.value, 1);
  list.splice(targetIdx, 0, moved);
  dragIdx.value = -1;

  const orders = list.map((e, i) => ({ id: e.id, sort_order: i }));
  const reordered = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:reorder",
    { orders },
    { errorPrefix: "排序失败：" },
  );
  if (!reordered) return;
  await loadEntries();
}

// Group management
const groupTableData = computed(() =>
  userGroups.value.map((g) => ({
    name: g,
    count: entries.value.filter((e) => e.group_name === g).length,
  }))
);

async function createGroup() {
  let newName: string;
  try {
    const result = await ElMessageBox.prompt("输入分组名称", "新建分组", {
      confirmButtonText: "确定",
      cancelButtonText: "取消",
      inputValidator: (v) => {
        if (!v || !v.trim()) return "分组名称不能为空";
        if (userGroups.value.includes(v.trim())) return "分组名称已存在";
        return true;
      },
    });
    newName = String(result.value).trim();
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`创建分组失败：${(e as Error).message}`);
    return;
  }
  if (!newName) return;
  const created = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:create-group",
    { name: newName },
    { errorPrefix: "创建分组失败：" },
  );
  if (!created) return;
  ElMessage.success(`分组「${newName}」已创建`);
  await loadGroups();
}

async function startRenameGroup(oldName: string) {
  let newName: string;
  try {
    const result = await ElMessageBox.prompt("输入新的分组名称", "重命名分组", {
      inputValue: oldName,
      confirmButtonText: "确定",
      cancelButtonText: "取消",
      inputValidator: (v) => {
        if (!v || !v.trim()) return "分组名称不能为空";
        if (v.trim() !== oldName && userGroups.value.includes(v.trim())) return "分组名称已存在";
        return true;
      },
    });
    newName = String(result.value).trim();
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`重命名失败：${(e as Error).message}`);
    return;
  }
  if (!newName || newName === oldName) return;
  const renamed = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:rename-group",
    { old_name: oldName, new_name: newName },
    { errorPrefix: "重命名失败：" },
  );
  if (!renamed) return;
  ElMessage.success("分组已重命名");
  await Promise.all([loadEntries(), loadGroups()]);
}

async function deleteGroup(groupName: string) {
  try {
    await ElMessageBox.confirm(
      `删除分组「${groupName}」后，其中的应用将移至"未分组"。确定继续？`,
      "确认删除分组",
      { type: "warning" },
    );
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`删除分组失败：${(e as Error).message}`);
    return;
  }
  const deleted = await invokeWithLoading<Record<string, unknown>>(
    "tool:launcher:delete-group",
    { name: groupName },
    { errorPrefix: "删除分组失败：" },
  );
  if (!deleted) return;
  ElMessage.success("分组已删除");
  await Promise.all([loadEntries(), loadGroups()]);
}
</script>
<style scoped>
.launcher-panel {
  display: flex;
  height: 100%;
}
.launcher-groups {
  width: 160px;
  flex-shrink: 0;
  overflow-y: auto;
  border-right: 1px solid var(--lc-border-subtle);
  padding: 8px 0;
  background: var(--lc-surface-1);
}
.group-item {
  padding: 8px 16px;
  cursor: pointer;
  font-size: 13px;
  color: var(--lc-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: background 0.15s, color 0.15s;
}
.group-item:hover {
  background: var(--lc-accent-dim);
}
.group-item.active {
  background: var(--lc-accent-dim);
  color: var(--lc-accent);
  font-weight: 600;
  border-left: 3px solid var(--lc-accent);
  padding-left: 13px;
}
.launcher-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.launcher-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
  padding: 12px 16px;
  border-bottom: 1px solid var(--lc-border-subtle);
}
.launcher-content {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
  padding: 16px;
}
.launcher-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--lc-text-secondary);
  font-size: 14px;
}
.empty-inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}
.empty-inner p {
  margin: 0;
}
.empty-actions {
  display: flex;
  gap: 8px;
}

/* Grid */
.launcher-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
  gap: 12px;
  padding: 4px;
}
.grid-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 16px 8px;
  border-radius: var(--lc-radius-md);
  cursor: pointer;
  transition: background 0.15s, transform 0.1s;
}
.grid-card:hover {
  background: var(--lc-accent-dim);
}
.group-item:focus-visible {
  outline: 2px solid var(--lc-accent);
  outline-offset: -2px;
}
.grid-card:focus-visible,
.list-row:focus-visible {
  outline: 2px solid var(--lc-accent);
  outline-offset: 2px;
}
.grid-card.is-missing,
.list-row.is-missing {
  border: 1px dashed color-mix(in srgb, var(--el-color-danger) 55%, transparent);
  background: color-mix(in srgb, var(--el-color-danger) 5%, transparent);
}
.grid-card:active {
  transform: scale(0.96);
}
.app-icon {
  width: 48px;
  height: 48px;
  object-fit: contain;
}
.app-name {
  font-size: 12px;
  text-align: center;
  max-width: 90px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lc-text);
}
.entry-warning {
  color: var(--el-color-danger);
  font-size: 11px;
  line-height: 1;
}

/* List */
.launcher-list {
  display: flex;
  flex-direction: column;
}
.list-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  cursor: pointer;
  border-radius: var(--lc-radius-sm);
  transition: background 0.15s;
}
.list-row:hover {
  background: var(--lc-accent-dim);
}
.list-row:active {
  background: var(--lc-accent-dim);
  transform: scale(0.995);
}
.edit-path-row {
  display: flex;
  width: 100%;
  gap: 8px;
}
.form-hint {
  margin-top: 4px;
  color: var(--lc-text-muted);
  font-size: 12px;
  line-height: 1.45;
}
.list-icon {
  width: 32px;
  height: 32px;
  object-fit: contain;
  flex-shrink: 0;
}
.list-name {
  font-size: 14px;
  min-width: 140px;
  color: var(--lc-text);
}
.list-path {
  font-size: 12px;
  color: var(--lc-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Context menu */
.launcher-ctx-menu {
  position: fixed;
  z-index: 9999;
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  padding: 4px 0;
  min-width: 160px;
  box-shadow: var(--lc-shadow-lg);
}
.ctx-item {
  padding: 8px 16px;
  font-size: 13px;
  cursor: pointer;
  color: var(--lc-text);
  transition: background 0.15s;
}
.ctx-item:hover {
  background: var(--lc-accent-dim);
}
.ctx-danger {
  color: var(--lc-danger);
}
.ctx-danger:hover {
  background: color-mix(in srgb, var(--lc-danger) 10%, transparent);
}
.grid-card.drag-over {
  background: var(--lc-accent-dim);
  box-shadow: inset 3px 0 0 var(--lc-accent);
}
.list-row.drag-over {
  background: var(--lc-accent-dim);
  box-shadow: inset 3px 0 0 var(--lc-accent);
}
</style>
