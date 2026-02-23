<template>
  <el-dialog
    v-model="visible"
    :title="isEdit ? '编辑凭据' : '新建凭据'"
    width="420px"
    class="vault-entry-dialog"
    :before-close="onBeforeClose"
    destroy-on-close
    @closed="onClosed"
  >
    <el-form label-position="top" class="vault-entry-form">
      <el-form-item label="类型">
        <el-radio-group v-model="form.category" :disabled="isEdit">
          <el-radio-button value="app">应用系统</el-radio-button>
          <el-radio-button value="server">服务器</el-radio-button>
          <el-radio-button value="database">数据库</el-radio-button>
        </el-radio-group>
      </el-form-item>

      <el-form-item label="标题">
        <el-input v-model="form.title" placeholder="如：公司邮箱、测试服务器" />
      </el-form-item>

      <el-form-item label="环境标签">
        <el-select v-model="form.environment" placeholder="选择环境（可选）" clearable style="width: 100%">
          <el-option value="生产" />
          <el-option value="测试" />
          <el-option value="本地" />
        </el-select>
      </el-form-item>

      <!-- App fields -->
      <template v-if="form.category === 'app'">
        <el-form-item label="网址/应用">
          <el-input v-model="form.url" placeholder="https://... 或应用名称" />
        </el-form-item>
        <el-form-item label="账号">
          <el-input v-model="form.account" placeholder="用户名或邮箱" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input v-model="form.password" type="password" show-password placeholder="密码" />
          <PasswordStrengthIndicator :password="form.password" :immediate="isEdit" />
        </el-form-item>
      </template>

      <!-- Server fields -->
      <template v-if="form.category === 'server'">
        <el-form-item label="地址">
          <el-input v-model="form.address" placeholder="IP 或域名" />
        </el-form-item>
        <el-form-item label="服务器类型">
          <el-select v-model="form.serverType" style="width: 100%">
            <el-option value="Linux" />
            <el-option value="Windows" />
            <el-option value="macOS" />
          </el-select>
        </el-form-item>
        <el-form-item label="账号">
          <el-input v-model="form.account" placeholder="用户名" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input v-model="form.password" type="password" show-password placeholder="密码" />
          <PasswordStrengthIndicator :password="form.password" :immediate="isEdit" />
        </el-form-item>
      </template>

      <!-- Database fields -->
      <template v-if="form.category === 'database'">
        <el-form-item label="数据库类型">
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
        <div class="vault-entry-form__row">
          <el-form-item label="地址" class="vault-entry-form__flex">
            <el-input v-model="form.address" placeholder="IP 或域名" />
          </el-form-item>
          <el-form-item label="端口" class="vault-entry-form__port">
            <el-input-number v-model="form.port" :min="1" :max="65535" controls-position="right" />
          </el-form-item>
        </div>
        <el-form-item label="账号">
          <el-input v-model="form.account" placeholder="用户名" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input v-model="form.password" type="password" show-password placeholder="密码" />
          <PasswordStrengthIndicator :password="form.password" :immediate="isEdit" />
        </el-form-item>
        <el-form-item label="数据库名">
          <el-input v-model="form.dbName" placeholder="数据库名称" />
        </el-form-item>
        <el-form-item label="Schema">
          <el-input v-model="form.schema" placeholder="Schema（可选）" />
        </el-form-item>
      </template>

      <el-form-item label="备注">
        <el-input v-model="form.notes" type="textarea" :rows="3" placeholder="备注信息（可选）" />
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="onBeforeClose(() => { visible = false })">取消</el-button>
      <el-button type="primary" :loading="saving" @click="onSave">
        {{ isEdit ? "保存" : "创建" }}
      </el-button>
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

// 切换数据库类型时自动更新为对应默认端口
watch(() => form.dbType, (newType) => {
  if (form.category === "database" && newType in DB_DEFAULT_PORT) {
    form.port = DB_DEFAULT_PORT[newType];
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
.vault-entry-form .el-form-item {
  margin-bottom: 14px;
}

.vault-entry-form__row {
  display: flex;
  gap: 12px;
}

.vault-entry-form__flex {
  flex: 1;
}

.vault-entry-form__port {
  width: 130px;
}
</style>

<style>
/* Dialog enter animation (unscoped for el-dialog) */
.vault-entry-dialog .el-dialog {
  animation: dialog-enter 0.25s var(--lc-ease-out);
}

@keyframes dialog-enter {
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
