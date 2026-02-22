<template>
  <div class="launcher-panel" @dragover.prevent @dragenter.prevent>
    <div class="launcher-groups">
      <div
        v-for="g in groupList"
        :key="g"
        class="group-item"
        :class="{ active: activeGroup === g }"
        @click="activeGroup = g"
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
          style="width: 240px;"
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
          暂无应用，请在设置中扫描或手动添加
        </div>

        <!-- Grid View -->
        <div v-else-if="viewType === 'grid'" class="launcher-grid">
          <div
            v-for="(entry, idx) in filteredEntries"
            :key="entry.id"
            class="grid-card"
            draggable="true"
            @click="launchApp(entry)"
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
          </div>
        </div>

        <!-- List View -->
        <div v-else class="launcher-list">
          <div
            v-for="(entry, idx) in filteredEntries"
            :key="entry.id"
            class="list-row"
            draggable="true"
            @click="launchApp(entry)"
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
import { invokeToolByChannel } from "../bridge/tauri";
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
}

interface ScanItem {
  name: string;
  exe_path: string;
  arguments: string;
  _exists: boolean;
}

const entries = ref<LauncherEntry[]>([]);
const customGroups = ref<string[]>([]);
const searchQuery = ref("");
const viewType = ref<"grid" | "list">("grid");
const activeGroup = ref("全部");

// Context menu
const ctxVisible = ref(false);
const ctxX = ref(0);
const ctxY = ref(0);
const ctxEntry = ref<LauncherEntry | null>(null);

// Scan dialog
const scanDialogVisible = ref(false);
const scanItems = ref<ScanItem[]>([]);
const scanSelection = ref<ScanItem[]>([]);
const scanSearch = ref("");
const scanTableRef = ref();

// Edit dialog
const editDialogVisible = ref(false);
const editForm = ref({ id: 0, name: "", group_name: "" });

// Settings dialog
const settingsDialogVisible = ref(false);
const settingsTab = ref("add");

// Drag state
const dragIdx = ref(-1);
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

const filteredEntries = computed(() => {
  let list = entries.value;
  if (activeGroup.value !== "全部") {
    list = list.filter((e) => (e.group_name || "未分组") === activeGroup.value);
  }
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((e) => e.name.toLowerCase().includes(q) || e.exe_path.toLowerCase().includes(q));
  }
  return list;
});

const filteredScanItems = computed(() => {
  const q = scanSearch.value.trim().toLowerCase();
  if (!q) return scanItems.value;
  return scanItems.value.filter((s) => s.name.toLowerCase().includes(q) || s.exe_path.toLowerCase().includes(q));
});

// Load
async function loadEntries() {
  try {
    const res = (await invokeToolByChannel("tool:launcher:list", {})) as { items: LauncherEntry[] };
    entries.value = res.items;
  } catch (e) {
    ElMessage.error(`加载失败：${(e as Error).message}`);
  }
}

async function loadGroups() {
  try {
    const res = (await invokeToolByChannel("tool:launcher:list-groups", {})) as { groups: string[] };
    customGroups.value = res.groups;
  } catch (e) {
    // ignore
  }
}

onMounted(() => { loadEntries(); loadGroups(); document.addEventListener("click", hideCtx); });
onBeforeUnmount(() => { document.removeEventListener("click", hideCtx); });

// Launch
async function launchApp(entry: LauncherEntry, admin = false) {
  if (justDragged.value) { justDragged.value = false; return; }
  try {
    await invokeToolByChannel("tool:launcher:launch", {
      exe_path: entry.exe_path,
      arguments: entry.arguments,
      admin,
    });
  } catch (e) {
    ElMessage.error(`启动失败：${(e as Error).message}`);
  }
}

// Scan
async function openScanDialog() {
  scanDialogVisible.value = true;
  scanSearch.value = "";
  scanSelection.value = [];
  try {
    const res = (await invokeToolByChannel("tool:launcher:scan", {})) as { items: ScanItem[] };
    const existingPaths = new Set(entries.value.map((e) => e.exe_path.toLowerCase()));
    scanItems.value = res.items.map((s) => ({
      ...s,
      _exists: existingPaths.has(s.exe_path.toLowerCase()),
    }));
  } catch (e) {
    ElMessage.error(`扫描失败：${(e as Error).message}`);
  }
}

function onScanSelectionChange(rows: ScanItem[]) {
  scanSelection.value = rows;
}

async function addScanned() {
  try {
    const items = scanSelection.value.map((s) => ({
      name: s.name,
      exe_path: s.exe_path,
      arguments: s.arguments,
    }));
    await invokeToolByChannel("tool:launcher:add", { items });
    ElMessage.success(`已添加 ${items.length} 个应用`);
    scanDialogVisible.value = false;
    await loadEntries();
  } catch (e) {
    ElMessage.error(`添加失败：${(e as Error).message}`);
  }
}

// Manual add
async function handleManualAddFile() {
  try {
    const filePath = await open({
      multiple: false,
      filters: [{ name: "可执行文件", extensions: ["exe"] }],
    });
    if (!filePath) return;
    await invokeToolByChannel("tool:launcher:add-manual", { exe_path: filePath });
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
    await invokeToolByChannel("tool:launcher:add-manual", { exe_path: dirPath });
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
  try {
    await invokeToolByChannel("tool:launcher:open-folder", { exe_path: ctxEntry.value.exe_path });
  } catch (e) {
    ElMessage.error(`打开目录失败：${(e as Error).message}`);
  }
  hideCtx();
}

function ctxEdit() {
  if (!ctxEntry.value) return;
  editForm.value = {
    id: ctxEntry.value.id,
    name: ctxEntry.value.name,
    group_name: ctxEntry.value.group_name,
  };
  editDialogVisible.value = true;
  hideCtx();
}

async function ctxRemove() {
  if (!ctxEntry.value) return;
  hideCtx();
  try {
    await ElMessageBox.confirm(`确定删除「${ctxEntry.value.name}」？`, "确认删除", { type: "warning" });
    await invokeToolByChannel("tool:launcher:remove", { id: ctxEntry.value.id });
    ElMessage.success("已删除");
    await loadEntries();
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`删除失败：${(e as Error).message}`);
  }
}

// Edit save
async function saveEdit() {
  try {
    await invokeToolByChannel("tool:launcher:update", {
      id: editForm.value.id,
      name: editForm.value.name,
      group_name: editForm.value.group_name,
    });
    editDialogVisible.value = false;
    ElMessage.success("已保存");
    await loadEntries();
  } catch (e) {
    ElMessage.error(`保存失败：${(e as Error).message}`);
  }
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
  // visual feedback could be added here
  void idx;
}

function onDragEnd() {
  dragIdx.value = -1;
}

async function onDrop(targetIdx: number) {
  if (dragIdx.value < 0 || dragIdx.value === targetIdx) return;
  const list = [...filteredEntries.value];
  const [moved] = list.splice(dragIdx.value, 1);
  list.splice(targetIdx, 0, moved);
  dragIdx.value = -1;

  const orders = list.map((e, i) => ({ id: e.id, sort_order: i }));
  try {
    await invokeToolByChannel("tool:launcher:reorder", { orders });
    await loadEntries();
  } catch (e) {
    ElMessage.error(`排序失败：${(e as Error).message}`);
  }
}

// Group management
const groupTableData = computed(() =>
  userGroups.value.map((g) => ({
    name: g,
    count: entries.value.filter((e) => e.group_name === g).length,
  }))
);

async function createGroup() {
  try {
    const { value: newName } = await ElMessageBox.prompt("输入分组名称", "新建分组", {
      confirmButtonText: "确定",
      cancelButtonText: "取消",
      inputValidator: (v) => {
        if (!v || !v.trim()) return "分组名称不能为空";
        if (userGroups.value.includes(v.trim())) return "分组名称已存在";
        return true;
      },
    });
    if (!newName) return;
    await invokeToolByChannel("tool:launcher:create-group", { name: newName.trim() });
    ElMessage.success(`分组「${newName.trim()}」已创建`);
    await loadGroups();
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`创建分组失败：${(e as Error).message}`);
  }
}

async function startRenameGroup(oldName: string) {
  try {
    const { value: newName } = await ElMessageBox.prompt("输入新的分组名称", "重命名分组", {
      inputValue: oldName,
      confirmButtonText: "确定",
      cancelButtonText: "取消",
      inputValidator: (v) => {
        if (!v || !v.trim()) return "分组名称不能为空";
        if (v.trim() !== oldName && userGroups.value.includes(v.trim())) return "分组名称已存在";
        return true;
      },
    });
    if (!newName || newName.trim() === oldName) return;
    await invokeToolByChannel("tool:launcher:rename-group", { old_name: oldName, new_name: newName.trim() });
    ElMessage.success("分组已重命名");
    await Promise.all([loadEntries(), loadGroups()]);
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`重命名失败：${(e as Error).message}`);
  }
}

async function deleteGroup(groupName: string) {
  try {
    await ElMessageBox.confirm(
      `删除分组「${groupName}」后，其中的应用将移至"未分组"。确定继续？`,
      "确认删除分组",
      { type: "warning" },
    );
    await invokeToolByChannel("tool:launcher:delete-group", { name: groupName });
    ElMessage.success("分组已删除");
    await Promise.all([loadEntries(), loadGroups()]);
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`删除分组失败：${(e as Error).message}`);
  }
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
</style>

