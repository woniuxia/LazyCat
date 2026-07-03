<template>
  <div class="db-workbench-panel">
    <aside class="conn-side">
      <div class="conn-toolbar">
        <span class="conn-title">连接</span>
        <el-button size="small" type="primary" @click="openCreateDialog">新建连接</el-button>
      </div>
      <div v-loading="loading" class="conn-list">
        <template v-for="group in groupedConnections" :key="group.name">
          <div v-if="group.name" class="conn-group-title">{{ group.name }}</div>
          <div
            v-for="c in group.items"
            :key="c.id"
            class="conn-item"
            :class="{ active: c.id === activeConnectionId, opened: opened.has(c.id) }"
            @dblclick="openConnection(c)"
            @click="focusConnection(c)"
          >
            <span class="engine-badge" :class="c.engine">{{ c.engine === "mysql" ? "My" : "KB" }}</span>
            <span class="conn-name" :title="`${c.host}:${c.port}`">{{ c.name }}</span>
            <span class="env-dot" :style="{ background: DB_ENV_COLORS[c.envTag] }" :title="DB_ENV_LABELS[c.envTag]" />
            <el-icon v-if="c.readOnly" class="lock-icon" title="只读保护"><Lock /></el-icon>
            <span class="conn-actions" @click.stop @dblclick.stop>
              <el-dropdown trigger="click" size="small">
                <el-button link size="small"><el-icon><MoreFilled /></el-icon></el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item @click="openConnection(c)">
                      {{ opened.has(c.id) ? "切换到工作台" : "打开连接" }}
                    </el-dropdown-item>
                    <el-dropdown-item v-if="opened.has(c.id)" @click="closeConnection(c)">断开</el-dropdown-item>
                    <el-dropdown-item divided @click="openEditDialog(c)">编辑</el-dropdown-item>
                    <el-dropdown-item @click="deleteConnection(c)">删除</el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </span>
          </div>
        </template>
        <el-empty
          v-if="!loading && connections.length === 0"
          description="还没有数据库连接"
          :image-size="72"
        >
          <el-button type="primary" @click="openCreateDialog">新建连接</el-button>
        </el-empty>
      </div>
    </aside>

    <main class="workbench-main">
      <template v-if="openedList.length > 0">
        <div
          v-for="state in openedList"
          v-show="state.connectionId === activeConnectionId"
          :key="state.connectionId"
          class="workspace-holder"
        >
          <DbSqlWorkspace
            v-if="connectionOf(state.connectionId)"
            :connection="connectionOf(state.connectionId)!"
            :opened="state"
            @database-change="(db) => setActiveDatabase(state.connectionId, db)"
          />
        </div>
        <div v-if="!activeOpened" class="main-empty">
          <p>双击左侧连接打开工作台</p>
        </div>
      </template>
      <div v-else class="main-empty">
        <h3>数据库工作台</h3>
        <p>管理 MySQL / KingbaseES 连接，浏览库表结构，执行 SQL 与编辑表数据。</p>
        <p class="hint">双击左侧连接打开工作台；连接密码使用本地密钥加密存储。</p>
      </div>
    </main>

    <DbConnectionDialog
      v-model:visible="dialogVisible"
      :connection="editingConnection"
      @saved="onDialogSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Lock, MoreFilled } from "@element-plus/icons-vue";
import DbConnectionDialog from "./db/DbConnectionDialog.vue";
import DbSqlWorkspace from "./db/DbSqlWorkspace.vue";
import { useDbConnections } from "../composables/useDbConnections";
import {
  DB_ENV_COLORS,
  DB_ENV_LABELS,
  type DbConnection,
  type DbConnectionDraft,
} from "../types/db";

const {
  connections,
  loading,
  opened,
  refresh,
  save,
  remove,
  open,
  close,
  setActiveDatabase,
} = useDbConnections();

const dialogVisible = ref(false);
const editingConnection = ref<DbConnection | null>(null);
const activeConnectionId = ref("");

const openedList = computed(() => Array.from(opened.value.values()));
const activeOpened = computed(() => opened.value.get(activeConnectionId.value));

const groupedConnections = computed(() => {
  const groups = new Map<string, DbConnection[]>();
  for (const c of connections.value) {
    const key = c.groupName ?? "";
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(c);
  }
  return Array.from(groups.entries()).map(([name, items]) => ({ name, items }));
});

function connectionOf(id: string): DbConnection | undefined {
  return connections.value.find((c) => c.id === id);
}

function focusConnection(c: DbConnection): void {
  if (opened.value.has(c.id)) {
    activeConnectionId.value = c.id;
  }
}

async function openConnection(c: DbConnection): Promise<void> {
  if (opened.value.has(c.id)) {
    activeConnectionId.value = c.id;
    return;
  }
  const closeLoading = ElMessage({
    message: `正在连接 ${c.name}…`,
    type: "info",
    duration: 0,
  });
  try {
    await open(c.id);
    activeConnectionId.value = c.id;
  } catch (error) {
    ElMessage.error(`连接失败：${(error as Error).message}`);
  } finally {
    closeLoading.close();
  }
}

async function closeConnection(c: DbConnection): Promise<void> {
  try {
    await close(c.id);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
  if (activeConnectionId.value === c.id) {
    activeConnectionId.value = openedList.value[0]?.connectionId ?? "";
  }
}

function openCreateDialog(): void {
  editingConnection.value = null;
  dialogVisible.value = true;
}

function openEditDialog(c: DbConnection): void {
  editingConnection.value = c;
  dialogVisible.value = true;
}

async function onDialogSaved(draft: DbConnectionDraft): Promise<void> {
  try {
    await save(draft);
    dialogVisible.value = false;
    ElMessage.success(draft.id ? "连接已更新" : "连接已创建");
    // 已打开的连接改配置后需要重新打开
    if (draft.id && opened.value.has(draft.id)) {
      await close(draft.id).catch(() => undefined);
      if (activeConnectionId.value === draft.id) {
        activeConnectionId.value = "";
      }
      ElMessage.info("连接配置已变更，请重新打开该连接");
    }
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function deleteConnection(c: DbConnection): Promise<void> {
  try {
    await ElMessageBox.confirm(
      `删除连接「${c.name}」？其执行历史将一并删除，连接级 SQL 收藏会转为全局收藏。`,
      "删除连接",
      { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" }
    );
  } catch {
    return;
  }
  try {
    await remove(c.id);
    if (activeConnectionId.value === c.id) {
      activeConnectionId.value = openedList.value[0]?.connectionId ?? "";
    }
    ElMessage.success("已删除");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

onMounted(refresh);
</script>

<style scoped>
.db-workbench-panel {
  height: 100%;
  display: flex;
  min-height: 0;
}
.conn-side {
  width: 230px;
  flex-shrink: 0;
  border-right: 1px solid var(--lc-border);
  display: flex;
  flex-direction: column;
  padding: 12px 10px 12px 12px;
  min-height: 0;
}
.conn-toolbar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}
.conn-title {
  font-weight: 600;
  font-size: 14px;
}
.conn-list {
  flex: 1;
  overflow: auto;
  min-height: 0;
}
.conn-group-title {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  padding: 8px 4px 4px;
}
.conn-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 6px;
  border-radius: 8px;
  font-size: 13px;
  cursor: default;
}
.conn-item:hover {
  background: var(--el-fill-color-light);
}
.conn-item.active {
  background: var(--el-color-primary-light-9);
}
.conn-item.opened .conn-name {
  font-weight: 600;
}
.engine-badge {
  flex-shrink: 0;
  min-width: 24px;
  height: 18px;
  border-radius: 4px;
  font-size: 11px;
  line-height: 18px;
  text-align: center;
  color: #fff;
  background: #4479a1;
  padding: 0 3px;
}
.engine-badge.kingbase {
  background: #c04851;
}
.conn-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.env-dot {
  flex-shrink: 0;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.lock-icon {
  flex-shrink: 0;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
.conn-actions {
  flex-shrink: 0;
  display: none;
}
.conn-item:hover .conn-actions {
  display: inline-flex;
}
.workbench-main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.workspace-holder {
  flex: 1;
  min-height: 0;
  padding-left: 12px;
}
.main-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--el-text-color-secondary);
}
.main-empty h3 {
  margin: 0;
  color: var(--el-text-color-primary);
}
.main-empty .hint {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}
</style>
