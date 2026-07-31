<template>
  <el-dialog
    :model-value="modelValue"
    title="基础数据设置"
    width="920px"
    :close-on-click-modal="false"
    @update:model-value="(v) => emit('update:modelValue', v)"
  >
    <div class="basic-grid">
      <el-card>
        <template #header>
          <div class="card-header">
            <span>事项分类</span>
            <el-button text type="primary" @click="addType">新增</el-button>
          </div>
        </template>
        <el-table :data="types" size="small" border>
          <el-table-column prop="name" label="名称" min-width="120" />
          <el-table-column prop="color" label="颜色" width="110">
            <template #default="{ row }">
              <span class="color-dot" :style="{ backgroundColor: row.color || '#409eff' }" />
              {{ row.color || "-" }}
            </template>
          </el-table-column>
          <el-table-column label="操作" width="160">
            <template #default="{ row }">
              <el-button size="small" text @click="renameType(row)">编辑</el-button>
              <el-button size="small" text type="danger" @click="removeType(row)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>
      </el-card>

      <el-card>
        <template #header>
          <div class="card-header">
            <span>执行人</span>
            <el-button text type="primary" @click="addAssignee">新增</el-button>
          </div>
        </template>
        <el-table :data="assignees" size="small" border>
          <el-table-column prop="name" label="名称" min-width="120" />
          <el-table-column label="操作" width="160">
            <template #default="{ row }">
              <el-button size="small" text @click="renameAssignee(row)">编辑</el-button>
              <el-button size="small" text type="danger" @click="removeAssignee(row)"
                >删除</el-button
              >
            </template>
          </el-table-column>
        </el-table>
      </el-card>
    </div>
  </el-dialog>

  <el-dialog
    v-model="typeDialogVisible"
    :title="typeDialogTitle"
    width="480px"
    @closed="resetTypeDraft"
  >
    <el-form label-width="72px">
      <el-form-item label="名称">
        <el-input v-model.trim="typeDraft.name" placeholder="请输入分类名称" />
      </el-form-item>
      <el-form-item label="颜色">
        <el-input v-model.trim="typeDraft.color" placeholder="例如：#409eff" />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="typeDialogVisible = false">取消</el-button>
      <el-button type="primary" @click="saveType">保存</el-button>
    </template>
  </el-dialog>

  <el-dialog
    v-model="assigneeDialogVisible"
    :title="assigneeDialogTitle"
    width="420px"
    @closed="resetAssigneeDraft"
  >
    <el-form label-width="72px">
      <el-form-item label="名称">
        <el-input v-model.trim="assigneeDraft.name" placeholder="请输入执行人名称" />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="assigneeDialogVisible = false">取消</el-button>
      <el-button type="primary" @click="saveAssignee">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../../bridge/tauri";
import type { TodoAssignee, TodoType } from "../../types";

interface TodoTypeDraft {
  id: number;
  name: string;
  color: string;
  sortOrder: number;
}

interface TodoAssigneeDraft {
  id: number;
  name: string;
}

const props = defineProps<{
  modelValue: boolean;
  types: TodoType[];
  assignees: TodoAssignee[];
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  refresh: [];
}>();

const typeDialogVisible = ref(false);
const assigneeDialogVisible = ref(false);
const typeDraft = reactive<TodoTypeDraft>({ id: 0, name: "", color: "", sortOrder: 0 });
const assigneeDraft = reactive<TodoAssigneeDraft>({ id: 0, name: "" });

const typeDialogTitle = computed(() => (typeDraft.id ? "编辑分类" : "新增分类"));
const assigneeDialogTitle = computed(() => (assigneeDraft.id ? "编辑执行人" : "新增执行人"));

function getNextTypeSortOrder() {
  const max = props.types.reduce((acc, t) => (t.sortOrder > acc ? t.sortOrder : acc), 0);
  return max + 10;
}

function resetTypeDraft() {
  typeDraft.id = 0;
  typeDraft.name = "";
  typeDraft.color = "";
  typeDraft.sortOrder = getNextTypeSortOrder();
}

function addType() {
  resetTypeDraft();
  typeDialogVisible.value = true;
}

function renameType(item: TodoType) {
  typeDraft.id = item.id;
  typeDraft.name = item.name;
  typeDraft.color = item.color;
  typeDraft.sortOrder = item.sortOrder;
  typeDialogVisible.value = true;
}

async function saveType() {
  const name = typeDraft.name.trim();
  if (!name) {
    ElMessage.warning("请输入分类名称");
    return;
  }
  try {
    await invokeToolByChannel(
      "tool:todo:type-upsert",
      typeDraft.id
        ? { id: typeDraft.id, name, color: typeDraft.color, sortOrder: typeDraft.sortOrder }
        : {
            name,
            color: typeDraft.color,
            sortOrder: typeDraft.sortOrder || getNextTypeSortOrder(),
          },
    );
    typeDialogVisible.value = false;
    resetTypeDraft();
    emit("refresh");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function removeType(item: TodoType) {
  try {
    await ElMessageBox.confirm(`确认删除分类「${item.name}」吗？`, "删除确认", { type: "warning" });
    await invokeToolByChannel("tool:todo:type-delete", { id: item.id });
    emit("refresh");
  } catch (error) {
    if ((error as Error).message !== "cancel") ElMessage.error((error as Error).message);
  }
}

function resetAssigneeDraft() {
  assigneeDraft.id = 0;
  assigneeDraft.name = "";
}

function addAssignee() {
  resetAssigneeDraft();
  assigneeDialogVisible.value = true;
}

function renameAssignee(item: TodoAssignee) {
  assigneeDraft.id = item.id;
  assigneeDraft.name = item.name;
  assigneeDialogVisible.value = true;
}

async function saveAssignee() {
  const name = assigneeDraft.name.trim();
  if (!name) {
    ElMessage.warning("请输入执行人名称");
    return;
  }
  try {
    await invokeToolByChannel(
      "tool:todo:assignee-upsert",
      assigneeDraft.id ? { id: assigneeDraft.id, name } : { name },
    );
    assigneeDialogVisible.value = false;
    resetAssigneeDraft();
    emit("refresh");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function removeAssignee(item: TodoAssignee) {
  try {
    await ElMessageBox.confirm(`确认删除执行人「${item.name}」吗？`, "删除确认", {
      type: "warning",
    });
    await invokeToolByChannel("tool:todo:assignee-delete", { id: item.id });
    emit("refresh");
  } catch (error) {
    if ((error as Error).message !== "cancel") ElMessage.error((error as Error).message);
  }
}
</script>

<style>
.basic-grid {
  display: grid;
  grid-template-columns: minmax(420px, 1.45fr) minmax(280px, 1fr);
  gap: 12px;
  align-items: start;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.color-dot {
  display: inline-block;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  margin-right: 6px;
  vertical-align: middle;
  border: 1px solid var(--el-border-color-light);
}

@media (max-width: 900px) {
  .basic-grid {
    grid-template-columns: 1fr;
  }
}
</style>
