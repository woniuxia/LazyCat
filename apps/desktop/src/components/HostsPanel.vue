<template>
  <div class="hosts-panel">
    <!-- 管理员权限提示 -->
    <div v-if="adminChecked && !canWrite" class="hosts-admin-banner">
      <el-icon><InfoFilled /></el-icon>
      <span>当前非管理员模式。激活或恢复 hosts 时将弹出 UAC 提权确认窗口。</span>
    </div>

    <!-- 系统 hosts 与激活 profile 不一致提示 -->
    <div v-if="consistencyWarning" class="hosts-admin-banner is-warning">
      <el-icon><WarningFilled /></el-icon>
      <span>{{ consistencyWarning }}</span>
      <el-button size="small" text type="primary" @click="reloadSystemHosts">读取系统 hosts</el-button>
      <el-button size="small" text @click="dismissConsistencyWarning">忽略</el-button>
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

        <div ref="profileListRef" class="hosts-profile-list" :class="{ 'is-loading': listLoading || reorderLoading }">
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
              :class="{ 'is-readonly': isEditorReadonly }"
              :readonly="isEditorReadonly"
            />
          </div>
          <div class="hosts-editor-actions">
            <el-button
              :type="isEditorReadonly ? 'primary' : 'default'"
              :icon="isEditorReadonly ? EditPen : Lock"
              @click="toggleEditorMode"
            >
              {{ isEditorReadonly ? "可编辑" : "只读" }}
            </el-button>
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
          <div class="hosts-textarea-wrapper" :class="{ 'is-readonly': isEditorReadonly }">
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
              ref="textareaRef"
              v-model="hostsContent"
              class="hosts-textarea"
              :class="{ 'has-errors': validationErrors.length > 0, 'is-readonly': isEditorReadonly }"
              :readonly="isEditorReadonly"
              :title="isEditorReadonly ? '双击进入编辑模式' : ''"
              placeholder="# 示例 hosts 配置&#10;127.0.0.1  localhost&#10;192.168.1.100  myserver.local&#10;::1  localhost"
              @scroll="syncScroll"
              @dblclick="onEditorDblClick"
            />
          </div>

          <div v-if="validationErrors.length > 0" class="hosts-validation-panel">
            <div class="hosts-validation-title">
              <el-icon><WarningFilled /></el-icon>
              <span>
                发现 {{ validationErrorTotal }} 个问题<template v-if="validationErrorTotal > validationErrors.length">（仅显示前 {{ validationErrors.length }} 个）</template>
              </span>
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
              :disabled="isEditorReadonly"
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
              <el-table-column label="操作" width="120" align="center">
                <template #default="{ row }">
                  <el-button
                    size="small"
                    type="warning"
                    link
                    @click="restoreBackup(row.filename)"
                  >恢复</el-button>
                  <el-button
                    size="small"
                    type="danger"
                    link
                    @click="deleteBackup(row.filename)"
                  >删除</el-button>
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
  EditPen,
  Lock,
} from "@element-plus/icons-vue";
import Sortable from "sortablejs";
import { useListSearch } from "../composables/useListSearch";
import { useToolInvoke } from "../composables/useToolInvoke";
import type { HostsProfile, HostsBackupEntry } from "../types";

// --- state ---
const hostsName = ref("");
const hostsContent = ref("");
const hostsProfiles = ref<HostsProfile[]>([]);
const backupList = ref<HostsBackupEntry[]>([]);
const canWrite = ref(false);
const adminChecked = ref(false);
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
const isEditorReadonly = ref(true);
// 编辑器当前承载的 profile id；null 表示新建草稿
const editingProfileId = ref<number | null>(null);
// 系统 hosts 与激活 profile 不一致时显示的 banner 文案
const consistencyWarning = ref("");
// --- loading flags ---
const {
  loading: saving,
  invoke: invokeSaveRaw,
} = useToolInvoke();
const { loading: activating, invokeWithLoading: invokeActivating } = useToolInvoke();
const { loading: deleting, invokeWithLoading: invokeDeleting } = useToolInvoke();
const { loading: listLoading, invokeWithLoading: invokeList } = useToolInvoke();
const { loading: readingSystem, invokeWithLoading: invokeReadSystem } = useToolInvoke();
const { loading: backupListLoading, invokeWithLoading: invokeBackupList } = useToolInvoke();
const { loading: reorderLoading, invokeWithLoading: invokeReorder } = useToolInvoke();
const { invokeWithLoading: invokeHosts, invokeSilent } = useToolInvoke();

// --- computed ---
const {
  keyword: searchKeyword,
  filtered: filteredProfiles,
} = useListSearch(
  () => hostsProfiles.value,
  (profile, keyword) => profile.name.toLowerCase().includes(keyword.toLowerCase()),
);

const lineCount = computed(() => {
  if (!hostsContent.value) return 1;
  return hostsContent.value.split("\n").length;
});

// --- hosts syntax validation ---
// 收集所有错误用于计数；前 MAX_DISPLAY 条返回给 UI 渲染，其余仅计入总数。
const MAX_DISPLAY_ERRORS = 5;
const allValidationErrors = computed(() => {
  const lines = hostsContent.value.split("\n");
  const errors: string[] = [];
  // IPv4：四段 0-255
  const ipv4Re = /^(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)){3}$/;
  // IPv6：粗校验，允许压缩零段
  const ipv6Re = /^(?:[0-9a-fA-F]{1,4}:){2,7}[0-9a-fA-F]{1,4}$|^::$|^::1$|^(?:[0-9a-fA-F]{1,4}:){1,7}:$|^:(?::[0-9a-fA-F]{1,4}){1,7}$|^(?:[0-9a-fA-F]{1,4}:){1,6}(?::[0-9a-fA-F]{1,4}){1,6}$/;
  // 主机名 RFC 1123：字母数字开头/结尾，中间可有连字符
  const hostnameLabelRe = /^[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$/;

  for (let i = 0; i < lines.length; i++) {
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

    let badHostname: string | null = null;
    for (let j = 1; j < parts.length; j++) {
      const host = parts[j];
      const labels = host.split(".");
      if (!labels.every((label) => hostnameLabelRe.test(label))) {
        badHostname = host;
        break;
      }
    }
    if (badHostname) {
      errors.push(`第 ${i + 1} 行: "${badHostname}" 不是有效的主机名`);
    }
  }
  return errors;
});

const validationErrors = computed(() => allValidationErrors.value.slice(0, MAX_DISPLAY_ERRORS));
const validationErrorTotal = computed(() => allValidationErrors.value.length);

const errorLines = computed(() => {
  const lines = new Set<number>();
  // 行号高亮覆盖所有错误，而非仅前 5 条，否则高亮会与文案不一致
  allValidationErrors.value.forEach((err) => {
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
  // 超过 7 天的更新时间，列表里只展示日期；想看具体时间可以在备份表里查
  return raw.split(/[T ]/)[0];
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
  const data = await invokeList<HostsProfile[]>(
    "tool:hosts:list",
    {},
    { errorPrefix: "加载配置失败：" },
  );
  if (data) hostsProfiles.value = Array.isArray(data) ? data : [];
}

async function checkAdminAccess() {
  // 权限状态只影响提示文案，检测失败按非管理员处理即可。
  const data = await invokeSilent<{ canWrite?: boolean }>("tool:hosts:admin-check", {});
  canWrite.value = !!data?.canWrite;
  adminChecked.value = true;
}

async function loadBackupList() {
  const data = await invokeBackupList<HostsBackupEntry[]>(
    "tool:hosts:backup-list",
    {},
    { errorPrefix: "备份列表加载失败：" },
  );
  if (data) backupList.value = Array.isArray(data) ? data : [];
}

// --- actions ---
function pickHosts(profile: HostsProfile) {
  hostsName.value = profile.name;
  hostsContent.value = profile.content;
  editingProfileId.value = profile.id;
  isEditorReadonly.value = true;
}

async function toggleEditorMode() {
  const nextReadonly = !isEditorReadonly.value;
  isEditorReadonly.value = nextReadonly;
  if (!nextReadonly) {
    await nextTick();
    textareaRef.value?.focus();
  }
}

async function onEditorDblClick() {
  if (!isEditorReadonly.value) return;
  isEditorReadonly.value = false;
  await nextTick();
  textareaRef.value?.focus();
}

function generateUniqueName(baseName: string): string {
  const existing = new Set(hostsProfiles.value.map((p) => p.name));
  const base = `${baseName}-副本`;
  if (!existing.has(base)) return base;
  for (let i = 2; i < 1000; i++) {
    const candidate = `${baseName}-副本${i}`;
    if (!existing.has(candidate)) return candidate;
  }
  return `${baseName}-副本-${Date.now()}`;
}

function normalizeHostsContent(s: string): string {
  return (s ?? "").replace(/\r/g, "").trim();
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

async function saveHostsProfile(
  payload: { name: string; content: string; mode: "create" | "update" },
  duplicateMessage: string,
  fallbackPrefix: string,
) {
  saving.value = true;
  try {
    await invokeSaveRaw<Record<string, unknown>>("tool:hosts:save", payload);
    return true;
  } catch (error) {
    const msg = getErrorMessage(error);
    ElMessage.error(msg.includes("DUPLICATE_NAME") ? duplicateMessage : `${fallbackPrefix}${msg}`);
    return false;
  } finally {
    saving.value = false;
  }
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
  const target = contextMenuProfile.value;
  contextMenuVisible.value = false;
  contextMenuProfile.value = null;
  const activated = await invokeActivating<Record<string, unknown>>(
    "tool:hosts:activate",
    {
      profileName: target.name,
      content: target.content,
    },
    { errorPrefix: "激活失败：" },
  );
  if (!activated) return;
  await loadHostsProfiles();
  await verifyConsistency();
  if (backupExpanded.value.length > 0) await loadBackupList();
  ElMessage.success(`已将 "${target.name}" 设为当前 hosts 配置`);
}

async function cloneContextProfile() {
  if (!contextMenuProfile.value) return;
  const source = contextMenuProfile.value;
  contextMenuVisible.value = false;
  contextMenuProfile.value = null;
  const newName = generateUniqueName(source.name);
  const cloned = await saveHostsProfile(
    {
      name: newName,
      content: source.content,
      mode: "create",
    },
    `配置 "${newName}" 已存在`,
    "克隆失败：",
  );
  if (!cloned) return;
  await loadHostsProfiles();
  ElMessage.success(`已克隆为 "${newName}"`);
}

async function deleteContextProfile() {
  if (!contextMenuProfile.value) return;
  const name = contextMenuProfile.value.name;
  contextMenuVisible.value = false;
  contextMenuProfile.value = null;
  await deleteProfileByName(name);
}

function loadActiveProfileToEditor() {
  const activeProfile = hostsProfiles.value.find((p) => p.enabled);
  if (!activeProfile) return;
  pickHosts(activeProfile);
}

function createNewConfig() {
  hostsName.value = "";
  hostsContent.value = "# 新建 hosts 配置\n";
  editingProfileId.value = null;
  isEditorReadonly.value = false;
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
  const newName = generateUniqueName(hostsName.value.trim());
  const cloned = await saveHostsProfile(
    {
      name: newName,
      content: hostsContent.value,
      mode: "create",
    },
    `配置 "${newName}" 已存在`,
    "克隆失败：",
  );
  if (!cloned) return;
  await loadHostsProfiles();
  const clonedProfile = hostsProfiles.value.find((p) => p.name === newName);
  if (clonedProfile) pickHosts(clonedProfile);
  ElMessage.success(`已克隆为 "${newName}"`);
}

/**
 * 保存逻辑（严格区分新建 / 更新，避免静默覆盖）：
 *
 * - 编辑器无 editing 上下文（新建草稿） → mode: "create"
 *   - 命中已存在 name 时弹窗确认是否覆盖；确认后转为 update
 * - 编辑器有 editing 上下文：
 *   - name 未改 → mode: "update"
 *   - name 改成了另一个已存在 profile 的 name → 弹窗确认会覆盖目标 profile；确认后 update 目标
 *   - name 改成了完全新的名字 → mode: "create"（保留原 profile，等同另存为）
 */
async function saveHosts() {
  const trimmedName = hostsName.value.trim();
  if (!trimmedName) {
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

  const editing = editingProfileId.value
    ? hostsProfiles.value.find((p) => p.id === editingProfileId.value) ?? null
    : null;
  const existingByName = hostsProfiles.value.find((p) => p.name === trimmedName);

  let mode: "create" | "update";
  let targetName = trimmedName;

  if (editing && editing.name === trimmedName) {
    // 直接更新当前正在编辑的 profile
    mode = "update";
    if (editing.content === hostsContent.value) {
      ElMessage.info("内容无变化");
      return;
    }
  } else if (existingByName) {
    // 新名字命中了另一个已存在的 profile：必须显式确认才覆盖
    try {
      await ElMessageBox.confirm(
        `配置 "${trimmedName}" 已存在，将以当前编辑器内容覆盖它。是否继续？`,
        "覆盖确认",
        { confirmButtonText: "覆盖", cancelButtonText: "取消", type: "warning" },
      );
    } catch {
      return;
    }
    mode = "update";
    targetName = existingByName.name;
  } else {
    mode = "create";
  }

  const savedOk = await saveHostsProfile(
    {
      name: targetName,
      content: hostsContent.value,
      mode,
    },
    `配置 "${targetName}" 已存在`,
    "保存失败：",
  );
  if (!savedOk) return;
  await loadHostsProfiles();
  const saved = hostsProfiles.value.find((p) => p.name === targetName);
  if (saved) {
    editingProfileId.value = saved.id;
    hostsName.value = saved.name;
  }
  ElMessage.success("hosts 配置已保存");
}

/**
 * 激活逻辑（带 dirty 检测）：
 *
 * 当编辑器内容与已保存版本不一致时，给用户三选一：
 * - 保存后激活：先以 update 持久化编辑内容，再激活
 * - 使用已保存版本激活：放弃编辑器内未保存修改
 * - 取消
 */
async function activateHosts() {
  const trimmedName = hostsName.value.trim();
  if (!trimmedName) {
    ElMessage.warning("请先输入或选择一个配置");
    return;
  }

  const target = hostsProfiles.value.find((p) => p.name === trimmedName);
  let contentToActivate = hostsContent.value;

  if (target && target.content !== hostsContent.value) {
    let userChoice: "confirm" | "cancel" | "close";
    try {
      await ElMessageBox({
        title: "未保存的修改",
        message:
          "编辑器内容与已保存的配置不一致。\n激活时如不先保存，写入系统的将是编辑器中的内容，但配置文件中的版本不会更新，可能导致下次启动后看到的与实际生效的不一致。",
        showCancelButton: true,
        confirmButtonText: "先保存再激活",
        cancelButtonText: "用已保存版本激活",
        distinguishCancelAndClose: true,
        type: "warning",
      });
      userChoice = "confirm";
    } catch (e) {
      userChoice = e === "close" ? "close" : "cancel";
    }
    if (userChoice === "close") return;
    if (userChoice === "confirm") {
      const saved = await saveHostsProfile(
        {
          name: trimmedName,
          content: hostsContent.value,
          mode: "update",
        },
        `配置 "${trimmedName}" 已存在`,
        "保存失败：",
      );
      if (!saved) return;
    } else {
      // 用已保存版本激活：把编辑器恢复为持久化内容
      hostsContent.value = target.content;
      contentToActivate = target.content;
    }
  }

  const activated = await invokeActivating<Record<string, unknown>>(
    "tool:hosts:activate",
    {
      profileName: trimmedName,
      content: contentToActivate,
    },
    { errorPrefix: "激活失败：" },
  );
  if (!activated) return;
  await loadHostsProfiles();
  await verifyConsistency();
  if (backupExpanded.value.length > 0) await loadBackupList();
  ElMessage.success(`已将 "${trimmedName}" 设为当前 hosts 配置`);
}

async function deleteHosts() {
  await deleteProfileByName(hostsName.value.trim());
}

/**
 * 统一的删除入口：处理确认、清理编辑器、以及"删除了激活 profile 后系统 hosts
 * 仍在生效"的二次提示（引导用户去备份列表恢复）。
 */
async function deleteProfileByName(name: string) {
  if (!name) {
    ElMessage.warning("请先输入或选择要删除的配置");
    return;
  }
  try {
    await ElMessageBox.confirm(
      `确定要删除配置 "${name}" 吗？此操作不可撤销。`,
      "删除确认",
      { confirmButtonText: "删除", cancelButtonText: "取消", type: "warning" },
    );
  } catch {
    return;
  }
  const result = await invokeDeleting<{
    wasActive?: boolean;
    deleted?: boolean;
  }>(
    "tool:hosts:delete",
    { name },
    { errorPrefix: "删除失败：" },
  );
  if (!result) return;
  await loadHostsProfiles();
  if (hostsName.value === name) {
    hostsName.value = "";
    hostsContent.value = "";
    editingProfileId.value = null;
    isEditorReadonly.value = true;
  }
  ElMessage.success("hosts 配置已删除");

  if (result.wasActive) {
    // 系统 hosts 仍是被删 profile 的内容；引导用户去备份历史恢复
    try {
      await ElMessageBox.confirm(
        `刚删除的配置当前仍在系统 hosts 中生效。\n\n是否打开备份历史，恢复到此次激活前的状态？`,
        "系统 hosts 未自动清理",
        { confirmButtonText: "查看备份", cancelButtonText: "暂不处理", type: "warning" },
      );
      backupExpanded.value = ["backup"];
      await loadBackupList();
    } catch {
      /* 用户暂不处理 */
    }
    await verifyConsistency();
  }
}

async function readSystemHosts() {
  const data = await invokeReadSystem<{ content?: string }>(
    "tool:hosts:read-system",
    {},
    { errorPrefix: "读取系统 hosts 失败：" },
  );
  if (!data) return;
  const activeProfile = hostsProfiles.value.find((p) => p.enabled);
  hostsName.value = activeProfile?.name ?? "";
  hostsContent.value = data.content ?? "";
  editingProfileId.value = activeProfile?.id ?? null;
  isEditorReadonly.value = true;
  ElMessage.success("已加载系统 hosts 文件内容");
}

function clearEditor() {
  hostsName.value = "";
  hostsContent.value = "";
  editingProfileId.value = null;
}

async function restoreBackup(filename: string) {
  try {
    await ElMessageBox.confirm(
      `确定要恢复备份 "${filename}" 吗？当前系统 hosts 将被覆盖（恢复前会自动备份），所有 profile 的激活标记会被清除。`,
      "恢复确认",
      { confirmButtonText: "恢复", cancelButtonText: "取消", type: "warning" },
    );
  } catch {
    return;
  }
  const data = await invokeHosts<{ restoredFrom?: string }>(
    "tool:hosts:backup-restore",
    { filename },
    { errorPrefix: "恢复备份失败：" },
  );
  if (!data) return;
  ElMessage.success(`已从 "${data.restoredFrom}" 恢复 hosts 文件`);
  // 后端已清空 enabled，前端必须同步刷新列表与一致性 banner
  await loadHostsProfiles();
  await loadBackupList();
  await verifyConsistency();
}

async function deleteBackup(filename: string) {
  try {
    await ElMessageBox.confirm(
      `确定要删除备份 "${filename}" 吗？此操作不可撤销。`,
      "删除备份",
      { confirmButtonText: "删除", cancelButtonText: "取消", type: "warning" },
    );
  } catch {
    return;
  }
  const deleted = await invokeHosts<Record<string, unknown>>(
    "tool:hosts:backup-delete",
    { filename },
    { errorPrefix: "删除备份失败：" },
  );
  if (!deleted) return;
  ElMessage.success("备份已删除");
  await loadBackupList();
}

/**
 * 校验系统 hosts 与激活 profile 内容是否一致。
 * 不一致时（外部工具直接修改了 hosts、或激活后被还原）显示顶部 banner，
 * 引导用户用"读取系统 hosts"或"重新激活"两种方式收敛状态。
 */
async function verifyConsistency() {
  // 一致性校验是后台提示能力，读取失败时不打扰用户。
  const sys = await invokeSilent<{ content?: string }>("tool:hosts:read-system", {});
  const active = hostsProfiles.value.find((p) => p.enabled);
  if (!sys) {
    consistencyWarning.value = "";
    return;
  }
  if (!active) {
    consistencyWarning.value = "";
    return;
  }
  if (normalizeHostsContent(active.content) === normalizeHostsContent(sys?.content ?? "")) {
    consistencyWarning.value = "";
  } else {
    consistencyWarning.value = `当前激活的配置 "${active.name}" 与系统 hosts 文件不一致，可能被外部工具修改。`;
  }
}

async function reloadSystemHosts() {
  await readSystemHosts();
  consistencyWarning.value = "";
}

function dismissConsistencyWarning() {
  consistencyWarning.value = "";
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
      const reordered = await invokeReorder<Record<string, unknown>>(
        "tool:hosts:reorder",
        { ids },
        { errorPrefix: "排序保存失败：" },
      );
      if (!reordered) {
        await loadHostsProfiles();
      }
    },
  });
}

// --- lifecycle ---
onMounted(async () => {
  await loadHostsProfiles();
  loadActiveProfileToEditor();
  await checkAdminAccess();
  await loadBackupList();
  await verifyConsistency();
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

.hosts-admin-banner span {
  flex: 1;
}

.hosts-admin-banner.is-warning {
  border-color: var(--lc-warning, #f59e0b);
  background: rgba(245, 158, 11, 0.06);
  color: var(--lc-text);
}

.hosts-admin-banner.is-warning .el-icon {
  color: var(--lc-warning, #f59e0b);
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

.hosts-name-input.is-readonly :deep(.el-input__wrapper) {
  background: var(--lc-surface-1);
}

.hosts-editor-actions {
  display: flex;
  align-items: center;
  gap: 8px;
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

.hosts-textarea-wrapper.is-readonly {
  background: var(--lc-surface-1);
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

.hosts-textarea.is-readonly {
  color: var(--lc-text-secondary);
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
