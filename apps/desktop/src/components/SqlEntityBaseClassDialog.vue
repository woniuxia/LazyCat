<template>
  <el-dialog
    v-model="visible"
    title="基类管理"
    width="760px"
    destroy-on-close
    class="sql-entity-base-class-dialog"
    @closed="resetView"
  >
    <div v-if="mode === 'list'" class="manager-list">
      <div class="manager-toolbar">
        <div>
          <div class="manager-title">Java 基类</div>
          <div class="manager-hint">配置可复用的父类及其已经包含的属性。</div>
        </div>
        <el-button type="primary" @click="startCreate">新增基类</el-button>
      </div>

      <el-table v-loading="loading" :data="items" empty-text="暂无基类配置" max-height="420">
        <el-table-column prop="alias" label="别名" min-width="130" />
        <el-table-column
          prop="qualifiedName"
          label="完整类名"
          min-width="270"
          show-overflow-tooltip
        />
        <el-table-column label="字段数" width="88" align="center">
          <template #default="scope">{{ scope.row.fields.length }}</template>
        </el-table-column>
        <el-table-column label="操作" width="150" align="right">
          <template #default="scope">
            <el-button link type="primary" @click="startEdit(scope.row)">编辑</el-button>
            <el-button
              link
              type="danger"
              :loading="deletingId === scope.row.id"
              @click="removeItem(scope.row)"
            >
              删除
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <el-form v-else label-position="top" class="manager-form" @submit.prevent>
      <div class="form-heading">
        <el-button link type="primary" @click="mode = 'list'">返回列表</el-button>
        <span>{{ editingId === null ? "新增基类" : "编辑基类" }}</span>
      </div>
      <el-form-item label="别名" required>
        <el-input v-model="draft.alias" maxlength="50" placeholder="例如：审计基类" />
      </el-form-item>
      <el-form-item label="完整类名" required>
        <el-input v-model="draft.qualifiedName" placeholder="例如：com.example.common.BaseEntity" />
      </el-form-item>
      <el-form-item label="包含字段">
        <el-input
          v-model="draft.fieldsText"
          type="textarea"
          :rows="9"
          resize="vertical"
          placeholder="id&#10;createdAt&#10;updatedAt"
        />
        <div class="field-hint">支持逗号或换行分隔，请填写生成后的 Java 属性名。</div>
      </el-form-item>
    </el-form>

    <template #footer>
      <template v-if="mode === 'list'">
        <el-button @click="visible = false">关闭</el-button>
      </template>
      <template v-else>
        <el-button @click="mode = 'list'">取消</el-button>
        <el-button type="primary" :loading="saving" @click="save">保存</el-button>
      </template>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { reactive, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  SqlEntityBaseClass,
  SqlEntityBaseClassDraft,
  SqlEntityBaseClassListResponse,
} from "../types/sql-entity";
import { parseBaseClassFields, validateJavaQualifiedName } from "../utils/sqlEntityBaseClass";

const emit = defineEmits<{
  changed: [items: SqlEntityBaseClass[]];
}>();

const visible = ref(false);
const mode = ref<"list" | "edit">("list");
const items = ref<SqlEntityBaseClass[]>([]);
const loading = ref(false);
const saving = ref(false);
const deletingId = ref<number | null>(null);
const editingId = ref<number | null>(null);
const draft = reactive<SqlEntityBaseClassDraft>({
  alias: "",
  qualifiedName: "",
  fieldsText: "",
});

function resetDraft() {
  editingId.value = null;
  draft.alias = "";
  draft.qualifiedName = "";
  draft.fieldsText = "";
}

function resetView() {
  mode.value = "list";
  resetDraft();
}

async function loadItems() {
  loading.value = true;
  try {
    const result = (await invokeToolByChannel(
      "tool:sql-entity:base-class-list",
      {},
    )) as SqlEntityBaseClassListResponse;
    items.value = result.items;
  } finally {
    loading.value = false;
  }
}

function startCreate() {
  resetDraft();
  mode.value = "edit";
}

function startEdit(item: SqlEntityBaseClass) {
  editingId.value = item.id;
  draft.alias = item.alias;
  draft.qualifiedName = item.qualifiedName;
  draft.fieldsText = item.fields.join("\n");
  mode.value = "edit";
}

async function save() {
  if (!draft.alias.trim()) {
    ElMessage.warning("请输入别名");
    return;
  }
  const qualifiedNameError = validateJavaQualifiedName(draft.qualifiedName);
  if (qualifiedNameError) {
    ElMessage.warning(qualifiedNameError);
    return;
  }

  let fields: string[];
  try {
    fields = parseBaseClassFields(draft.fieldsText);
  } catch (error) {
    ElMessage.warning((error as Error).message);
    return;
  }

  saving.value = true;
  try {
    const channel =
      editingId.value === null
        ? "tool:sql-entity:base-class-create"
        : "tool:sql-entity:base-class-update";
    await invokeToolByChannel(channel, {
      ...(editingId.value === null ? {} : { id: editingId.value }),
      alias: draft.alias.trim(),
      qualifiedName: draft.qualifiedName.trim(),
      fields,
    });
    await loadItems();
    mode.value = "list";
    resetDraft();
    emit("changed", items.value);
    ElMessage.success("基类已保存");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    saving.value = false;
  }
}

async function removeItem(item: SqlEntityBaseClass) {
  try {
    await ElMessageBox.confirm(
      `确定删除基类“${item.alias}”吗？当前转换选择会同步移除该配置。`,
      "删除基类",
      { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" },
    );
  } catch {
    return;
  }

  deletingId.value = item.id;
  try {
    await invokeToolByChannel("tool:sql-entity:base-class-delete", { id: item.id });
    await loadItems();
    emit("changed", items.value);
    ElMessage.success("基类已删除");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    deletingId.value = null;
  }
}

async function open() {
  visible.value = true;
  mode.value = "list";
  try {
    await loadItems();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

defineExpose({ open });
</script>

<style scoped>
.manager-list,
.manager-form {
  min-height: 280px;
}

.manager-toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 16px;
}

.manager-title {
  color: var(--el-text-color-primary);
  font-size: 15px;
  font-weight: 600;
}

.manager-hint,
.field-hint {
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.6;
}

.manager-hint {
  margin-top: 3px;
}

.form-heading {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
  color: var(--el-text-color-primary);
  font-size: 15px;
  font-weight: 600;
}

.field-hint {
  margin-top: 6px;
}
</style>
