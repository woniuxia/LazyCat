<template>
  <el-dialog
    v-model="visible"
    title="配置菜单显示项"
    width="480px"
    :close-on-click-modal="false"
    @open="onOpen"
  >
    <p style="margin-bottom: 12px; color: var(--el-text-color-secondary); font-size: 13px;">
      取消勾选的工具将从侧边栏隐藏，但仍可通过首页、收藏或标签页访问。
    </p>
    <el-tree
      ref="treeRef"
      :data="treeData"
      show-checkbox
      default-expand-all
      node-key="id"
      :props="{ label: 'label', children: 'children' }"
    />
    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" @click="onSave">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { ElMessage } from "element-plus";
import type { ElTree } from "element-plus";
import type { SidebarItem } from "../types";

interface TreeNode {
  id: string;
  label: string;
  children?: TreeNode[];
}

const props = defineProps<{
  sidebarItems: SidebarItem[];
  getHiddenIds: () => string[];
  setHiddenIds: (ids: string[]) => void;
}>();

const visible = ref(false);
const treeRef = ref<InstanceType<typeof ElTree>>();

const treeData = computed<TreeNode[]>(() => {
  return props.sidebarItems.map((item) => {
    if (item.kind === "tool") {
      return { id: item.tool.id, label: item.tool.name };
    }
    return {
      id: item.group.id,
      label: item.group.name,
      children: item.group.tools.map((t) => ({ id: t.id, label: t.name })),
    };
  });
});

/** 收集所有叶节点（工具）ID */
function allLeafIds(): string[] {
  const ids: string[] = [];
  for (const item of props.sidebarItems) {
    if (item.kind === "tool") {
      ids.push(item.tool.id);
    } else {
      for (const t of item.group.tools) {
        ids.push(t.id);
      }
    }
  }
  return ids;
}

function onOpen() {
  const hidden = new Set(props.getHiddenIds());
  const checkedLeaves = allLeafIds().filter((id) => !hidden.has(id));
  // nextTick needed because el-tree renders after dialog open
  setTimeout(() => {
    treeRef.value?.setCheckedKeys(checkedLeaves, false);
  }, 0);
}

function onSave() {
  const checkedLeaves = treeRef.value?.getCheckedKeys(true) as string[] ?? [];
  if (checkedLeaves.length === 0) {
    ElMessage.warning("至少保留一个工具可见");
    return;
  }
  const checkedSet = new Set(checkedLeaves);
  const hiddenIds = allLeafIds().filter((id) => !checkedSet.has(id));
  props.setHiddenIds(hiddenIds);
  visible.value = false;
  ElMessage.success("菜单显示配置已保存");
}

function show() {
  visible.value = true;
}

defineExpose({ show });
</script>
