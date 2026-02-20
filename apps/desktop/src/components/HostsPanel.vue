<template>
  <div class="panel-grid">
    <!-- 管理员权限提示 -->
    <div v-if="adminChecked && !canWrite" class="panel-grid-full hosts-admin-info">
      <el-icon><WarningFilled /></el-icon>
      当前非管理员模式。激活或恢复 hosts 时将弹出 UAC 提权确认窗口。
    </div>

    <!-- 配置编辑区 -->
    <el-divider class="panel-grid-full" content-position="left">配置编辑</el-divider>

    <el-input
      class="panel-grid-full"
      v-model="hostsName"
      placeholder="配置名称，例如 local-dev"
    />

    <el-input
      class="panel-grid-full"
      v-model="hostsContent"
      type="textarea"
      :rows="14"
      :class="{ 'hosts-textarea-error': validationErrors.length > 0 }"
      placeholder="# 示例 hosts 配置&#10;127.0.0.1  localhost&#10;192.168.1.100  myserver.local&#10;::1  localhost"
    />

    <div v-if="validationErrors.length > 0" class="panel-grid-full">
      <div
        v-for="(err, idx) in validationErrors"
        :key="idx"
        class="hosts-validation-line"
      >
        {{ err }}
      </div>
    </div>

    <div class="panel-grid-full">
      <el-space wrap>
        <el-button
          type="primary"
          :loading="saving"
          @click="saveHosts"
        >保存配置</el-button>
        <el-button
          type="success"
          :loading="activating"
          @click="activateHosts"
        >设为当前配置</el-button>
        <el-button
          type="danger"
          :loading="deleting"
          @click="deleteHosts"
        >删除配置</el-button>
        <el-button
          :loading="readingSystem"
          @click="readSystemHosts"
        >读取系统 hosts</el-button>
        <el-button @click="clearEditor">清空编辑器</el-button>
      </el-space>
    </div>

    <!-- 配置列表 -->
    <el-divider class="panel-grid-full" content-position="left">配置列表</el-divider>

    <el-table
      ref="profileTableRef"
      class="panel-grid-full"
      :data="hostsProfiles"
      border
      max-height="280"
      highlight-current-row
      :row-class-name="profileRowClass"
      :loading="listLoading"
      row-key="id"
    >
      <el-table-column prop="name" label="名称" min-width="180">
        <template #default="{ row }">
          {{ row.name }}
          <el-tag v-if="row.enabled" type="success" size="small" style="margin-left:6px">当前激活</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="updatedAt" label="更新时间" width="180">
        <template #default="{ row }">{{ formatTime(row.updatedAt) }}</template>
      </el-table-column>
      <el-table-column label="操作" width="110" align="center">
        <template #default="{ row }">
          <el-button size="small" @click="pickHosts(row)">载入</el-button>
        </template>
      </el-table-column>
    </el-table>

    <div v-if="hostsProfiles.length === 0 && !listLoading" class="panel-grid-full hosts-empty-hint">
      暂无已保存的 hosts 配置
    </div>

    <!-- 备份历史 -->
    <el-divider class="panel-grid-full" content-position="left">备份历史</el-divider>

    <div class="panel-grid-full">
      <el-button :loading="backupListLoading" @click="loadBackupList">刷新备份列表</el-button>
    </div>

    <el-table
      class="panel-grid-full"
      :data="backupList"
      border
      max-height="200"
      :loading="backupListLoading"
    >
      <el-table-column prop="filename" label="文件名" min-width="260" />
      <el-table-column prop="modifiedAt" label="备份时间" width="180" />
      <el-table-column label="大小" width="100" align="right">
        <template #default="{ row }">{{ formatSize(row.size) }}</template>
      </el-table-column>
      <el-table-column label="操作" width="110" align="center">
        <template #default="{ row }">
          <el-button
            size="small"
            type="warning"
            @click="restoreBackup(row.filename)"
          >恢复</el-button>
        </template>
      </el-table-column>
    </el-table>

    <div v-if="backupList.length === 0 && !backupListLoading" class="panel-grid-full hosts-empty-hint">
      暂无备份记录
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { WarningFilled } from "@element-plus/icons-vue";
import Sortable from "sortablejs";
import type { SortableEvent } from "sortablejs";
import { invokeToolByChannel } from "../bridge/tauri";
import type { HostsProfile, HostsBackupEntry } from "../types";

// --- state ---
const hostsName = ref("");
const hostsContent = ref("");
const hostsProfiles = ref<HostsProfile[]>([]);
const backupList = ref<HostsBackupEntry[]>([]);
const canWrite = ref(false);
const adminChecked = ref(false);
const profileTableRef = ref<InstanceType<typeof import("element-plus").ElTable> | null>(null);

// --- loading flags ---
const saving = ref(false);
const activating = ref(false);
const deleting = ref(false);
const listLoading = ref(false);
const readingSystem = ref(false);
const backupListLoading = ref(false);

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
      errors.push(`\u7B2C ${i + 1} \u884C: \u81F3\u5C11\u9700\u8981 IP \u5730\u5740\u548C\u4E00\u4E2A\u4E3B\u673A\u540D`);
      continue;
    }

    const ip = parts[0];
    if (!ipv4Re.test(ip) && !ipv6Re.test(ip)) {
      errors.push(`\u7B2C ${i + 1} \u884C: "${ip}" \u4E0D\u662F\u6709\u6548\u7684 IP \u5730\u5740`);
      continue;
    }

    for (let j = 1; j < parts.length; j++) {
      if (!hostnameRe.test(parts[j])) {
        errors.push(`\u7B2C ${i + 1} \u884C: "${parts[j]}" \u4E0D\u662F\u6709\u6548\u7684\u4E3B\u673A\u540D`);
        break;
      }
    }
  }
  return errors;
});

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

async function saveHosts() {
  if (!hostsName.value.trim()) {
    ElMessage.warning("\u8BF7\u8F93\u5165\u914D\u7F6E\u540D\u79F0");
    return;
  }
  if (validationErrors.value.length > 0) {
    try {
      await ElMessageBox.confirm(
        "hosts \u5185\u5BB9\u5B58\u5728\u8BED\u6CD5\u9519\u8BEF\uFF0C\u786E\u5B9A\u8981\u4FDD\u5B58\u5417\uFF1F",
        "\u8BED\u6CD5\u8B66\u544A",
        { confirmButtonText: "\u4ECD\u7136\u4FDD\u5B58", cancelButtonText: "\u53D6\u6D88", type: "warning" },
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
        "\u5F53\u524D\u5185\u5BB9\u4E0E\u5DF2\u4FDD\u5B58\u7684\u914D\u7F6E\u76F8\u540C\uFF0C\u786E\u5B9A\u8981\u8986\u76D6\u5417\uFF1F",
        "\u91CD\u590D\u5185\u5BB9",
        { confirmButtonText: "\u786E\u5B9A\u8986\u76D6", cancelButtonText: "\u53D6\u6D88", type: "info" },
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
    ElMessage.success("hosts \u914D\u7F6E\u5DF2\u4FDD\u5B58");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    saving.value = false;
  }
}

async function activateHosts() {
  if (!hostsName.value.trim()) {
    ElMessage.warning("\u8BF7\u5148\u8F93\u5165\u6216\u9009\u62E9\u4E00\u4E2A\u914D\u7F6E");
    return;
  }
  activating.value = true;
  try {
    await invokeToolByChannel("tool:hosts:activate", {
      profileName: hostsName.value.trim(),
      content: hostsContent.value,
    });
    await loadHostsProfiles();
    ElMessage.success(`\u5DF2\u5C06 "${hostsName.value}" \u8BBE\u4E3A\u5F53\u524D hosts \u914D\u7F6E`);
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    activating.value = false;
  }
}

async function deleteHosts() {
  if (!hostsName.value.trim()) {
    ElMessage.warning("\u8BF7\u5148\u8F93\u5165\u6216\u9009\u62E9\u8981\u5220\u9664\u7684\u914D\u7F6E");
    return;
  }
  try {
    await ElMessageBox.confirm(
      `\u786E\u5B9A\u8981\u5220\u9664\u914D\u7F6E "${hostsName.value}" \u5417\uFF1F\u6B64\u64CD\u4F5C\u4E0D\u53EF\u64A4\u9500\u3002`,
      "\u5220\u9664\u786E\u8BA4",
      { confirmButtonText: "\u5220\u9664", cancelButtonText: "\u53D6\u6D88", type: "warning" },
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
    ElMessage.success("hosts \u914D\u7F6E\u5DF2\u5220\u9664");
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
    ElMessage.success("\u5DF2\u52A0\u8F7D\u7CFB\u7EDF hosts \u6587\u4EF6\u5185\u5BB9");
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
      `\u786E\u5B9A\u8981\u6062\u590D\u5907\u4EFD "${filename}" \u5417\uFF1F\u5F53\u524D\u7CFB\u7EDF hosts \u5C06\u88AB\u8986\u76D6\uFF08\u6062\u590D\u524D\u4F1A\u81EA\u52A8\u5907\u4EFD\uFF09\u3002`,
      "\u6062\u590D\u786E\u8BA4",
      { confirmButtonText: "\u6062\u590D", cancelButtonText: "\u53D6\u6D88", type: "warning" },
    );
  } catch {
    return;
  }
  try {
    const data = (await invokeToolByChannel("tool:hosts:backup-restore", { filename })) as { restoredFrom?: string };
    ElMessage.success(`\u5DF2\u4ECE "${data?.restoredFrom}" \u6062\u590D hosts \u6587\u4EF6`);
    await loadBackupList();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

// --- helpers ---
function formatTime(raw: string): string {
  if (!raw) return "";
  return raw.replace("T", " ").replace(/\.\d+$/, "");
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function profileRowClass({ row }: { row: HostsProfile }): string {
  return row.enabled ? "hosts-row-active" : "";
}

// --- drag sort ---
let sortableInstance: Sortable | null = null;

async function onSortEnd(evt: SortableEvent) {
  const { oldIndex, newIndex } = evt;
  if (oldIndex == null || newIndex == null || oldIndex === newIndex) return;
  const list = [...hostsProfiles.value];
  const [moved] = list.splice(oldIndex, 1);
  list.splice(newIndex, 0, moved);
  hostsProfiles.value = list;
  const ids = list.map((p) => p.id);
  try {
    await invokeToolByChannel("tool:hosts:reorder", { ids });
  } catch (error) {
    ElMessage.error((error as Error).message);
    await loadHostsProfiles();
  }
}

function initSortable() {
  if (sortableInstance) {
    sortableInstance.destroy();
    sortableInstance = null;
  }
  const tableEl = profileTableRef.value?.$el as HTMLElement | undefined;
  if (!tableEl) return;
  const tbody = tableEl.querySelector<HTMLElement>(".el-table__body-wrapper tbody");
  if (!tbody) return;
  sortableInstance = Sortable.create(tbody, {
    animation: 150,
    ghostClass: "sortable-ghost",
    onEnd: onSortEnd,
  });
}

watch(hostsProfiles, async () => {
  await nextTick();
  initSortable();
});

// --- lifecycle ---
onMounted(() => {
  loadHostsProfiles();
  checkAdminAccess();
  loadBackupList();
});

onBeforeUnmount(() => {
  if (sortableInstance) {
    sortableInstance.destroy();
    sortableInstance = null;
  }
});
</script>

<style scoped>
.hosts-admin-info {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  background: var(--el-color-info-light-9);
  border: 1px solid var(--el-color-info-light-5);
  border-radius: 4px;
  color: var(--el-color-info-dark-2);
  font-size: 13px;
}

.hosts-validation-line {
  color: var(--el-color-danger);
  font-size: 12px;
  line-height: 1.6;
}

.hosts-textarea-error :deep(.el-textarea__inner) {
  border-color: var(--el-color-danger);
}

.hosts-empty-hint {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  text-align: center;
  padding: 12px 0;
}

:deep(.hosts-row-active) {
  background-color: var(--el-color-success-light-9) !important;
}

:deep(.el-table__body-wrapper tbody .el-table__row) {
  cursor: grab;
}

:deep(.sortable-ghost) {
  opacity: 0.4;
}
</style>
