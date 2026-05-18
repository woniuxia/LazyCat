<template>
  <el-dialog
    v-model="dialogVisible"
    :title="editing ? '编辑项目' : '新建项目'"
    width="680px"
    :close-on-click-modal="false"
    :close-on-press-escape="false"
    :before-close="handleBeforeClose"
    @close="onDialogClose"
  >
    <el-form :model="form" label-width="60px" size="default" @submit.prevent="submit">
      <el-form-item label="名称">
        <el-input v-model="form.name" placeholder="项目名称" @keyup.enter="submit" />
      </el-form-item>
      <el-form-item label="描述" class="pm-form-item-top">
        <RichDescriptionEditor
          ref="editorRef"
          v-model="form.description"
          owner-type="pm_project"
          :owner-id="editing?.id ?? null"
          placeholder="项目描述（支持粘贴图片）"
        />
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
      <el-button :disabled="submitting" @click="handleCancel">取消</el-button>
      <el-button
        type="primary"
        :loading="submitting"
        :disabled="submitting"
        @click="submit"
      >确定</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, inject, watch, nextTick } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useToolInvoke } from "../composables/useToolInvoke";
import type { PmProject, PmSiyuanLocation } from "../types/pm";
import { formatPmSiyuanLocationLabel } from "../utils/pmSiyuan";
import { PM_SIYUAN_KEY } from "../composables/pmSiyuanKey";
import RichDescriptionEditor from "./RichDescriptionEditor.vue";
import {
  useRichDescriptionLifecycle,
  type RichEditorExposed,
} from "../composables/useRichDescriptionLifecycle";

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
const editorRef = ref<RichEditorExposed | null>(null);
const submittedThisRound = ref(false);
const submitting = ref(false);

const lifecycle = useRichDescriptionLifecycle({
  ownerType: "pm_project",
  editorRef,
  getRealId: () => editing.value?.id ?? null,
});

const presetColors = [
  "#0ea5e9", "#67c23a", "#e6a23c", "#f56c6c",
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
  submittedThisRound.value = false;
  dialogVisible.value = true;
  // 等 Dialog 挂载/Editor 拿到新值后再 reset，保证 tempId 每次新建都是新的
  queueMicrotask(() => editorRef.value && (editorRef.value as any).reset?.(""));
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
  submittedThisRound.value = false;
  dialogVisible.value = true;
  queueMicrotask(() =>
    editorRef.value && (editorRef.value as any).reset?.(p.description ?? "")
  );
}

function resetForm() {
  editing.value = null;
}

// ── Dirty 检测 ───────────────────────────────────────────
const initialSnapshot = ref<string>("");

function currentSnapshot(): string {
  return JSON.stringify({
    name: form.value.name,
    description: form.value.description,
    color: form.value.color,
    useSiyuanOverride: form.value.useSiyuanOverride,
    siyuanLocationOverride: form.value.siyuanLocationOverride,
  });
}

function isDirty(): boolean {
  return initialSnapshot.value !== "" && currentSnapshot() !== initialSnapshot.value;
}

async function confirmDiscardIfDirty(): Promise<boolean> {
  if (!isDirty()) return true;
  try {
    await ElMessageBox.confirm(
      "有未保存的修改，确定关闭？已编辑的内容将丢失。",
      "未保存的修改",
      {
        confirmButtonText: "放弃修改",
        cancelButtonText: "继续编辑",
        type: "warning",
      },
    );
    return true;
  } catch {
    return false;
  }
}

async function handleBeforeClose(done: () => void) {
  if (submitting.value) return;
  const ok = await confirmDiscardIfDirty();
  if (ok) done();
}

async function handleCancel() {
  if (submitting.value) return;
  const ok = await confirmDiscardIfDirty();
  if (ok) dialogVisible.value = false;
}

watch(dialogVisible, (v) => {
  if (v) {
    void nextTick(() => {
      initialSnapshot.value = currentSnapshot();
    });
  } else {
    initialSnapshot.value = "";
  }
});

async function onDialogClose() {
  // 未提交关闭：清理新建场景残留的 tmp 附件
  if (!submittedThisRound.value) {
    try {
      await lifecycle.onCancel();
    } catch (e) {
      console.warn("cleanup tmp attachments failed:", e);
    }
  }
  submittedThisRound.value = false;
  resetForm();
}

async function submit() {
  if (submitting.value) return;
  if (!form.value.name.trim()) {
    ElMessage.warning("请输入项目名称");
    return;
  }
  submitting.value = true;
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
      // 编辑：保存前按当前 doc 清理已删图的残留附件
      await lifecycle.beforeCloseEdit();
      await invoke("tool:pm:project-update", {
        id: editing.value.id,
        ...payload,
        sortOrder: editing.value.sortOrder,
      });
      submittedThisRound.value = true;
      dialogVisible.value = false;
      emit("projects-changed", {});
    } else {
      const res = (await invoke("tool:pm:project-create", payload)) as { id: number };
      // 新建：将 tmp-<uuid> 下的附件 rebind 到 realId
      await lifecycle.afterSubmit(res.id);
      submittedThisRound.value = true;
      dialogVisible.value = false;
      emit("projects-changed", { newProjectId: res.id });
    }
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    submitting.value = false;
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
    emit("projects-changed", { deletedProjectId: p.id } as any);
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
