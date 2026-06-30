<template>
  <aside class="api-workbench-sidebar" @contextmenu.prevent.stop="openMenu($event, { type: 'blank' })">
    <div class="api-workbench-toolbar">
      <strong>接口集合</strong>
      <el-button size="small" type="primary" :disabled="loading" @click="emitCommand('collection:create', { type: 'blank' })">
        新建
      </el-button>
    </div>

    <el-empty v-if="!loading && collections.length === 0" description="暂无接口集合" />
    <div v-else class="api-workbench-collection-list">
      <button
        v-for="collection in collections"
        :key="collection.id"
        type="button"
        class="api-workbench-collection"
        :class="{ active: selectedCollectionId === collection.id }"
        @click="emit('selectCollection', collection.id)"
        @contextmenu.prevent.stop="openMenu($event, { type: 'collection', collectionId: collection.id })"
      >
        <span>{{ collection.name }}</span>
        <small>{{ collection.requests.length }} 个接口</small>
      </button>
    </div>

    <div class="api-workbench-nav-section">
      <div class="api-workbench-nav-title">
        <span>接口树</span>
        <small v-if="selectedCollection">{{ selectedCollection.requests.length }} 个接口</small>
      </div>
      <el-input
        v-if="selectedCollection"
        v-model="searchQuery"
        size="small"
        clearable
        placeholder="搜索接口、Method、URL、文件夹"
      />

      <div v-if="selectedCollection && tree" class="api-workbench-nav-tree">
        <div
          v-if="tree.unassigned.requests.length > 0"
          class="api-workbench-nav-group"
          @contextmenu.prevent.stop="openMenu($event, { type: 'blank' })"
        >
          <span>未分组</span>
          <small>{{ tree.unassigned.requests.length }}</small>
        </div>
        <button
          v-for="request in tree.unassigned.requests"
          :key="'unassigned-' + request.id"
          type="button"
          class="api-workbench-request-row"
          :class="{ active: selectedRequestId === request.id }"
          @click="emit('openRequest', request.id)"
          @contextmenu.prevent.stop="
            openMenu($event, {
              type: 'request',
              collectionId: request.collectionId,
              requestId: request.id,
              folderId: null,
            })
          "
        >
          <strong>{{ request.method }}</strong>
          <span>{{ request.name }}</span>
        </button>

        <template v-for="row in visibleRows" :key="row.key">
          <button
            v-if="row.kind === 'folder'"
            type="button"
            class="api-workbench-folder-row"
            :style="{ paddingLeft: row.depth * 14 + 8 + 'px' }"
            :aria-expanded="row.expanded"
            @click="toggleFolder(row.folder.id)"
            @contextmenu.prevent.stop="
              openMenu($event, {
                type: 'folder',
                collectionId: row.folder.collectionId,
                folderId: row.folder.id,
              })
            "
          >
            <span class="api-workbench-folder-arrow">{{ row.expanded ? "v" : ">" }}</span>
            <span class="api-workbench-folder-name">{{ row.folder.name }}</span>
            <small>{{ row.childCount }}</small>
          </button>

          <button
            v-else
            type="button"
            class="api-workbench-request-row"
            :class="{ active: selectedRequestId === row.request.id }"
            :style="{ paddingLeft: row.depth * 14 + 8 + 'px' }"
            @click="emit('openRequest', row.request.id)"
            @contextmenu.prevent.stop="
              openMenu($event, {
                type: 'request',
                collectionId: row.request.collectionId,
                requestId: row.request.id,
                folderId: row.request.folderId,
              })
            "
          >
            <strong>{{ row.request.method }}</strong>
            <span>{{ row.request.name }}</span>
          </button>
        </template>

        <el-empty
          v-if="visibleCollection && visibleCollection.folders.length === 0 && visibleCollection.requests.length === 0"
          :description="searchQuery.trim() ? '当前集合无匹配接口' : '暂无接口'"
        />
      </div>

      <el-empty v-else-if="!loading" description="请选择集合" />
    </div>

    <ApiWorkbenchContextMenu
      :visible="menuVisible"
      :x="menuX"
      :y="menuY"
      :items="menuItems"
      @close="closeMenu"
      @select="selectMenuItem"
    />
  </aside>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import ApiWorkbenchContextMenu from "./ApiWorkbenchContextMenu.vue";
import type {
  ApiWorkbenchCollection,
  ApiWorkbenchMenuItem,
  ApiWorkbenchNavCommand,
  ApiWorkbenchNavTarget,
  ApiWorkbenchTreeFolderNode,
  ApiWorkbenchTreeRequestNode,
} from "../types/api-workbench";
import {
  buildApiWorkbenchNavMenuItems,
  buildApiWorkbenchTree,
  getApiWorkbenchFolderAncestorIds,
} from "../utils/apiWorkbenchTree";
import { filterApiWorkbenchCollection } from "../utils/apiWorkbenchSearch";

type ApiWorkbenchSidebarRow =
  | {
      kind: "folder";
      key: string;
      folder: ApiWorkbenchTreeFolderNode;
      depth: number;
      expanded: boolean;
      childCount: number;
    }
  | {
      kind: "request";
      key: string;
      request: ApiWorkbenchTreeRequestNode;
      depth: number;
    };

const props = defineProps<{
  collections: ApiWorkbenchCollection[];
  selectedCollectionId: number | null;
  selectedRequestId: number | null;
  loading: boolean;
}>();

const emit = defineEmits<{
  selectCollection: [collectionId: number];
  openRequest: [requestId: number];
  command: [command: ApiWorkbenchNavCommand, target: ApiWorkbenchNavTarget];
}>();

const expandedFolderKeys = ref(new Set<string>());
const menuVisible = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const menuItems = ref<ApiWorkbenchMenuItem[]>([]);
const menuTarget = ref<ApiWorkbenchNavTarget>({ type: "blank" });
const searchQuery = ref("");

const selectedCollection = computed(
  () => props.collections.find((item) => item.id === props.selectedCollectionId) ?? null,
);
const visibleCollection = computed(() =>
  selectedCollection.value
    ? filterApiWorkbenchCollection(selectedCollection.value, searchQuery.value)
    : null,
);
const searchActive = computed(() => searchQuery.value.trim().length > 0);
const tree = computed(() =>
  visibleCollection.value ? buildApiWorkbenchTree(visibleCollection.value) : null,
);

const visibleRows = computed(() => {
  const currentTree = tree.value;
  if (!currentTree) return [];
  const rows: ApiWorkbenchSidebarRow[] = [];
  for (const root of currentTree.roots) collectVisibleRows(root, 0, rows);
  return rows;
});

function folderKey(collectionId: number, folderId: number): string {
  return `${collectionId}:${folderId}`;
}

function collectVisibleRows(
  folder: ApiWorkbenchTreeFolderNode,
  depth: number,
  rows: ApiWorkbenchSidebarRow[],
) {
  const key = folderKey(folder.collectionId, folder.id);
  const expanded = searchActive.value || expandedFolderKeys.value.has(key);
  rows.push({
    kind: "folder",
    key,
    folder,
    depth,
    expanded,
    childCount: folder.children.length + folder.requests.length,
  });
  if (!expanded) return;
  for (const request of folder.requests) {
    rows.push({ kind: "request", key: `request:${request.id}`, request, depth: depth + 1 });
  }
  for (const child of folder.children) collectVisibleRows(child, depth + 1, rows);
}

function toggleFolder(folderId: number) {
  const collection = selectedCollection.value;
  if (!collection) return;
  const key = folderKey(collection.id, folderId);
  const next = new Set(expandedFolderKeys.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expandedFolderKeys.value = next;
}

function openMenu(event: MouseEvent, target: ApiWorkbenchNavTarget) {
  menuTarget.value = target;
  menuItems.value = buildApiWorkbenchNavMenuItems(target, {
    hasSelectedCollection: selectedCollection.value !== null,
  });
  menuX.value = event.clientX;
  menuY.value = event.clientY;
  menuVisible.value = true;
}

function closeMenu() {
  menuVisible.value = false;
}

function emitCommand(command: ApiWorkbenchNavCommand, target: ApiWorkbenchNavTarget) {
  emit("command", command, target);
}

function selectMenuItem(item: ApiWorkbenchMenuItem) {
  closeMenu();
  emit("command", item.key as ApiWorkbenchNavCommand, menuTarget.value);
}

defineExpose({
  expandFolder(folderId: number | null) {
    const collection = selectedCollection.value;
    if (!collection) return;
    const next = new Set(expandedFolderKeys.value);
    const ids = getApiWorkbenchFolderAncestorIds(collection.folders, folderId);
    for (const id of ids) next.add(folderKey(collection.id, id));
    if (folderId !== null) next.add(folderKey(collection.id, folderId));
    expandedFolderKeys.value = next;
  },
});
</script>

<style scoped>
.api-workbench-sidebar {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 12px;
  overflow: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
  padding: 12px;
}

.api-workbench-toolbar,
.api-workbench-nav-title,
.api-workbench-nav-group {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.api-workbench-collection-list,
.api-workbench-nav-tree {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.api-workbench-collection {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
  color: var(--el-text-color-primary);
  cursor: pointer;
  padding: 8px;
  text-align: left;
}

.api-workbench-collection:hover,
.api-workbench-collection.active {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}

.api-workbench-collection span,
.api-workbench-folder-name,
.api-workbench-request-row span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.api-workbench-collection small,
.api-workbench-nav-title small,
.api-workbench-nav-group small,
.api-workbench-folder-row small {
  flex: none;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.api-workbench-nav-section {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 8px;
}

.api-workbench-nav-title {
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.api-workbench-nav-group {
  min-height: 28px;
  border-radius: 6px;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  padding: 4px 8px;
}

.api-workbench-folder-row,
.api-workbench-request-row {
  display: grid;
  width: 100%;
  min-height: 32px;
  align-items: center;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--el-text-color-primary);
  cursor: pointer;
  padding: 6px 8px;
  text-align: left;
}

.api-workbench-folder-row {
  grid-template-columns: 16px minmax(0, 1fr) auto;
  gap: 6px;
}

.api-workbench-folder-row:hover,
.api-workbench-request-row:hover,
.api-workbench-request-row.active {
  background: var(--el-fill-color-light);
}

.api-workbench-folder-arrow {
  color: var(--el-text-color-secondary);
  font-family: var(--lc-font-mono);
  font-size: 11px;
}

.api-workbench-request-row {
  grid-template-columns: 48px minmax(0, 1fr);
  gap: 6px;
}

.api-workbench-request-row strong {
  display: inline-flex;
  min-width: 0;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  background: var(--el-fill-color);
  color: var(--el-color-primary);
  font-family: var(--lc-font-mono);
  font-size: 11px;
  line-height: 1;
  padding: 4px 5px;
}
</style>
