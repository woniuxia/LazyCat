<template>
  <section class="json-tree-viewer" :aria-label="ariaLabel" tabindex="-1" @keydown="onRootKeydown">
    <div v-if="showToolbar" class="json-tree-toolbar">
      <el-button size="small" :icon="CopyDocument" @click="copyJson">复制</el-button>
      <template v-if="hasExpandableNodes">
        <el-button size="small" :icon="Expand" @click="expandAll">展开全部</el-button>
        <el-button size="small" :icon="Fold" @click="collapseAll">折叠全部</el-button>
        <el-button size="small" @click="foldToTwoLevels">折到 2 层</el-button>
      </template>
      <template v-if="editable">
        <el-button
          size="small"
          :icon="RefreshLeft"
          :disabled="!canUndo"
          aria-label="撤销"
          title="撤销 (Ctrl+Z)"
          @click="undo"
        />
        <el-button
          size="small"
          :icon="RefreshRight"
          :disabled="!canRedo"
          aria-label="重做"
          title="重做 (Ctrl+Y)"
          @click="redo"
        />
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
        :editable="editable"
        :editing-state="editingState"
        @toggle="toggleNode"
        @open-menu="openNodeMenu"
        @request-edit="onRequestEdit"
        @edit-submit="onEditSubmit"
        @edit-cancel="cancelEditing"
      />
    </div>

    <JsonTreeNodeMenu
      :visible="menuVisible"
      :x="menuX"
      :y="menuY"
      :node="menuNode"
      :parent="menuParent"
      :editable="editable"
      @close="closeNodeMenu"
      @action="onMenuAction"
    />
  </section>
</template>

<script setup lang="ts">
/**
 * 通用 JSON 树查看/编辑组件。
 *
 * 受控契约(editable 模式):
 * - 组件不持有文档;编辑成功仅 emit update:value(新根,基于旧根结构共享)。
 * - 消费方必须把 emit 的根对象原样回写 value(不得 clone/深转换),否则每次编辑
 *   都会被判定为外部换文档,展开状态与撤销栈被清空;建议以 shallowRef 持有文档。
 * - 编辑回流经 toRaw 引用比较判定,消费方用 ref/reactive 包装也能正确识别。
 * - 编辑产物是标准 JSON 值;数字按 JS number 语义,超出 Number.MAX_SAFE_INTEGER
 *   的大整数在编辑/序列化后丢失精度。
 * - 无虚拟滚动:超大文档依赖默认折叠深度与消费方体积闸门,"展开全部"可能长时间阻塞渲染。
 */
import { computed, nextTick, ref, shallowRef, toRaw, watch } from "vue";
import { ElMessage } from "element-plus";
import {
  ArrowDown,
  ArrowUp,
  CopyDocument,
  Expand,
  Fold,
  RefreshLeft,
  RefreshRight,
} from "@element-plus/icons-vue";
import JsonTreeNode from "./JsonTreeNode.vue";
import JsonTreeNodeMenu from "./JsonTreeNodeMenu.vue";
import { useJsonTreeSearch } from "../../composables/useJsonTreeSearch";
import { useJsonTreeEditing } from "../../composables/useJsonTreeEditing";
import type { JsonTreeEditingMode } from "../../composables/useJsonTreeEditing";
import type {
  JsonTreeNodeEditRequest,
  JsonTreeNodeEditSubmit,
  JsonTreeNodeMenuAction,
  JsonTreeNodeMenuTarget,
} from "../../types/json-tree";
import {
  buildJsonTree,
  collectExpandableKeys,
  collectExpandedKeysByDepth,
  encodeJsonTreePath,
  findJsonTreeParentNode,
  formatJsonForCopy,
  isJsonTreeExpandable,
  toJsonPath,
} from "../../utils/jsonTreeView";
import type { JsonTreeNode as JsonTreeNodeModel } from "../../utils/jsonTreeView";
import { defaultJsonValueForType, parseLooseJsonInput } from "../../utils/jsonTreeEdit";

interface JsonTreeViewerProps {
  value: unknown;
  defaultExpandDepth?: number | "all";
  showToolbar?: boolean;
  copyText?: string;
  ariaLabel?: string;
  showSearch?: boolean;
  editable?: boolean;
}

const props = withDefaults(defineProps<JsonTreeViewerProps>(), {
  defaultExpandDepth: "all",
  showToolbar: true,
  ariaLabel: "JSON 内容",
  showSearch: true,
  editable: false,
});

const emit = defineEmits<{
  "update:value": [value: unknown];
}>();

const bodyRef = ref<HTMLElement | null>(null);
// 内部一律基于 toRaw 后的原始对象,避免 raw/proxy 混合子树破坏结构共享
const tree = computed(() => buildJsonTree(toRaw(props.value)));
const expandedKeys = ref<Set<string>>(new Set());
const hasExpandableNodes = computed(() => collectExpandableKeys(tree.value).size > 0);

function resetExpandedByDepth() {
  expandedKeys.value = collectExpandedKeysByDepth(tree.value, props.defaultExpandDepth);
}

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

// 菜单状态必须在下方 immediate watch 之前初始化:
// onValueChange 首次触发即调用 handleExternalDocument -> closeNodeMenu,声明靠后会命中 TDZ
const menuVisible = ref(false);
const menuX = ref(0);
const menuY = ref(0);
// 菜单目标节点:关闭时保留引用给离场动画,下次打开覆盖
const menuNode = shallowRef<JsonTreeNodeModel | null>(null);
const menuParent = shallowRef<JsonTreeNodeModel | null>(null);

function openNodeMenu(target: JsonTreeNodeMenuTarget) {
  menuNode.value = target.node;
  menuParent.value = findJsonTreeParentNode(tree.value, target.node.path);
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

const {
  editing: editingState,
  canUndo,
  canRedo,
  applyOp,
  undo,
  redo,
  beginEdit,
  endEdit,
  cancelEditing,
  onValueChange,
} = useJsonTreeEditing({
  getValue: () => props.value,
  expandedKeys,
  emitValue: (value) => emit("update:value", value),
  onExternalDocument: handleExternalDocument,
});

/** 外部换文档:重置展开、清空搜索词与命中、取消进行中交互。 */
function handleExternalDocument() {
  resetExpandedByDepth();
  searchQuery.value = "";
  closeNodeMenu();
}

// 编辑回流保持展开与搜索;外部换文档走 handleExternalDocument 重置
watch(() => props.value, onValueChange, { immediate: true });

watch(
  () => props.defaultExpandDepth,
  () => {
    resetExpandedByDepth();
  },
);

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

/** 切换编辑目标前先结算当前编辑(空 key 插入取消即回滚)。 */
function startEdit(key: string, mode: JsonTreeEditingMode) {
  if (editingState.value) cancelEditing();
  beginEdit(key, mode);
}

function expandNodeKey(key: string) {
  if (!expandedKeys.value.has(key)) {
    expandedKeys.value = new Set([...expandedKeys.value, key]);
  }
}

function onRequestEdit(request: JsonTreeNodeEditRequest) {
  if (!props.editable) return;
  startEdit(request.node.key, request.mode);
}

function onEditSubmit(payload: JsonTreeNodeEditSubmit) {
  const { node, mode, text } = payload;
  if (mode === "value") {
    if (applyOp({ type: "set-value", path: node.path, value: parseLooseJsonInput(text) })) {
      endEdit();
    }
    return;
  }
  // rename / insert-key:空名插入提交视为取消(回滚),同名重命名视为无操作
  const currentKey = String(node.path[node.path.length - 1] ?? "");
  if (mode === "insert-key" && !text) {
    cancelEditing();
    return;
  }
  if (text === currentKey) {
    endEdit();
    return;
  }
  if (applyOp({ type: "rename-key", path: node.path, newKey: text })) {
    endEdit();
  }
  // 失败(如重名):保留编辑态让用户修正或 Esc 取消
}

async function onMenuAction(action: JsonTreeNodeMenuAction) {
  const node = menuNode.value;
  closeNodeMenu();
  if (!node) return;
  switch (action.kind) {
    case "copy-path":
      await copyToClipboard(toJsonPath(node.path));
      break;
    case "copy-value":
      await copyToClipboard(formatJsonForCopy(node.value));
      break;
    case "edit-value":
      startEdit(node.key, "value");
      break;
    case "rename-key":
      startEdit(node.key, "rename");
      break;
    case "add-child":
      if (node.valueType === "object") {
        if (applyOp({ type: "insert", parentPath: node.path, key: "", value: null })) {
          expandNodeKey(node.key);
          startEdit(encodeJsonTreePath([...node.path, ""]), "insert-key");
        }
      } else if (node.valueType === "array") {
        const index = node.childCount;
        if (applyOp({ type: "insert", parentPath: node.path, index, value: null })) {
          expandNodeKey(node.key);
          startEdit(encodeJsonTreePath([...node.path, index]), "value");
        }
      }
      break;
    case "insert-before":
    case "insert-after": {
      const lastSegment = node.path[node.path.length - 1];
      if (typeof lastSegment !== "number") break;
      const index = action.kind === "insert-after" ? lastSegment + 1 : lastSegment;
      const parentPath = node.path.slice(0, -1);
      if (applyOp({ type: "insert", parentPath, index, value: null })) {
        startEdit(encodeJsonTreePath([...parentPath, index]), "value");
      }
      break;
    }
    case "switch-type":
      applyOp({
        type: "set-value",
        path: node.path,
        value: defaultJsonValueForType(action.valueType),
      });
      break;
    case "move-up":
      applyOp({ type: "move", path: node.path, offset: -1 });
      break;
    case "move-down":
      applyOp({ type: "move", path: node.path, offset: 1 });
      break;
    case "remove":
      applyOp({ type: "remove", path: node.path });
      break;
  }
}

/** Ctrl+Z / Ctrl+Y(含 Ctrl+Shift+Z);输入框聚焦时让位原生撤销。 */
function onRootKeydown(event: KeyboardEvent) {
  if (!props.editable) return;
  const target = event.target;
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
  if (!(event.ctrlKey || event.metaKey)) return;
  const key = event.key.toLowerCase();
  if (key === "z" && event.shiftKey) {
    event.preventDefault();
    redo();
  } else if (key === "z") {
    event.preventDefault();
    undo();
  } else if (key === "y") {
    event.preventDefault();
    redo();
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

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
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
  outline: none;
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
