<template>
  <el-dialog
    :model-value="visible"
    :title="isEdit ? '项目设置' : '新建项目'"
    width="560px"
    :close-on-click-modal="false"
    :before-close="handleBeforeClose"
  >
    <el-form :model="form" label-width="96px" label-position="right">
      <el-form-item label="名称" required>
        <el-input v-model="form.name" placeholder="例如：管理后台 Mock" maxlength="60" />
      </el-form-item>
      <el-form-item label="监听地址">
        <el-select v-model="form.host">
          <el-option label="127.0.0.1" value="127.0.0.1" />
          <el-option label="0.0.0.0" value="0.0.0.0" />
        </el-select>
        <div v-if="form.host === '0.0.0.0'" class="field-hint">监听 0.0.0.0 后局域网内其他设备可访问该服务</div>
      </el-form-item>
      <el-form-item label="端口">
        <el-input-number v-model="form.port" :min="1" :max="65535" controls-position="right" />
        <div v-if="portConflict" class="field-hint warn">
          端口与项目「{{ portConflict.name }}」相同，二者不能同时运行
        </div>
      </el-form-item>
      <el-form-item label="描述">
        <el-input v-model="form.description" type="textarea" :rows="3" />
      </el-form-item>
      <el-form-item label="CORS">
        <el-checkbox v-model="form.enabledCorsDefault">新路由默认启用 CORS</el-checkbox>
      </el-form-item>
    </el-form>
    <template #footer>
      <div class="dialog-footer">
        <el-button v-if="isEdit" :icon="Delete" type="danger" text @click="emit('delete')">删除项目</el-button>
        <div class="footer-actions">
          <el-button @click="requestClose">取消</el-button>
          <el-button type="primary" @click="handleSave">保存</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { Delete } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import type { ApiMockProjectFormModel, ApiMockProjectSummary } from "../../types/api-mock";
import { findMockPortConflict, serializeMockProjectForm } from "../../utils/apiMock";

const props = defineProps<{
  visible: boolean;
  /** 编辑目标；为空表示新建 */
  project: ApiMockProjectSummary | null;
  /** 全部项目，用于端口冲突提示 */
  projects: ApiMockProjectSummary[];
}>();

const emit = defineEmits<{
  (event: "update:visible", value: boolean): void;
  (event: "save", model: ApiMockProjectFormModel): void;
  (event: "delete"): void;
}>();

const form = reactive<ApiMockProjectFormModel>({
  id: null,
  name: "",
  description: "",
  host: "127.0.0.1",
  port: 18080,
  enabledCorsDefault: true,
});

let baseline = "";

const isEdit = computed(() => form.id !== null);

const portConflict = computed(() => findMockPortConflict(props.projects, form.id, form.port));

function snapshotText(): string {
  return serializeMockProjectForm({
    name: form.name,
    description: form.description,
    host: form.host,
    port: form.port,
    enabledCorsDefault: form.enabledCorsDefault,
  });
}

function assignForm(project: ApiMockProjectSummary | null) {
  form.id = project?.id ?? null;
  form.name = project?.name ?? "";
  form.description = project?.description ?? "";
  form.host = project?.host ?? "127.0.0.1";
  form.port = project?.port ?? 18080;
  form.enabledCorsDefault = project?.enabledCorsDefault ?? true;
  baseline = snapshotText();
}

watch(
  () => props.visible,
  (visible) => {
    if (visible) assignForm(props.project);
  },
);

async function requestClose() {
  if (snapshotText() !== baseline) {
    try {
      await ElMessageBox.confirm("关闭后将丢失未保存的修改。", "未保存的修改", {
        type: "warning",
        confirmButtonText: "放弃修改",
        cancelButtonText: "继续编辑",
      });
    } catch {
      return;
    }
  }
  emit("update:visible", false);
}

function handleBeforeClose(_done: () => void) {
  // 关闭统一走 requestClose，由父组件更新 visible 驱动收起。
  void requestClose();
}

function handleSave() {
  if (!form.name.trim()) {
    ElMessage.error("项目名称不能为空");
    return;
  }
  emit("save", { ...form, name: form.name.trim() });
}
</script>

<style scoped>
.field-hint {
  width: 100%;
  font-size: 12px;
  line-height: 1.6;
  color: #64748b;
}

.field-hint.warn {
  color: #b45309;
}

.dialog-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.footer-actions {
  margin-left: auto;
  display: flex;
  gap: 8px;
}
</style>
