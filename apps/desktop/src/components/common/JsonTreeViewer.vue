<template>
  <section class="json-tree-viewer" :aria-label="ariaLabel">
    <div v-if="showToolbar" class="json-tree-toolbar">
      <el-button size="small" :icon="CopyDocument" @click="copyJson">复制</el-button>
      <template v-if="hasExpandableNodes">
        <el-button size="small" :icon="Expand" @click="expandAll">展开全部</el-button>
        <el-button size="small" :icon="Fold" @click="collapseAll">折叠全部</el-button>
        <el-button size="small" @click="foldToTwoLevels">折到 2 层</el-button>
      </template>
    </div>

    <div class="json-tree-body" role="tree">
      <JsonTreeNode
        :node="tree"
        :expanded-keys="expandedKeys"
        @toggle="toggleNode"
      />
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { CopyDocument, Expand, Fold } from "@element-plus/icons-vue";
import JsonTreeNode from "./JsonTreeNode.vue";
import {
  buildJsonTree,
  collectExpandableKeys,
  collectExpandedKeysByDepth,
  formatJsonForCopy,
  isJsonTreeExpandable,
} from "../../utils/jsonTreeView";

interface JsonTreeViewerProps {
  value: unknown;
  defaultExpandDepth?: number | "all";
  showToolbar?: boolean;
  copyText?: string;
  ariaLabel?: string;
}

const props = withDefaults(defineProps<JsonTreeViewerProps>(), {
  defaultExpandDepth: "all",
  showToolbar: true,
  ariaLabel: "JSON 内容",
});

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
  try {
    await navigator.clipboard.writeText(props.copyText ?? formatJsonForCopy(props.value));
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
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
  gap: 6px;
  padding: 8px;
  border-bottom: 1px solid #e7eaf1;
  background: #ffffff;
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
