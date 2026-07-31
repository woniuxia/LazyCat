<template>
  <div class="json-tree-node" role="treeitem">
    <div class="json-tree-line" :data-key="node.key" @contextmenu.prevent.stop="onContextMenu">
      <button
        v-if="expandable"
        class="json-tree-toggle"
        type="button"
        :aria-label="expanded ? '折叠节点' : '展开节点'"
        :aria-expanded="expanded"
        @click.stop="$emit('toggle', node.key)"
      >
        <el-icon :size="11">
          <CaretBottom v-if="expanded" />
          <CaretRight v-else />
        </el-icon>
      </button>
      <span v-else class="json-tree-toggle-spacer" aria-hidden="true" />

      <template v-if="isEditingName">
        <input
          :ref="focusEditInput"
          v-model="editDraft"
          class="json-tree-edit-input is-name"
          type="text"
          spellcheck="false"
          placeholder="字段名"
          aria-label="编辑字段名"
          @keydown.enter.prevent="submitEdit"
          @keydown.esc.prevent="cancelEdit"
          @blur="cancelEdit"
          @click.stop
          @dblclick.stop
        />
        <span class="json-tree-separator">:</span>
      </template>
      <template v-else>
        <span
          v-if="node.label"
          class="json-tree-label"
          :class="labelHighlightClass"
          @dblclick.stop="onLabelDblclick"
          >{{ node.label }}</span
        >
        <span v-if="node.label" class="json-tree-separator">:</span>
      </template>

      <template v-if="isObjectLike">
        <span class="json-tree-bracket">{{ expanded ? openToken : collapsedToken }}</span>
        <span class="json-tree-summary">{{ node.summary }}</span>
      </template>
      <input
        v-else-if="isEditingValue"
        :ref="focusEditInput"
        v-model="editDraft"
        class="json-tree-edit-input"
        type="text"
        spellcheck="false"
        aria-label="编辑值"
        @keydown.enter.prevent="submitEdit"
        @keydown.esc.prevent="cancelEdit"
        @blur="cancelEdit"
        @click.stop
        @dblclick.stop
      />
      <span
        v-else
        class="json-tree-value"
        :class="[`is-${node.valueType}`, valueHighlightClass]"
        @dblclick.stop="onValueDblclick"
      >
        {{ node.summary }}
      </span>

      <button
        type="button"
        class="json-tree-more"
        aria-label="节点菜单"
        title="节点菜单"
        @click.stop="onMoreClick"
      >
        ⋯
      </button>
    </div>

    <template v-if="isObjectLike && expanded && expandable">
      <div class="json-tree-children" role="group">
        <JsonTreeNode
          v-for="child in node.children"
          :key="child.key"
          :node="child"
          :expanded-keys="expandedKeys"
          :matched-keys="matchedKeys"
          :active-match-key="activeMatchKey"
          :editable="editable"
          :editing-state="editingState"
          @toggle="$emit('toggle', $event)"
          @open-menu="$emit('open-menu', $event)"
          @request-edit="$emit('request-edit', $event)"
          @edit-submit="$emit('edit-submit', $event)"
          @edit-cancel="$emit('edit-cancel')"
        />
      </div>
      <div class="json-tree-line json-tree-close-line">
        <span class="json-tree-toggle-spacer" aria-hidden="true" />
        <span class="json-tree-bracket">{{ closeToken }}</span>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { CaretBottom, CaretRight } from "@element-plus/icons-vue";
import { formatJsonPrimitive, isJsonTreeExpandable } from "../../utils/jsonTreeView";
import type { JsonTreeNode as JsonTreeNodeModel } from "../../utils/jsonTreeView";
import { jsonTreeSearchMatchId } from "../../utils/jsonTreeSearch";
import type {
  JsonTreeNodeEditRequest,
  JsonTreeNodeEditSubmit,
  JsonTreeNodeMenuTarget,
} from "../../types/json-tree";
import type { JsonTreeEditingState } from "../../composables/useJsonTreeEditing";

defineOptions({ name: "JsonTreeNode" });

const props = withDefaults(
  defineProps<{
    node: JsonTreeNodeModel;
    expandedKeys: Set<string>;
    /** 命中标识集合(jsonTreeSearchMatchId 产物),区分 key/value 命中。 */
    matchedKeys?: Set<string>;
    /** 当前命中的标识,展示更强高亮。 */
    activeMatchKey?: string | null;
    editable?: boolean;
    /** 当前行内编辑态(整树唯一),key 命中本节点时渲染行内输入。 */
    editingState?: JsonTreeEditingState | null;
  }>(),
  {
    matchedKeys: () => new Set<string>(),
    activeMatchKey: null,
    editable: false,
    editingState: null,
  },
);

const emit = defineEmits<{
  toggle: [key: string];
  "open-menu": [target: JsonTreeNodeMenuTarget];
  "request-edit": [request: JsonTreeNodeEditRequest];
  "edit-submit": [payload: JsonTreeNodeEditSubmit];
  "edit-cancel": [];
}>();

function onContextMenu(event: MouseEvent) {
  emit("open-menu", { node: props.node, x: event.clientX, y: event.clientY });
}

function onMoreClick(event: MouseEvent) {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  emit("open-menu", { node: props.node, x: rect.left, y: rect.bottom + 2 });
}

const isObjectLike = computed(
  () => props.node.valueType === "object" || props.node.valueType === "array",
);
const expandable = computed(() => isJsonTreeExpandable(props.node));
const expanded = computed(() => props.expandedKeys.has(props.node.key));
const isArray = computed(() => props.node.valueType === "array");
const openToken = computed(() => (isArray.value ? "[" : "{"));
const closeToken = computed(() => (isArray.value ? "]" : "}"));
const collapsedToken = computed(() => {
  if (!props.node.childCount) return `${openToken.value}${closeToken.value}`;
  return isArray.value ? "[...]" : "{...}";
});

const isEditingThisNode = computed(() => props.editingState?.key === props.node.key);
const isEditingValue = computed(
  () => isEditingThisNode.value && props.editingState?.mode === "value",
);
const isEditingName = computed(
  () =>
    isEditingThisNode.value &&
    (props.editingState?.mode === "rename" || props.editingState?.mode === "insert-key"),
);

const editDraft = ref("");

watch(
  [isEditingValue, isEditingName],
  ([valueEditing, nameEditing]) => {
    if (valueEditing) {
      editDraft.value = formatJsonPrimitive(props.node.value, props.node.valueType);
    } else if (nameEditing) {
      const lastSegment = props.node.path[props.node.path.length - 1];
      editDraft.value = typeof lastSegment === "string" ? lastSegment : "";
    }
  },
  { immediate: true },
);

// 已聚焦的编辑输入框(非响应式缓存):函数 ref 在每次 patch 都会重调,
// 若重复 select() 会在每次输入后全选内容,导致下一个字符覆盖全文
let lastFocusedEditInput: HTMLInputElement | null = null;

/** 函数 ref 仅在元素首次出现时聚焦全选,不写任何响应式状态。 */
function focusEditInput(el: unknown) {
  if (!(el instanceof HTMLInputElement)) return;
  if (el === lastFocusedEditInput) return;
  lastFocusedEditInput = el;
  el.focus();
  el.select();
}

function submitEdit() {
  const mode = props.editingState?.mode;
  if (!mode) return;
  emit("edit-submit", { node: props.node, mode, text: editDraft.value });
}

function cancelEdit() {
  emit("edit-cancel");
}

function onValueDblclick() {
  if (!props.editable || isObjectLike.value) return;
  emit("request-edit", { node: props.node, mode: "value" });
}

function onLabelDblclick() {
  if (!props.editable) return;
  const lastSegment = props.node.path[props.node.path.length - 1];
  if (typeof lastSegment !== "string") return;
  emit("request-edit", { node: props.node, mode: "rename" });
}

const labelMatchId = computed(() => jsonTreeSearchMatchId({ field: "key", key: props.node.key }));
const valueMatchId = computed(() => jsonTreeSearchMatchId({ field: "value", key: props.node.key }));
// 编辑中的行不显示命中高亮,结算后重算
const labelHighlightClass = computed(() => {
  if (isEditingThisNode.value) return {};
  return {
    "json-tree-match": props.matchedKeys.has(labelMatchId.value),
    "json-tree-match-active": props.activeMatchKey === labelMatchId.value,
  };
});
const valueHighlightClass = computed(() => {
  if (isEditingThisNode.value) return {};
  return {
    "json-tree-match": props.matchedKeys.has(valueMatchId.value),
    "json-tree-match-active": props.activeMatchKey === valueMatchId.value,
  };
});
</script>

<style scoped>
.json-tree-node {
  min-width: max-content;
}

.json-tree-line {
  display: flex;
  min-height: 22px;
  align-items: center;
  gap: 6px;
  white-space: pre-wrap;
  word-break: break-word;
}

.json-tree-children {
  margin-left: 18px;
  padding-left: 10px;
  border-left: 1px solid #e4eaf3;
}

.json-tree-toggle,
.json-tree-toggle-spacer {
  width: 17px;
  height: 17px;
  flex: 0 0 17px;
}

.json-tree-toggle {
  display: inline-grid;
  place-items: center;
  padding: 0;
  border: 1px solid #ccd6e6;
  border-radius: 5px;
  background: #ffffff;
  color: #536176;
  cursor: pointer;
  line-height: 1;
  transition:
    border-color 0.14s ease,
    background-color 0.14s ease,
    color 0.14s ease;
}

.json-tree-toggle:hover {
  border-color: #8ca6d8;
  background: #eef4ff;
  color: #1f4e9e;
}

.json-tree-label {
  color: #7b3f10;
}

.json-tree-separator,
.json-tree-bracket {
  color: #6c778a;
}

.json-tree-summary {
  display: inline-flex;
  align-items: center;
  height: 18px;
  margin-left: 2px;
  padding: 0 6px;
  border: 1px solid #e3e8f1;
  border-radius: 999px;
  background: #f5f7fb;
  color: #6c778a;
  font-size: 11px;
  line-height: 1;
}

.json-tree-value.is-string {
  color: #0f766e;
}

.json-tree-value.is-number {
  color: #2454a6;
}

.json-tree-value.is-boolean,
.json-tree-value.is-null {
  color: #7c3aed;
}

.json-tree-value.is-unknown {
  color: #b45309;
}

.json-tree-edit-input {
  min-width: 120px;
  max-width: 360px;
  height: 20px;
  padding: 0 6px;
  border: 1px solid #8ca6d8;
  border-radius: 4px;
  background: #ffffff;
  color: #263247;
  font: inherit;
  font-size: 12px;
  line-height: 1;
  outline: none;
}

.json-tree-edit-input.is-name {
  min-width: 90px;
  max-width: 220px;
  color: #7b3f10;
}

.json-tree-edit-input:focus {
  border-color: #1f4e9e;
  box-shadow: 0 0 0 2px rgba(31, 78, 158, 0.12);
}

.json-tree-more {
  display: inline-grid;
  width: 20px;
  height: 17px;
  flex: 0 0 auto;
  place-items: center;
  margin-left: 2px;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 5px;
  background: transparent;
  color: #536176;
  cursor: pointer;
  font-size: 12px;
  line-height: 1;
  opacity: 0;
  transition:
    opacity 0.12s ease,
    border-color 0.14s ease,
    background-color 0.14s ease,
    color 0.14s ease;
}

.json-tree-line:hover .json-tree-more,
.json-tree-more:focus-visible {
  opacity: 1;
}

.json-tree-more:hover {
  border-color: #8ca6d8;
  background: #eef4ff;
  color: #1f4e9e;
}

.json-tree-match {
  border-radius: 4px;
  background: #fdf0c7;
  box-shadow: 0 0 0 1px #f3dda0;
}

.json-tree-match-active {
  background: #fbca4e;
  box-shadow: 0 0 0 1px #d9990a;
  color: #52340a;
}

.json-tree-close-line {
  color: #6c778a;
}
</style>
