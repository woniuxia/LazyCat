<template>
  <div class="hosts-panel">
    <!-- 管理员权限提示 -->
    <div v-if="adminChecked && !canWrite" class="hosts-admin-banner">
      <el-icon><InfoFilled /></el-icon>
      <span>当前非管理员模式。激活或恢复 hosts 时将弹出 UAC 提权确认窗口。</span>
    </div>

    <div class="hosts-layout">
      <!-- 左侧：配置列表 -->
      <aside class="hosts-sidebar">
        <div class="hosts-search">
          <svg class="hosts-search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.3-4.3" />
          </svg>
          <input
            v-model="searchKeyword"
            type="text"
            placeholder="搜索配置..."
            class="hosts-search-input"
          />
          <button v-if="searchKeyword" class="hosts-search-clear" @click="searchKeyword = ''">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M18 6 6 18" />
              <path d="m6 6 12 12" />
            </svg>
          </button>
        </div>

        <div class="hosts-list-header">
          <span class="hosts-list-title">配置列表</span>
          <span class="hosts-list-count">{{ filteredProfiles.length }} 个</span>
        </div>

        <div ref="profileListRef" class="hosts-profile-list" :class="{ 'is-loading': listLoading }">
          <div
            v-for="(profile, index) in filteredProfiles"
            :key="profile.id"
            class="hosts-profile-card"
            :class="{
              'is-active': hostsName === profile.name,
              'is-enabled': profile.enabled
            }"
            :style="{ animationDelay: `${index * 40}ms` }"
            @click="pickHosts(profile)"
            @contextmenu.prevent="onCardContextMenu($event, profile)"
          >
            <div class="hosts-profile-status">
              <span
                v-if="profile.enabled"
                class="hosts-status-indicator is-active"
                title="当前激活"
              />
              <span v-else class="hosts-status-indicator" />
            </div>
            <div class="hosts-profile-info">
              <div class="hosts-profile-name">{{ profile.name }}</div>
              <div class="hosts-profile-meta">
                <span>{{ getEntryCount(profile.content) }} 条映射</span>
                <span class="hosts-profile-dot" />
                <span>{{ formatRelativeTime(profile.updatedAt) }}</span>
              </div>
            </div>
            <el-icon class="hosts-drag-handle"><Rank /></el-icon>
          </div>

          <el-empty
            v-if="filteredProfiles.length === 0 && !listLoading"
            description="暂无配置"
            :image-size="60"
          />
        </div>

        <div class="hosts-sidebar-actions">
          <el-button type="primary" :icon="Plus" @click="createNewConfig">新建配置</el-button>
        </div>
      </aside>

      <!-- 右侧：编辑器 -->
      <main class="hosts-editor-area">
        <div class="hosts-editor-header">
          <div class="hosts-editor-title">
            <span>配置名称</span>
            <el-input
              v-model="hostsName"
              placeholder="输入配置名称，如：local-dev"
              class="hosts-name-input"
            />
          </div>
          <div class="hosts-editor-actions">
            <el-dropdown @command="handleMoreAction">
              <el-button :icon="MoreFilled" circle />
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item command="clone" :icon="CopyDocument">克隆配置</el-dropdown-item>
                  <el-dropdown-item command="readSystem" :icon="Reading">读取系统 hosts</el-dropdown-item>
                  <el-dropdown-item command="clear" :icon="Delete">清空编辑器</el-dropdown-item>
                  <el-dropdown-item divided command="delete" :icon="DeleteFilled" class="hosts-delete-item">
                    删除配置
                  </el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>
        </div>

        <div class="hosts-editor-body">
          <div class="hosts-textarea-wrapper">
            <div class="hosts-line-numbers">
              <div
                v-for="line in lineCount"
                :key="line"
                class="hosts-line-number"
                :class="{ 'has-error': hasLineError(line) }"
              >
                {{ line }}
              </div>
            </div>
            <textarea
              v-model="hostsContent"
              class="hosts-textarea"
              :class="{ 'has-errors': validationErrors.length > 0 }"
              placeholder="# 示例 hosts 配置&#10;127.0.0.1  localhost&#10;192.168.1.100  myserver.local&#10;::1  localhost"
              @scroll="syncScroll"
            />
          </div>

          <div v-if="validationErrors.length > 0" class="hosts-validation-panel">
            <div class="hosts-validation-title">
              <el-icon><WarningFilled /></el-icon>
              <span>发现 {{ validationErrors.length }} 个问题</span>
            </div>
            <div class="hosts-validation-list">
              <div
                v-for="(err, idx) in validationErrors"
                :key="idx"
                class="hosts-validation-item"
              >
                {{ err }}
              </div>
            </div>
          </div>
        </div>

        <div class="hosts-editor-footer">
          <div class="hosts-stats">
            <span>{{ lineCount }} 行</span>
            <span class="hosts-stats-dot" />
            <span>{{ getEntryCount(hostsContent) }} 条映射</span>
            <span class="hosts-stats-dot" />
            <span>{{ getUniqueIPCount(hostsContent) }} 个 IP</span>
          </div>
          <div class="hosts-actions">
            <el-button
              type="primary"
              :loading="saving"
              :icon="Check"
              @click="saveHosts"
            >
              保存配置
            </el-button>
            <el-button
              type="success"
              :loading="activating"
              :icon="Select"
              @click="activateHosts"
            >
              设为当前配置
            </el-button>
          </div>
        </div>

        <!-- 备份区域 -->
        <el-collapse v-model="backupExpanded" class="hosts-backup-collapse">
          <el-collapse-item name="backup">
            <template #title>
              <div class="hosts-backup-title">
                <el-icon><FolderOpened /></el-icon>
                <span>备份历史</span>
                <el-tag size="small" type="info" class="hosts-backup-count">{{ backupList.length }}</el-tag>
              </div>
            </template>
            <el-table
              :data="backupList"
              size="small"
              :loading="backupListLoading"
              class="hosts-backup-table"
            >
              <el-table-column prop="filename" label="文件名" min-width="200" show-overflow-tooltip />
              <el-table-column prop="modifiedAt" label="备份时间" width="150">
                <template #default="{ row }">{{ formatTime(row.modifiedAt) }}</template>
              </el-table-column>
              <el-table-column label="大小" width="80" align="right">
                <template #default="{ row }">{{ formatSize(row.size) }}</template>
              </el-table-column>
              <el-table-column label="操作" width="80" align="center">
                <template #default="{ row }">
                  <el-button
                    size="small"
                    type="warning"
                    link
                    @click="restoreBackup(row.filename)"
                  >恢复</el-button>
                </template>
              </el-table-column>
            </el-table>
            <div v-if="backupList.length === 0 && !backupListLoading" class="hosts-backup-empty">
              暂无备份记录
            </div>
          </el-collapse-item>
        </el-collapse>
      </main>
    </div>

    <!-- 右键菜单（全局层级，避免定位问题） -->
    <Teleport to="body">
      <div
        v-show="contextMenuVisible"
        ref="contextMenuRef"
        class="hosts-context-menu"
        :style="contextMenuPosition"
      >
        <div
          class="hosts-context-menu-item"
          :class="{ 'is-disabled': contextMenuProfile?.enabled }"
          @click="activateContextProfile"
        >
          <el-icon><CircleCheck /></el-icon>
          <span>{{ contextMenuProfile?.enabled ? "已是当前配置" : "设为当前配置" }}</span>
        </div>
        <div class="hosts-context-menu-divider"></div>
        <div class="hosts-context-menu-item" @click="cloneContextProfile">
          <el-icon><DocumentCopy /></el-icon>
          <span>克隆配置</span>
        </div>
        <div class="hosts-context-menu-item is-danger" @click="deleteContextProfile">
          <el-icon><Delete /></el-icon>
          <span>删除配置</span>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  InfoFilled,
  WarningFilled,
  Rank,
  Plus,
  MoreFilled,
  CopyDocument,
  Reading,
  Delete,
  DeleteFilled,
  Check,
  CircleCheck,
  FolderOpened,
  DocumentCopy,
} from "@element-plus/icons-vue";
import Sortable from "sortablejs";
import { invokeToolByChannel } from "../bridge/tauri";
import type { HostsProfile, HostsBackupEntry } from "../types";

// --- state ---
const hostsName = ref("");
const hostsContent = ref("");
const hostsProfiles = ref<HostsProfile[]>([]);
const backupList = ref<HostsBackupEntry[]>([]);
const canWrite = ref(false);
const adminChecked = ref(false);
const searchKeyword = ref("");
const backupExpanded = ref<string[]>([]);
const profileListRef = ref<HTMLElement | null>(null);
const textareaRef = ref<HTMLTextAreaElement | null>(null);
const contextMenuRef = ref(null);
const contextMenuVisible = ref(false);
const contextMenuProfile = ref<HostsProfile | null>(null);
const contextMenuPosition = ref<{ left: string; top: string; position: string }>({
  left: "0px",
  top: "0px",
  position: "fixed",
});

// --- loading flags ---
const saving = ref(false);
const activating = ref(false);
const deleting = ref(false);
const listLoading = ref(false);
const readingSystem = ref(false);
const backupListLoading = ref(false);

// --- computed ---
const filteredProfiles = computed(() => {
  if (!searchKeyword.value.trim()) return hostsProfiles.value;
  const keyword = searchKeyword.value.toLowerCase();
  return hostsProfiles.value.filter((p) => p.name.toLowerCase().includes(keyword));
});

const lineCount = computed(() => {
  if (!hostsContent.value) return 1;
  return hostsContent.value.split("\n").length;
});

// --- hosts syntax validation ---
const validationErrors = computed(() => {
  const lines = hostsContent.value.split("\n");
  const errors: string[] = [];
  const ipv4Re = /^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$/;
  const ipv6Re = /^[0-9a-fA-F:]+$/;
  const hostnameRe = /^[a-zA-Z0-9._-]+$/;

  for (let i = 0; i < lines.length; i++) {
    if (errors.length >= 5) break;
    const raw = lines[i].trim();
    if (raw === "" || raw.startsWith("#")) continue;

    const commentIdx = raw.indexOf("#");
    const effective = commentIdx >= 0 ? raw.substring(0, commentIdx).trim() : raw;
    const parts = effective.split(/\s+/);

    if (parts.length < 2) {
      errors.push(`第 ${i + 1} 行: 至少需要 IP 地址和一个主机名`);
      continue;
    }

    const ip = parts[0];
    if (!ipv4Re.test(ip) && !ipv6Re.test(ip)) {
      errors.push(`第 ${i + 1} 行: "${ip}" 不是有效的 IP 地址`);
      continue;
    }

    for (let j = 1; j < parts.length; j++) {
      if (!hostnameRe.test(parts[j])) {
        errors.push(`第 ${i + 1} 行: "${parts[j]}" 不是有效的主机名`);
        break;
      }
    }
  }
  return errors;
});

const errorLines = computed(() => {
  const lines = new Set<number>();
  validationErrors.value.forEach((err) => {
    const match = err.match(/第 (\d+) 行/);
    if (match) lines.add(parseInt(match[1]));
  });
  return lines;
});

// --- helpers ---
function hasLineError(lineNum: number): boolean {
  return errorLines.value.has(lineNum);
}

function getEntryCount(content: string): number {
  if (!content) return 0;
  return content.split("\n").filter((line) => {
    const trimmed = line.trim();
    return trimmed && !trimmed.startsWith("#");
  }).length;
}

function getUniqueIPCount(content: string): number {
  if (!content) return 0;
  const ips = new Set<string>();
  content.split("\n").forEach((line) => {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) return;
    const parts = trimmed.split(/\s+/);
    if (parts[0]) ips.add(parts[0]);
  });
  return ips.size;
}

function formatTime(raw: string): string {
  if (!raw) return "";
  return raw.replace("T", " ").replace(/\.\d+$/, "");
}

function formatRelativeTime(raw: string): string {
  if (!raw) return "";
  const date = new Date(raw);
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  if (hours < 24) return `${hours} 小时前`;
  if (days < 7) return `${days} 天前`;
  return formatTime(raw);
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function syncScroll(e: Event) {
  const textarea = e.target as HTMLTextAreaElement;
  const lineNumbers = textarea.parentElement?.querySelector(".hosts-line-numbers") as HTMLElement;
  if (lineNumbers) {
    lineNumbers.scrollTop = textarea.scrollTop;
  }
}

// --- data loading ---
async function loadHostsProfiles() {
  listLoading.value = true;
  try {
    const data = await invokeToolByChannel("tool:hosts:list", {});
    hostsProfiles.value = Array.isArray(data) ? (data as HostsProfile[]) : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    listLoading.value = false;
  }
}

async function checkAdminAccess() {
  try {
    const data = (await invokeToolByChannel("tool:hosts:admin-check", {})) as { canWrite?: boolean };
    canWrite.value = !!data?.canWrite;
  } catch {
    canWrite.value = false;
  } finally {
    adminChecked.value = true;
  }
}

async function loadBackupList() {
  backupListLoading.value = true;
  try {
    const data = await invokeToolByChannel("tool:hosts:backup-list", {});
    backupList.value = Array.isArray(data) ? (data as HostsBackupEntry[]) : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    backupListLoading.value = false;
  }
}

// --- actions ---
function pickHosts(profile: HostsProfile) {
  hostsName.value = profile.name;
  hostsContent.value = profile.content;
}

// --- context menu ---
function onCardContextMenu(e: MouseEvent, profile: HostsProfile) {
  e.preventDefault();
  e.stopPropagation();
  contextMenuProfile.value = profile;
  contextMenuPosition.value = {
    left: `${e.clientX}px`,
    top: `${e.clientY}px`,
    position: "fixed",
  };
  contextMenuVisible.value = true;

  // 点击其他地方关闭菜单
  const closeMenu = (ev: MouseEvent) => {
    const target = ev.target as HTMLElement;
    if (!contextMenuRef.value?.contains(target)) {
      contextMenuVisible.value = false;
      document.removeEventListener("click", closeMenu);
      document.removeEventListener("contextmenu", closeMenu);
    }
  };

  // 延迟添加监听器，避免当前点击立即触发关闭
  setTimeout(() => {
    document.addEventListener("click", closeMenu);
    document.addEventListener("contextmenu", closeMenu);
  }, 0);
}

async function activateContextProfile() {
  if (!contextMenuProfile.value || contextMenuProfile.value.enabled) return;
  contextMenuVisible.value = false;
  activating.value = true;
  try {
    await invokeToolByChannel("tool:hosts:activate", {
      profileName: contextMenuProfile.value.name,
      content: contextMenuProfile.value.content,
    });
    await loadHostsProfiles();
    ElMessage.success(`已将 "${contextMenuProfile.value.name}" 设为当前 hosts 配置`);
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    activating.value = false;
    contextMenuProfile.value = null;
  }
}

async function cloneContextProfile() {
  if (!contextMenuProfile.value) return;
  contextMenuVisible.value = false;
  const newName = contextMenuProfile.value.name + "-副本";
  try {
    await invokeToolByChannel("tool:hosts:save", {
      name: newName,
      content: contextMenuProfile.value.content,
    });
    await loadHostsProfiles();
    ElMessage.success(`已克隆配置 "${contextMenuProfile.value.name}"`);
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    contextMenuProfile.value = null;
  }
}

async function deleteContextProfile() {
  if (!contextMenuProfile.value) return;
  contextMenuVisible.value = false;
  try {
    await ElMessageBox.confirm(
      `确定要删除配置 "${contextMenuProfile.value.name}" 吗？此操作不可撤销。`,
      "删除确认",
      { confirmButtonText: "删除", cancelButtonText: "取消", type: "warning" },
    );
  } catch {
    contextMenuProfile.value = null;
    return;
  }
  deleting.value = true;
  try {
    await invokeToolByChannel("tool:hosts:delete", { name: contextMenuProfile.value.name });
    await loadHostsProfiles();
    // 如果删除的是当前编辑器中的配置，清空编辑器
    if (hostsName.value === contextMenuProfile.value.name) {
      hostsName.value = "";
      hostsContent.value = "";
    }
    ElMessage.success("hosts 配置已删除");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    deleting.value = false;
    contextMenuProfile.value = null;
  }
}

function loadActiveProfileToEditor() {
  const activeProfile = hostsProfiles.value.find((p) => p.enabled);
  if (!activeProfile) return;
  pickHosts(activeProfile);
}

function createNewConfig() {
  hostsName.value = "";
  hostsContent.value = "# 新建 hosts 配置\n";
}

async function handleMoreAction(command: string) {
  switch (command) {
    case "clone":
      await cloneConfig();
      break;
    case "readSystem":
      await readSystemHosts();
      break;
    case "clear":
      clearEditor();
      break;
    case "delete":
      await deleteHosts();
      break;
  }
}

async function cloneConfig() {
  if (!hostsName.value.trim() || !hostsContent.value.trim()) {
    ElMessage.warning("请先选择一个配置或输入内容");
    return;
  }
  const newName = hostsName.value + "-副本";
  hostsName.value = newName;
  await saveHosts();
}

async function saveHosts() {
  if (!hostsName.value.trim()) {
    ElMessage.warning("请输入配置名称");
    return;
  }
  if (validationErrors.value.length > 0) {
    try {
      await ElMessageBox.confirm(
        "hosts 内容存在语法错误，确定要保存吗？",
        "语法警告",
        { confirmButtonText: "仍然保存", cancelButtonText: "取消", type: "warning" },
      );
    } catch {
      return;
    }
  }
  const existing = hostsProfiles.value.find(
    (p) => p.name === hostsName.value.trim() && p.content === hostsContent.value,
  );
  if (existing) {
    try {
      await ElMessageBox.confirm(
        "当前内容与已保存的配置相同，确定要覆盖吗？",
        "重复内容",
        { confirmButtonText: "确定覆盖", cancelButtonText: "取消", type: "info" },
      );
    } catch {
      return;
    }
  }
  saving.value = true;
  try {
    await invokeToolByChannel("tool:hosts:save", {
      name: hostsName.value.trim(),
      content: hostsContent.value,
    });
    await loadHostsProfiles();
    ElMessage.success("hosts 配置已保存");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    saving.value = false;
  }
}

async function activateHosts() {
  if (!hostsName.value.trim()) {
    ElMessage.warning("请先输入或选择一个配置");
    return;
  }
  activating.value = true;
  try {
    await invokeToolByChannel("tool:hosts:activate", {
      profileName: hostsName.value.trim(),
      content: hostsContent.value,
    });
    await loadHostsProfiles();
    ElMessage.success(`已将 "${hostsName.value}" 设为当前 hosts 配置`);
  } catch (error) {
    console.error("[HOSTS] activateHosts 失败:", error);
    ElMessage.error((error as Error).message);
  } finally {
    activating.value = false;
  }
}

async function deleteHosts() {
  if (!hostsName.value.trim()) {
    ElMessage.warning("请先输入或选择要删除的配置");
    return;
  }
  try {
    await ElMessageBox.confirm(
      `确定要删除配置 "${hostsName.value}" 吗？此操作不可撤销。`,
      "删除确认",
      { confirmButtonText: "删除", cancelButtonText: "取消", type: "warning" },
    );
  } catch {
    return;
  }
  deleting.value = true;
  try {
    await invokeToolByChannel("tool:hosts:delete", { name: hostsName.value.trim() });
    await loadHostsProfiles();
    hostsName.value = "";
    hostsContent.value = "";
    ElMessage.success("hosts 配置已删除");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    deleting.value = false;
  }
}

async function readSystemHosts() {
  readingSystem.value = true;
  try {
    const data = (await invokeToolByChannel("tool:hosts:read-system", {})) as { content?: string };
    const activeProfile = hostsProfiles.value.find((p) => p.enabled);
    hostsName.value = activeProfile?.name ?? "";
    hostsContent.value = data?.content ?? "";
    ElMessage.success("已加载系统 hosts 文件内容");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    readingSystem.value = false;
  }
}

function clearEditor() {
  hostsName.value = "";
  hostsContent.value = "";
}

async function restoreBackup(filename: string) {
  try {
    await ElMessageBox.confirm(
      `确定要恢复备份 "${filename}" 吗？当前系统 hosts 将被覆盖（恢复前会自动备份）。`,
      "恢复确认",
      { confirmButtonText: "恢复", cancelButtonText: "取消", type: "warning" },
    );
  } catch {
    return;
  }
  try {
    const data = (await invokeToolByChannel("tool:hosts:backup-restore", { filename })) as { restoredFrom?: string };
    ElMessage.success(`已从 "${data?.restoredFrom}" 恢复 hosts 文件`);
    await loadBackupList();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

// --- drag sort ---
let sortableInstance: Sortable | null = null;

function initSortable(retries = 5) {
  if (sortableInstance) return;
  const listEl = profileListRef.value;
  if (!listEl || listEl.children.length === 0) {
    if (retries > 0) setTimeout(() => initSortable(retries - 1), 200);
    return;
  }
  sortableInstance = Sortable.create(listEl, {
    animation: 150,
    handle: ".hosts-drag-handle",
    ghostClass: "sortable-ghost",
    forceFallback: true,
    onEnd: async (evt) => {
      const { oldIndex, newIndex } = evt;
      if (oldIndex == null || newIndex == null || oldIndex === newIndex) return;
      const moved = hostsProfiles.value.splice(oldIndex, 1)[0];
      hostsProfiles.value.splice(newIndex, 0, moved);
      const ids = hostsProfiles.value.map((p) => p.id);
      try {
        await invokeToolByChannel("tool:hosts:reorder", { ids });
      } catch (error) {
        ElMessage.error((error as Error).message);
        await loadHostsProfiles();
      }
    },
  });
}

// --- lifecycle ---
onMounted(async () => {
  await loadHostsProfiles();
  loadActiveProfileToEditor();
  checkAdminAccess();
  loadBackupList();
  await nextTick();
  initSortable();
});

onBeforeUnmount(() => {
  if (sortableInstance) {
    sortableInstance.destroy();
    sortableInstance = null;
  }
});
</script>

<style scoped>
.hosts-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 12px;
}

/* Admin Banner */
.hosts-admin-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  color: var(--lc-text-secondary);
  font-size: 13px;
}

.hosts-admin-banner .el-icon {
  color: var(--lc-info);
  font-size: 16px;
}

/* Layout */
.hosts-layout {
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 16px;
  flex: 1;
  min-height: 0;
}

/* Sidebar */
.hosts-sidebar {
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: var(--lc-surface-1);
  border-radius: var(--lc-radius-md);
  padding: 16px;
  border: 1px solid var(--lc-border);
}

/* Search */
.hosts-search {
  position: relative;
  display: flex;
  align-items: center;
}

.hosts-search-icon {
  position: absolute;
  left: 12px;
  width: 16px;
  height: 16px;
  color: var(--lc-text-muted);
  pointer-events: none;
}

.hosts-search-input {
  width: 100%;
  height: 36px;
  padding: 0 32px;
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  color: var(--lc-text);
  font-size: 14px;
  transition: all var(--lc-duration) var(--lc-ease);
}

.hosts-search-input:focus {
  outline: none;
  border-color: var(--lc-border-active);
  background: var(--lc-surface-2);
}

.hosts-search-input::placeholder {
  color: var(--lc-text-muted);
}

.hosts-search-clear {
  position: absolute;
  right: 8px;
  width: 20px;
  height: 20px;
  padding: 0;
  background: transparent;
  border: none;
  color: var(--lc-text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  transition: all var(--lc-duration) var(--lc-ease);
}

.hosts-search-clear:hover {
  background: var(--lc-surface-2);
  color: var(--lc-text);
}

.hosts-search-clear svg {
  width: 14px;
  height: 14px;
}

/* List Header */
.hosts-list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 4px;
}

.hosts-list-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--lc-text);
}

.hosts-list-count {
  font-size: 12px;
  color: var(--lc-text-muted);
}

/* Profile List */
.hosts-profile-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 0;
  padding-right: 4px;
}

.hosts-profile-list.is-loading {
  opacity: 0.6;
  pointer-events: none;
}

/* Profile Card */
.hosts-profile-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px;
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  cursor: pointer;
  transition: all var(--lc-duration) var(--lc-ease);
  animation: cardEnter 250ms var(--lc-ease-out) forwards;
  opacity: 0;
  transform: translateY(8px);
}

@keyframes cardEnter {
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.hosts-profile-card:hover {
  border-color: var(--lc-border-hover);
  background: var(--lc-surface-2);
  transform: translateX(2px);
}

.hosts-profile-card.is-active {
  border-color: var(--lc-accent);
  background: var(--lc-accent-dim);
}

.hosts-profile-card.is-enabled {
  border-left: 3px solid var(--lc-success);
}

.hosts-profile-status {
  flex-shrink: 0;
}

.hosts-status-indicator {
  display: block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--lc-text-muted);
}

.hosts-status-indicator.is-active {
  background: var(--lc-success);
  box-shadow: 0 0 8px var(--lc-success);
}

.hosts-profile-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.hosts-profile-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--lc-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.hosts-profile-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--lc-text-muted);
}

.hosts-profile-dot {
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: var(--lc-text-muted);
}

.hosts-drag-handle {
  flex-shrink: 0;
  color: var(--lc-text-muted);
  font-size: 16px;
  cursor: grab;
  opacity: 0.5;
  transition: opacity var(--lc-duration) var(--lc-ease);
}

.hosts-profile-card:hover .hosts-drag-handle {
  opacity: 1;
}

.hosts-drag-handle:active {
  cursor: grabbing;
}

/* Sidebar Actions */
.hosts-sidebar-actions {
  padding-top: 8px;
  border-top: 1px solid var(--lc-border);
}

.hosts-sidebar-actions .el-button {
  width: 100%;
}

/* Editor Area */
.hosts-editor-area {
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: var(--lc-surface-1);
  border-radius: var(--lc-radius-md);
  padding: 16px;
  border: 1px solid var(--lc-border);
  min-height: 0;
}

/* Editor Header */
.hosts-editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.hosts-editor-title {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
}

.hosts-editor-title span {
  font-size: 14px;
  font-weight: 500;
  color: var(--lc-text);
  white-space: nowrap;
}

.hosts-name-input {
  max-width: 300px;
}

.hosts-name-input :deep(.el-input__wrapper) {
  background: var(--lc-surface-0);
}

/* Editor Body */
.hosts-editor-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  overflow: hidden;
}

.hosts-textarea-wrapper {
  flex: 1;
  display: flex;
  gap: 8px;
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  overflow: hidden;
  min-height: 0;
}

.hosts-line-numbers {
  width: 48px;
  padding: 12px 4px;
  background: var(--lc-surface-1);
  border-right: 1px solid var(--lc-border);
  font-family: var(--lc-font-mono);
  font-size: 13px;
  line-height: 1.6;
  color: var(--lc-text-muted);
  text-align: right;
  overflow: hidden;
  user-select: none;
}

.hosts-line-number {
  padding: 0 8px;
  transition: all var(--lc-duration) var(--lc-ease);
}

.hosts-line-number.has-error {
  color: var(--lc-danger);
  background: rgba(248, 113, 113, 0.1);
  border-radius: 4px;
}

.hosts-textarea {
  flex: 1;
  padding: 12px;
  background: transparent;
  border: none;
  color: var(--lc-text);
  font-family: var(--lc-font-mono);
  font-size: 13px;
  line-height: 1.6;
  resize: none;
  outline: none;
  overflow: auto;
  tab-size: 2;
}

.hosts-textarea::placeholder {
  color: var(--lc-text-muted);
}

.hosts-textarea.has-errors {
  background: linear-gradient(to right, rgba(248, 113, 113, 0.05), transparent 20px);
}

/* Validation Panel */
.hosts-validation-panel {
  padding: 12px 16px;
  background: rgba(248, 113, 113, 0.08);
  border: 1px solid rgba(248, 113, 113, 0.2);
  border-radius: var(--lc-radius-md);
}

.hosts-validation-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 500;
  color: var(--lc-danger);
  margin-bottom: 8px;
}

.hosts-validation-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.hosts-validation-item {
  font-size: 12px;
  color: var(--lc-text-secondary);
  padding: 4px 0;
  padding-left: 24px;
}

/* Editor Footer */
.hosts-editor-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 12px;
  border-top: 1px solid var(--lc-border);
}

.hosts-stats {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--lc-text-muted);
}

.hosts-stats-dot {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--lc-text-muted);
}

.hosts-actions {
  display: flex;
  gap: 8px;
}

/* Backup Collapse */
.hosts-backup-collapse {
  margin-top: 8px;
}

.hosts-backup-collapse :deep(.el-collapse-item__header) {
  background: transparent;
  border: none;
  padding: 0;
  height: 40px;
  font-size: 14px;
  color: var(--lc-text);
}

.hosts-backup-collapse :deep(.el-collapse-item__content) {
  padding: 12px 0 0 0;
}

.hosts-backup-title {
  display: flex;
  align-items: center;
  gap: 8px;
}

.hosts-backup-title .el-icon {
  font-size: 16px;
  color: var(--lc-text-secondary);
}

.hosts-backup-count {
  margin-left: 4px;
}

.hosts-backup-table {
  background: transparent;
}

.hosts-backup-table :deep(.el-table__header-wrapper) {
  background: var(--lc-surface-0);
}

.hosts-backup-empty {
  text-align: center;
  padding: 24px;
  color: var(--lc-text-muted);
  font-size: 13px;
}

/* Delete Item */
.hosts-delete-item {
  color: var(--lc-danger) !important;
}

/* Context Menu */
.hosts-context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 160px;
  background: var(--lc-surface-2);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  box-shadow: var(--lc-shadow-lg);
  padding: 6px 0;
  animation: contextMenuEnter 150ms var(--lc-ease-out);
}

@keyframes contextMenuEnter {
  from {
    opacity: 0;
    transform: scale(0.95);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.hosts-context-menu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 16px;
  color: var(--lc-text);
  font-size: 13px;
  cursor: pointer;
  transition: all var(--lc-duration) var(--lc-ease);
  user-select: none;
}

.hosts-context-menu-item:hover:not(.is-disabled) {
  background: var(--lc-surface-3);
}

.hosts-context-menu-item.is-disabled {
  color: var(--lc-success);
  opacity: 0.8;
  cursor: not-allowed;
}

.hosts-context-menu-item.is-danger {
  color: var(--lc-danger);
}

.hosts-context-menu-item.is-danger:hover {
  background: rgba(248, 113, 113, 0.1);
}

.hosts-context-menu-item .el-icon {
  font-size: 16px;
}

.hosts-context-menu-divider {
  height: 1px;
  background: var(--lc-border);
  margin: 6px 0;
}

/* Sortable Ghost */
:deep(.sortable-ghost) {
  opacity: 0.4;
  background: var(--lc-accent-dim);
  border: 1px dashed var(--lc-accent);
}

/* Responsive */
@media (max-width: 900px) {
  .hosts-layout {
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr;
  }

  .hosts-sidebar {
    max-height: 300px;
  }
}
</style>
