<template>
  <el-dialog
    v-model="visible"
    :title="isEdit ? '编辑凭据' : '新建凭据'"
    width="480px"
    class="vault-entry-dialog"
    :before-close="onBeforeClose"
    destroy-on-close
    @closed="onClosed"
  >
    <el-form label-position="top" class="vault-entry-form">
      <!-- Type Selector -->
      <div class="vault-type-selector">
        <div
          v-for="cat in CAT_OPTIONS"
          :key="cat.value"
          class="vault-type-option"
          :class="{ 'is-active': form.category === cat.value }"
          @click="form.category = cat.value"
        >
          <div class="vault-type-icon">
            <svg v-if="cat.value === 'app'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="2" y="3" width="20" height="14" rx="2" />
              <path d="M8 21h8" />
              <path d="M12 17v4" />
            </svg>
            <svg v-else-if="cat.value === 'server'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="2" y="2" width="20" height="8" rx="2" />
              <rect x="2" y="14" width="20" height="8" rx="2" />
              <circle cx="6" cy="6" r="1" fill="currentColor" />
              <circle cx="6" cy="18" r="1" fill="currentColor" />
            </svg>
            <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <ellipse cx="12" cy="5" rx="9" ry="3" />
              <path d="M3 5v14a9 3 0 0 0 18 0V5" />
            </svg>
          </div>
          <span class="vault-type-name">{{ cat.label }}</span>
        </div>
      </div>

      <div class="vault-form-section">
        <div class="vault-section-title">基础信息</div>
        <div class="vault-form-row">
          <el-form-item label="标题" class="vault-form-item-title">
            <el-input v-model="form.title" placeholder="如：公司邮箱、测试服务器" />
          </el-form-item>
          <el-form-item label="环境" class="vault-form-item-env">
            <el-select v-model="form.environment" placeholder="选择" clearable style="width: 100%">
              <el-option value="生产" />
              <el-option value="测试" />
              <el-option value="本地" />
            </el-select>
          </el-form-item>
        </div>
      </div>

      <div class="vault-form-section">
        <div class="vault-section-title">凭据详情</div>

        <!-- App fields -->
        <template v-if="form.category === 'app'">
          <el-form-item label="网址 / 应用">
            <el-input v-model="form.url" placeholder="https://... 或应用名称">
              <template #prefix>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                  <circle cx="12" cy="12" r="10" />
                  <path d="M2 12h20" />
                  <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
                </svg>
              </template>
            </el-input>
          </el-form-item>
          <div class="vault-form-row">
            <el-form-item label="账号" class="vault-form-item-flex">
              <el-input v-model="form.account" placeholder="用户名或邮箱">
                <template #prefix>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>
                </template>
              </el-input>
            </el-form-item>
            <el-form-item label="密码" class="vault-form-item-flex">
              <el-input v-model="form.password" type="password" show-password placeholder="密码">
                <template #prefix>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                    <rect x="3" y="11" width="18" height="11" rx="2" />
                    <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                  </svg>
                </template>
              </el-input>
            </el-form-item>
          </div>
          <PasswordStrengthIndicator :password="form.password" :immediate="isEdit" />
        </template>

        <!-- Server fields -->
        <template v-if="form.category === 'server'">
          <div class="vault-form-row">
            <el-form-item label="地址" class="vault-form-item-flex">
              <el-input v-model="form.address" placeholder="IP 或域名">
                <template #prefix>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                    <rect x="2" y="2" width="20" height="8" rx="2" />
                    <rect x="2" y="14" width="20" height="8" rx="2" />
                    <circle cx="6" cy="6" r="1" fill="currentColor" />
                    <circle cx="6" cy="18" r="1" fill="currentColor" />
                  </svg>
                </template>
              </el-input>
            </el-form-item>
            <el-form-item label="服务器类型" class="vault-form-item-select">
              <el-select v-model="form.serverType" style="width: 100%">
                <el-option value="Linux" />
                <el-option value="Windows" />
                <el-option value="macOS" />
              </el-select>
            </el-form-item>
          </div>
          <div class="vault-form-row">
            <el-form-item label="账号" class="vault-form-item-flex">
              <el-input v-model="form.account" placeholder="用户名">
                <template #prefix>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>
                </template>
              </el-input>
            </el-form-item>
            <el-form-item label="密码" class="vault-form-item-flex">
              <el-input v-model="form.password" type="password" show-password placeholder="密码">
                <template #prefix>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                    <rect x="3" y="11" width="18" height="11" rx="2" />
                    <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                  </svg>
                </template>
              </el-input>
            </el-form-item>
          </div>
          <PasswordStrengthIndicator :password="form.password" :immediate="isEdit" />
        </template>

        <!-- Database fields -->
        <template v-if="form.category === 'database'">
          <div class="vault-form-row">
            <el-form-item label="数据库类型" class="vault-form-item-select">
              <el-select v-model="form.dbType" style="width: 100%">
                <el-option value="Kingbase" />
                <el-option value="MySQL" />
                <el-option value="Redis" />
                <el-option value="DaMeng" />
                <el-option value="TiDB" />
                <el-option value="PostgreSQL" />
                <el-option value="SQL Server" />
                <el-option value="Oracle" />
                <el-option value="SQLite" />
                <el-option value="MongoDB" />
              </el-select>
            </el-form-item>
            <el-form-item label="端口" class="vault-form-item-port">
              <el-input-number v-model="form.port" :min="1" :max="65535" controls-position="right" style="width: 100%" />
            </el-form-item>
          </div>
          <el-form-item label="地址">
            <el-input v-model="form.address" placeholder="IP 或域名">
              <template #prefix>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                  <ellipse cx="12" cy="5" rx="9" ry="3" />
                  <path d="M3 5v14a9 3 0 0 0 18 0V5" />
                </svg>
              </template>
            </el-input>
          </el-form-item>
          <div class="vault-form-row">
            <el-form-item label="账号" class="vault-form-item-flex">
              <el-input v-model="form.account" placeholder="用户名">
                <template #prefix>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>
                </template>
              </el-input>
            </el-form-item>
            <el-form-item label="密码" class="vault-form-item-flex">
              <el-input v-model="form.password" type="password" show-password placeholder="密码">
                <template #prefix>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
                    <rect x="3" y="11" width="18" height="11" rx="2" />
                    <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                  </svg>
                </template>
              </el-input>
            </el-form-item>
          </div>
          <PasswordStrengthIndicator :password="form.password" :immediate="isEdit" />
          <div class="vault-form-row">
            <el-form-item label="数据库名" class="vault-form-item-flex">
              <el-input v-model="form.dbName" placeholder="数据库名称" />
            </el-form-item>
            <el-form-item label="Schema" class="vault-form-item-flex">
              <el-input v-model="form.schema" placeholder="可选" />
            </el-form-item>
          </div>
        </template>
      </div>

      <div class="vault-form-section">
        <div class="vault-section-title">备注</div>
        <el-form-item>
          <el-input v-model="form.notes" type="textarea" :rows="2" placeholder="添加备注信息（可选）" />
        </el-form-item>
      </div>
    </el-form>

    <template #footer>
      <div class="vault-dialog-footer">
        <el-button @click="onBeforeClose(() => { visible = false })">取消</el-button>
        <el-button type="primary" :loading="saving" @click="onSave">
          {{ isEdit ? "保存" : "创建" }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, reactive, watch } from "vue";
import { ElMessageBox, ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import PasswordStrengthIndicator from "./PasswordStrengthIndicator.vue";

const DB_DEFAULT_PORT: Record<string, number> = {
  Kingbase: 54321,
  DaMeng: 5236,
  TiDB: 4000,
  MySQL: 3306,
  PostgreSQL: 5432,
  "SQL Server": 1433,
  Oracle: 1521,
  SQLite: 0,
  MongoDB: 27017,
  Redis: 6379,
};

const CAT_OPTIONS = [
  { value: "app", label: "应用系统" },
  { value: "server", label: "服务器" },
  { value: "database", label: "数据库" },
] as const;

interface FormState {
  id?: number;
  category: "app" | "server" | "database";
  title: string;
  environment: string;
  url: string;
  account: string;
  password: string;
  notes: string;
  address: string;
  serverType: string;
  dbType: string;
  port: number;
  dbName: string;
  schema: string;
}

const visible = ref(false);
const isEdit = ref(false);
const saving = ref(false);
let snapshot = "";

const defaultForm = (): FormState => ({
  category: "app",
  title: "",
  environment: "",
  url: "",
  account: "",
  password: "",
  notes: "",
  address: "",
  serverType: "Linux",
  dbType: "Kingbase",
  port: DB_DEFAULT_PORT["Kingbase"],
  dbName: "",
  schema: "",
});

const form = reactive<FormState>(defaultForm());

function formSnapshot(): string {
  const { id: _, ...rest } = form;
  return JSON.stringify(rest);
}

function isDirty(): boolean {
  return formSnapshot() !== snapshot;
}

watch(() => form.dbType, (newType) => {
  if (form.category === "database" && newType in DB_DEFAULT_PORT) {
    form.port = DB_DEFAULT_PORT[newType];
  }
});

watch(() => form.category, (newCat, oldCat) => {
  if (!isEdit.value) return;
  // app <-> server/database: 互迁 url <-> address
  if (oldCat === "app" && (newCat === "server" || newCat === "database")) {
    if (!form.address && form.url) form.address = form.url;
  } else if ((oldCat === "server" || oldCat === "database") && newCat === "app") {
    if (!form.url && form.address) form.url = form.address;
  }
  // 切换到 database 时填充默认端口
  if (newCat === "database" && form.dbType in DB_DEFAULT_PORT) {
    form.port = DB_DEFAULT_PORT[form.dbType];
  }
});

const emit = defineEmits<{
  (e: "saved"): void;
}>();

function show(entry?: {
  id: number;
  category: string;
  title: string;
  environment: string;
  fields: Record<string, unknown>;
}) {
  Object.assign(form, defaultForm());
  if (entry) {
    isEdit.value = true;
    form.id = entry.id;
    form.category = entry.category as FormState["category"];
    form.title = entry.title;
    form.environment = entry.environment;
    const f = entry.fields;
    form.url = (f.url as string) || "";
    form.account = (f.account as string) || "";
    form.password = (f.password as string) || "";
    form.notes = (f.notes as string) || "";
    form.address = (f.address as string) || "";
    form.serverType = (f.serverType as string) || "Linux";
    form.dbType = (f.dbType as string) || "MySQL";
    form.port = (f.port as number) || 3306;
    form.dbName = (f.dbName as string) || "";
    form.schema = (f.schema as string) || "";
  } else {
    isEdit.value = false;
  }
  snapshot = formSnapshot();
  visible.value = true;
}

async function onBeforeClose(done: () => void) {
  if (!isDirty()) {
    done();
    return;
  }
  try {
    await ElMessageBox.confirm("当前内容已修改，确定要关闭吗？", "关闭确认", {
      confirmButtonText: "关闭",
      cancelButtonText: "继续编辑",
      type: "warning",
    });
    done();
  } catch {
    // cancelled
  }
}

async function onSave() {
  if (!form.title.trim()) {
    return;
  }
  saving.value = true;
  try {
    const payload: Record<string, unknown> = {
      category: form.category,
      title: form.title.trim(),
      environment: form.environment.trim(),
      account: form.account,
      password: form.password,
      notes: form.notes,
    };
    if (form.category === "app") {
      payload.url = form.url;
    } else if (form.category === "server") {
      payload.address = form.address;
      payload.serverType = form.serverType;
    } else if (form.category === "database") {
      payload.dbType = form.dbType;
      payload.address = form.address;
      payload.port = form.port;
      payload.dbName = form.dbName;
      payload.schema = form.schema;
    }

    if (isEdit.value && form.id) {
      payload.id = form.id;
      await invokeToolByChannel("tool:vault:update", payload);
    } else {
      await invokeToolByChannel("tool:vault:create", payload);
    }
    snapshot = formSnapshot();
    visible.value = false;
    emit("saved");
  } catch (err) {
    const msg = (err as Error).message || "未知错误";
    ElMessage.error(`保存失败: ${msg}`);
  } finally {
    saving.value = false;
  }
}

function onClosed() {
  Object.assign(form, defaultForm());
  isEdit.value = false;
  snapshot = "";
}

defineExpose({ show });
</script>

<style scoped>
.vault-entry-form {
  padding: 0 4px;
}

/* --- Type Selector --- */
.vault-type-selector {
  display: flex;
  gap: 12px;
  margin-bottom: 24px;
}

.vault-type-option {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 16px 12px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-0);
  cursor: pointer;
  transition: all 150ms var(--lc-ease);
}

.vault-type-option:hover:not(.is-disabled) {
  border-color: var(--lc-border-hover);
  background: var(--lc-surface-1);
}

.vault-type-option.is-active {
  border-color: var(--lc-accent);
  background: var(--lc-accent-dim);
}

.vault-type-icon {
  width: 28px;
  height: 28px;
  color: var(--lc-text-secondary);
  transition: color 150ms var(--lc-ease);
}

.vault-type-icon svg {
  width: 100%;
  height: 100%;
}

.vault-type-option.is-active .vault-type-icon {
  color: var(--lc-accent);
}

.vault-type-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--lc-text-secondary);
  transition: color 150ms var(--lc-ease);
}

.vault-type-option.is-active .vault-type-name {
  color: var(--lc-text);
}

/* --- Form Section --- */
.vault-form-section {
  margin-bottom: 20px;
}

.vault-section-title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--lc-text-muted);
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--lc-border-subtle);
}

/* --- Form Rows --- */
.vault-form-row {
  display: flex;
  gap: 16px;
}

.vault-form-row .el-form-item {
  margin-bottom: 16px;
}

.vault-form-item-flex {
  flex: 1;
}

.vault-form-item-title {
  flex: 1;
}

.vault-form-item-env {
  width: 120px;
  flex-shrink: 0;
}

.vault-form-item-select {
  width: 160px;
  flex-shrink: 0;
}

.vault-form-item-port {
  width: 120px;
  flex-shrink: 0;
}

/* --- Input Icons --- */
.vault-input-icon {
  width: 16px;
  height: 16px;
  color: var(--lc-text-muted);
}

/* --- Form Items --- */
.vault-entry-form :deep(.el-form-item) {
  margin-bottom: 16px;
}

.vault-entry-form :deep(.el-form-item__label) {
  font-size: 13px;
  font-weight: 500;
  color: var(--lc-text);
  padding-bottom: 6px;
}

.vault-entry-form :deep(.el-input__inner),
.vault-entry-form :deep(.el-textarea__inner) {
  font-size: 14px;
}

/* --- Dialog Footer --- */
.vault-dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}
</style>

<style>
/* Dialog animations */
.vault-entry-dialog .el-dialog {
  border-radius: var(--lc-radius-lg);
  background: var(--lc-surface-0);
  animation: vault-dialog-enter 0.25s var(--lc-ease-out);
}

.vault-entry-dialog .el-dialog__header {
  margin-right: 0;
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--lc-border);
}

.vault-entry-dialog .el-dialog__title {
  font-family: var(--lc-font-display);
  font-size: 18px;
  font-weight: 600;
  color: var(--lc-text);
}

.vault-entry-dialog .el-dialog__body {
  padding: 20px 24px;
}

.vault-entry-dialog .el-dialog__footer {
  padding: 16px 24px 20px;
  border-top: 1px solid var(--lc-border);
}

@keyframes vault-dialog-enter {
  from {
    opacity: 0;
    transform: scale(0.95) translateY(-10px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}
</style>
