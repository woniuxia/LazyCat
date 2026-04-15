<template>
  <el-dialog v-model="dialogVisible" :title="editing ? '编辑项目' : '新建项目'" width="520px" @close="resetForm">
    <el-form :model="form" label-width="60px" size="default" @submit.prevent="submit">
      <el-form-item label="名称">
        <el-input v-model="form.name" placeholder="项目名称" @keyup.enter="submit" />
      </el-form-item>
      <el-form-item label="描述">
        <el-input v-model="form.description" type="textarea" :rows="2" placeholder="项目描述（可选）" />
      </el-form-item>
      <el-form-item label="颜色">
        <el-color-picker v-model="form.color" :predefine="presetColors" />
      </el-form-item>
      <el-form-item label="思源位置" class="pm-form-item-top">
        <div class="pm-siyuan-config-card">
          <el-radio-group v-model="form.useSiyuanOverride">
            <el-radio :value="false">继承全局默认</el-radio>
            <el-radio :value="true">使用项目专属位置</el-radio>
          </el-radio-group>
          <div class="pm-siyuan-config-summary">
            当前：{{
              form.useSiyuanOverride
                ? formatPmSiyuanLocationLabel(form.siyuanLocationOverride)
                : formatPmSiyuanLocationLabel(siyuan.globalSiyuanLocation.value)
            }}
          </div>
          <div v-if="form.useSiyuanOverride" class="pm-siyuan-inline-actions">
            <el-button size="small" @click="siyuan.openLocationPicker('project')">选择位置</el-button>
            <el-button size="small" @click="siyuan.clearProjectSiyuanOverride()">清空</el-button>
          </div>
        </div>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="dialogVisible = false">取消</el-button>
      <el-button type="primary" @click="submit">确定</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, inject } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useToolInvoke } from "../composables/useToolInvoke";
import type { PmProject, PmSiyuanLocation } from "../types/pm";
import { formatPmSiyuanLocationLabel } from "../utils/pmSiyuan";
import { PM_SIYUAN_KEY } from "../composables/pmSiyuanKey";

interface ProjectForm {
  name: string;
  description: string;
  color: string;
  useSiyuanOverride: boolean;
  siyuanLocationOverride: PmSiyuanLocation | null;
}

const emit = defineEmits<{
  "projects-changed": [{ newProjectId?: number }];
}>();

const { invoke } = useToolInvoke();
const siyuan = inject(PM_SIYUAN_KEY)!;

const dialogVisible = ref(false);
const editing = ref<PmProject | null>(null);

const presetColors = [
  "#4d7df2", "#67c23a", "#e6a23c", "#f56c6c",
  "#909399", "#409eff", "#19be6b", "#ff9900",
  "#ed4014", "#2b85e4", "#5cad2f", "#ff6900",
];

const form = ref<ProjectForm>(createEmptyForm());

function createEmptyForm(): ProjectForm {
  const randomColor = presetColors[Math.floor(Math.random() * presetColors.length)];
  return {
    name: "",
    description: "",
    color: randomColor,
    useSiyuanOverride: false,
    siyuanLocationOverride: null,
  };
}

function showCreate() {
  editing.value = null;
  form.value = createEmptyForm();
  dialogVisible.value = true;
}

function showEdit(p: PmProject) {
  editing.value = p;
  form.value = {
    name: p.name,
    description: p.description,
    color: p.color,
    useSiyuanOverride: Boolean(p.siyuanLocationOverride),
    siyuanLocationOverride: siyuan.cloneLocation(p.siyuanLocationOverride),
  };
  dialogVisible.value = true;
}

function resetForm() {
  editing.value = null;
}

async function submit() {
  if (!form.value.name.trim()) {
    ElMessage.warning("请输入项目名称");
    return;
  }
  try {
    const payload = {
      name: form.value.name,
      description: form.value.description,
      color: form.value.color,
      siyuanLocationOverride: form.value.useSiyuanOverride
        ? form.value.siyuanLocationOverride
        : null,
    };
    if (editing.value) {
      await invoke("tool:pm:project-update", {
        id: editing.value.id,
        ...payload,
        sortOrder: editing.value.sortOrder,
      });
      dialogVisible.value = false;
      emit("projects-changed", {});
    } else {
      await invoke("tool:pm:project-create", payload);
      dialogVisible.value = false;
      emit("projects-changed", {});
    }
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function archiveProject(p: PmProject) {
  try {
    await invoke("tool:pm:project-archive", { id: p.id });
    emit("projects-changed", {});
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function restoreProject(p: PmProject) {
  try {
    await invoke("tool:pm:project-restore", { id: p.id });
    emit("projects-changed", {});
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function deleteProject(p: PmProject) {
  try {
    await ElMessageBox.confirm(`确定删除项目「${p.name}」？此操作会同时删除所有工作项。`, "删除确认", {
      type: "warning",
    });
    await invoke("tool:pm:project-delete", { id: p.id });
    emit("projects-changed", { deletedProjectId: p.id });
  } catch (e) {
    if ((e as string) !== "cancel") {
      ElMessage.error((e as Error).message);
    }
  }
}

defineExpose({
  showCreate,
  showEdit,
  handleContext(event: MouseEvent, p: PmProject, openCtxMenu: (event: MouseEvent, actions: any[]) => void) {
    const actions = p.status === "active"
      ? [
          { label: "编辑", action: () => showEdit(p) },
          { label: "归档", action: () => archiveProject(p) },
          { divider: true, label: "", action: () => {} },
          { label: "删除", action: () => deleteProject(p), danger: true },
        ]
      : [
          { label: "编辑", action: () => showEdit(p) },
          { label: "恢复", action: () => restoreProject(p) },
          { divider: true, label: "", action: () => {} },
          { label: "删除", action: () => deleteProject(p), danger: true },
        ];
    openCtxMenu(event, actions);
  },
  getSiyuanOverride: () => form.value.siyuanLocationOverride,
  setSiyuanOverride: (useOverride: boolean, location: PmSiyuanLocation | null) => {
    form.value.useSiyuanOverride = useOverride;
    form.value.siyuanLocationOverride = location;
  },
});
</script>

<style>
.pm-form-item-top .el-form-item__content {
  align-items: flex-start;
}

.pm-form-item-top .el-form-item__label {
  margin-bottom: 8px;
}

.pm-siyuan-config-card {
  width: 100%;
}
</style>
