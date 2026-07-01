<template>
  <div class="json-tree-node" role="treeitem">
    <div class="json-tree-line">
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

      <span v-if="node.label" class="json-tree-label">{{ node.label }}</span>
      <span v-if="node.label" class="json-tree-separator">:</span>

      <template v-if="isObjectLike">
        <span class="json-tree-bracket">{{ expanded ? openToken : collapsedToken }}</span>
        <span class="json-tree-summary">{{ node.summary }}</span>
      </template>
      <span v-else class="json-tree-value" :class="`is-${node.valueType}`">
        {{ node.summary }}
      </span>
    </div>

    <template v-if="isObjectLike && expanded && expandable">
      <div class="json-tree-children" role="group">
        <JsonTreeNode
          v-for="child in node.children"
          :key="child.key"
          :node="child"
          :expanded-keys="expandedKeys"
          @toggle="$emit('toggle', $event)"
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
import { computed } from "vue";
import { CaretBottom, CaretRight } from "@element-plus/icons-vue";
import { isJsonTreeExpandable } from "../../utils/jsonTreeView";
import type { JsonTreeNode as JsonTreeNodeModel } from "../../utils/jsonTreeView";

defineOptions({ name: "JsonTreeNode" });

const props = defineProps<{
  node: JsonTreeNodeModel;
  expandedKeys: Set<string>;
}>();

defineEmits<{
  toggle: [key: string];
}>();

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

.json-tree-close-line {
  color: #6c778a;
}
</style>
