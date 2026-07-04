<template>
  <section class="json-tree-viewer" :aria-label="ariaLabel">
    <div v-if="showToolbar" class="json-tree-toolbar">
      <el-button size="small" :icon="CopyDocument" @click="copyJson">复制</el-button>
      <template v-if="hasExpandableNodes">
        <el-button size="small" :icon="Expand" @click="expandAll">展开全部</el-button>
        <el-button size="small" :icon="Fold" @click="collapseAll">折叠全部</el-button>
        <el-button size="small" @click="foldToTwoLevels">折到 2 层</el-button>
      </template>
      <div v-if="showSearch" class="json-tree-search">
        <el-input
          v-model="searchQuery"
          class="json-tree-search-input"
          size="small"
          clearable
          placeholder="搜索 key / 值"
          aria-label="树内搜索"
          @keydown.enter.exact.prevent="goToNextMatch"
          @keydown.shift.enter.prevent="goToPrevMatch"
        />
        <span
          v-if="searchCountText"
          class="json-tree-search-count"
          :class="{ 'is-miss': !searchMatches.length }"
        >
          {{ searchCountText }}
        </span>
        <el-button
          size="small"
          :icon="ArrowUp"
          :disabled="!searchMatches.length"
          aria-label="上一处"
          title="上一处 (Shift+Enter)"
          @click="goToPrevMatch"
        />
        <el-button
          size="small"
          :icon="ArrowDown"
          :disabled="!searchMatches.length"
          aria-label="下一处"
          title="下一处 (Enter)"
          @click="goToNextMatch"
        />
      </div>
    </div>

    <div ref="bodyRef" class="json-tree-body" role="tree">
      <JsonTreeNode
        :node="tree"
        :expanded-keys="expandedKeys"
        :matched-keys="searchMatchedIds"
        :active-match-key="searchActiveMatchId"
        @toggle="toggleNode"
        @open-menu="openNodeMenu"
      />
    </div>

    <JsonTreeNodeMenu
      :visible="menuVisible"
      :x="menuX"
      :y="menuY"
      @close="closeNodeMenu"
      @action="onMenuAction"
    />
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, shallowRef, watch } from "vue";
import { ElMessage } from "element-plus";
import { ArrowDown, ArrowUp, CopyDocument, Expand, Fold } from "@element-plus/icons-vue";
import JsonTreeNode from "./JsonTreeNode.vue";
import JsonTreeNodeMenu from "./JsonTreeNodeMenu.vue";
import { useJsonTreeSearch } from "../../composables/useJsonTreeSearch";
import type {
  JsonTreeNodeMenuAction,
  JsonTreeNodeMenuTarget,
} from "../../types/json-tree";
import {
  buildJsonTree,
  collectExpandableKeys,
  collectExpandedKeysByDepth,
  formatJsonForCopy,
  isJsonTreeExpandable,
  toJsonPath,
} from "../../utils/jsonTreeView";
import type { JsonTreeNode as JsonTreeNodeModel } from "../../utils/jsonTreeView";

interface JsonTreeViewerProps {
  value: unknown;
  defaultExpandDepth?: number | "all";
  showToolbar?: boolean;
  copyText?: string;
  ariaLabel?: string;
  showSearch?: boolean;
}

const props = withDefaults(defineProps<JsonTreeViewerProps>(), {
  defaultExpandDepth: "all",
  showToolbar: true,
  ariaLabel: "JSON 内容",
  showSearch: true,
});

const bodyRef = ref<HTMLElement | null>(null);
const tree = computed(() => buildJsonTree(props.value));
const expandedKeys = ref<Set<string>>(new Set());
const hasExpandableNodes = computed(() => collectExpandableKeys(tree.value).size > 0);

watch(
  [tree, () => props.defaultExpandDepth],
  () => {
    expandedKeys.value = collectExpandedKeysByDepth(tree.value, props.defaultExpandDepth);
  },
  { immediate: true },
);

const {
  query: searchQuery,
  matches: searchMatches,
  activeIndex: searchActiveIndex,
  activeKey: searchActiveKey,
  activeMatchId: searchActiveMatchId,
  matchedIds: searchMatchedIds,
  revealKeys: searchRevealKeys,
  goNext: searchGoNext,
  goPrev: searchGoPrev,
} = useJsonTreeSearch(tree);

const searchCountText = computed(() => {
  if (!searchQuery.value) return "";
  if (!searchMatches.value.length) return "无匹配";
  return `第 ${searchActiveIndex.value + 1}/${searchMatches.value.length} 处`;
});

function goToNextMatch() {
  searchGoNext();
  revealActiveMatch();
}

function goToPrevMatch() {
  searchGoPrev();
  revealActiveMatch();
}

function escapeAttrValue(value: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") return CSS.escape(value);
  return value.replace(/["\\]/g, "\\$&");
}

function revealActiveMatch() {
  const key = searchActiveKey.value;
  if (!key) return;
  if (searchRevealKeys.value.size) {
    expandedKeys.value = new Set([...expandedKeys.value, ...searchRevealKeys.value]);
  }
  void nextTick(() => {
    bodyRef.value
      ?.querySelector(`[data-key="${escapeAttrValue(key)}"]`)
      ?.scrollIntoView({ block: "nearest" });
  });
}

watch(searchActiveKey, (key) => {
  if (key) revealActiveMatch();
});

const menuVisible = ref(false);
const menuX = ref(0);
const menuY = ref(0);
// 菜单目标节点:关闭时保留引用给离场动画,下次打开覆盖
const menuNode = shallowRef<JsonTreeNodeModel | null>(null);

function openNodeMenu(target: JsonTreeNodeMenuTarget) {
  menuNode.value = target.node;
  menuX.value = target.x;
  menuY.value = target.y;
  menuVisible.value = true;
}

function closeNodeMenu() {
  menuVisible.value = false;
}

// 菜单打开期间文档变化:目标节点已失效,关闭菜单丢弃交互
watch(tree, () => {
  if (menuVisible.value) closeNodeMenu();
});

async function onMenuAction(action: JsonTreeNodeMenuAction) {
  const node = menuNode.value;
  closeNodeMenu();
  if (!node) return;
  if (action.kind === "copy-path") {
    await copyToClipboard(toJsonPath(node.path));
  } else if (action.kind === "copy-value") {
    await copyToClipboard(formatJsonForCopy(node.value));
  }
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}

function toggleNode(key: string) {
  const next = new Set(expandedKeys.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expandedKeys.value = next;
}

function expandAll() {
  expandedKeys.value = collectExpandableKeys(tree.value);
}

function collapseAll() {
  const next = new Set<string>();
  if (isJsonTreeExpandable(tree.value)) next.add(tree.value.key);
  expandedKeys.value = next;
}

function foldToTwoLevels() {
  expandedKeys.value = collectExpandedKeysByDepth(tree.value, 2);
}

async function copyJson() {
  await copyToClipboard(props.copyText ?? formatJsonForCopy(props.value));
}
</script>

<style scoped>
.json-tree-viewer {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid #e7eaf1;
  border-radius: 8px;
  background: #fbfcff;
  color: #263247;
  font-family: "Cascadia Mono", Consolas, monospace;
}

.json-tree-toolbar {
  display: flex;
  flex: 0 0 auto;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 8px;
  border-bottom: 1px solid #e7eaf1;
  background: #ffffff;
}

.json-tree-search {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

.json-tree-search :deep(.el-button + .el-button) {
  margin-left: 0;
}

.json-tree-search-input {
  width: 168px;
}

.json-tree-search-count {
  flex: 0 0 auto;
  color: #6c778a;
  font-size: 11px;
  white-space: nowrap;
}

.json-tree-search-count.is-miss {
  color: #c2410c;
}

.json-tree-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  padding: 10px 12px 12px 10px;
  font-size: 12px;
  line-height: 1.55;
}
</style>
