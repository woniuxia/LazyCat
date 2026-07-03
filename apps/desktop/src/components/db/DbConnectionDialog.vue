<template>
  <el-dialog
    :model-value="visible"
    :title="isEdit ? '编辑连接' : '新建连接'"
    width="560px"
    :close-on-click-modal="false"
    @update:model-value="emit('update:visible', $event)"
  >
    <el-form :model="form" label-width="96px" label-position="right">
      <el-form-item label="连接名称" required>
        <el-input v-model="form.name" placeholder="例如：测试库-订单中心" maxlength="60" />
      </el-form-item>
      <el-form-item label="数据库类型" required>
        <el-radio-group v-model="form.engine" :disabled="isEdit" @change="onEngineChange">
          <el-radio-button value="mysql">MySQL</el-radio-button>
          <el-radio-button value="kingbase">KingbaseES</el-radio-button>
        </el-radio-group>
      </el-form-item>
      <el-form-item label="主机" required>
        <div class="host-row">
          <el-input v-model="form.host" placeholder="IP 或主机名" />
          <el-input-number
            v-model="form.port"
            :min="1"
            :max="65535"
            :controls="false"
            class="port-input"
            @change="portTouched = true"
          />
        </div>
      </el-form-item>
      <el-form-item label="用户名">
        <el-input v-model="form.username" placeholder="数据库用户名" />
      </el-form-item>
      <el-form-item label="密码">
        <el-input
          v-model="passwordInput"
          type="password"
          show-password
          :placeholder="passwordPlaceholder"
          @input="passwordTouched = true"
        />
        <div v-if="isEdit && hasPassword && !passwordTouched" class="field-hint">
          已保存密码，留空保持不变；输入新值覆盖，清空后保存即删除密码
        </div>
      </el-form-item>
      <el-form-item :label="form.engine === 'kingbase' ? '默认数据库' : '默认库'" :required="form.engine === 'kingbase'">
        <el-input
          v-model="form.defaultDatabase"
          :placeholder="form.engine === 'kingbase' ? 'KingbaseES 必填，例如 test' : '可选'"
        />
      </el-form-item>
      <el-form-item label="环境标签">
        <el-radio-group v-model="form.envTag">
          <el-radio-button v-for="(label, tag) in DB_ENV_LABELS" :key="tag" :value="tag">
            {{ label }}
          </el-radio-button>
        </el-radio-group>
        <div v-if="form.envTag === 'prod'" class="field-hint warn">
          生产环境连接：执行任何写语句前会要求二次确认
        </div>
      </el-form-item>
      <el-form-item label="只读保护">
        <el-switch v-model="form.readOnly" />
        <span class="field-hint inline">开启后本连接拒绝一切写语句（后端强制）</span>
      </el-form-item>
      <el-form-item label="分组">
        <el-input v-model="form.groupName" placeholder="可选，例如：订单中心" maxlength="30" />
      </el-form-item>
      <el-collapse class="advanced">
        <el-collapse-item title="高级选项" name="adv">
          <el-form-item label="查询超时(秒)">
            <el-input-number v-model="form.timeoutSecs" :min="5" :max="600" />
          </el-form-item>
          <el-form-item label="行数上限">
            <el-input-number v-model="form.maxRows" :min="100" :max="100000" :step="100" />
          </el-form-item>
        </el-collapse-item>
      </el-collapse>
    </el-form>
    <template #footer>
      <div class="dialog-footer">
        <el-button :loading="testing" @click="testConnection">测试连接</el-button>
        <span v-if="testResult" :class="['test-result', testResult.ok ? 'ok' : 'fail']">
          {{ testResult.message }}
        </span>
        <div class="footer-actions">
          <el-button @click="emit('update:visible', false)">取消</el-button>
          <el-button type="primary" :loading="saving" @click="save">保存</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../../bridge/tauri";
import {
  DB_ENGINE_DEFAULT_PORTS,
  DB_ENV_LABELS,
  type DbConnection,
  type DbConnectionDraft,
  type DbEngine,
  type DbEnvTag,
} from "../../types/db";

const props = defineProps<{
  visible: boolean;
  /** 编辑目标；为空表示新建 */
  connection?: DbConnection | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", value: boolean): void;
  (e: "saved", draft: DbConnectionDraft): void;
}>();

const form = reactive({
  name: "",
  engine: "mysql" as DbEngine,
  host: "",
  port: DB_ENGINE_DEFAULT_PORTS.mysql,
  username: "",
  defaultDatabase: "",
  envTag: "dev" as DbEnvTag,
  readOnly: false,
  groupName: "",
  timeoutSecs: 30,
  maxRows: 1000,
});

const passwordInput = ref("");
const passwordTouched = ref(false);
const portTouched = ref(false);
const testing = ref(false);
const saving = ref(false);
const testResult = ref<{ ok: boolean; message: string } | null>(null);

const isEdit = computed(() => !!props.connection);
const hasPassword = computed(() => props.connection?.hasPassword ?? false);
const passwordPlaceholder = computed(() =>
  isEdit.value && hasPassword.value ? "已保存（留空保持不变）" : "可选"
);

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    testResult.value = null;
    passwordInput.value = "";
    passwordTouched.value = false;
    const c = props.connection;
    if (c) {
      form.name = c.name;
      form.engine = c.engine;
      form.host = c.host;
      form.port = c.port;
      form.username = c.username;
      form.defaultDatabase = c.defaultDatabase ?? "";
      form.envTag = c.envTag;
      form.readOnly = c.readOnly;
      form.groupName = c.groupName ?? "";
      form.timeoutSecs = c.options.timeoutSecs ?? 30;
      form.maxRows = c.options.maxRows ?? 1000;
      portTouched.value = true;
    } else {
      form.name = "";
      form.engine = "mysql";
      form.host = "";
      form.port = DB_ENGINE_DEFAULT_PORTS.mysql;
      form.username = "";
      form.defaultDatabase = "";
      form.envTag = "dev";
      form.readOnly = false;
      form.groupName = "";
      form.timeoutSecs = 30;
      form.maxRows = 1000;
      portTouched.value = false;
    }
  }
);

function onEngineChange(): void {
  if (!portTouched.value) {
    form.port = DB_ENGINE_DEFAULT_PORTS[form.engine];
  }
}

function validate(): string | null {
  if (!form.name.trim()) return "请填写连接名称";
  if (!form.host.trim()) return "请填写主机地址";
  if (form.engine === "kingbase" && !form.defaultDatabase.trim()) {
    return "KingbaseES 连接必须填写默认数据库";
  }
  return null;
}

async function testConnection(): Promise<void> {
  const invalid = validate();
  if (invalid) {
    ElMessage.warning(invalid);
    return;
  }
  testing.value = true;
  testResult.value = null;
  try {
    const payload: Record<string, unknown> = {
      engine: form.engine,
      host: form.host.trim(),
      port: form.port,
      username: form.username.trim(),
      defaultDatabase: form.defaultDatabase.trim() || undefined,
    };
    // 未改动密码时带 connectionId 让后端回退已存密文
    if (passwordTouched.value) {
      payload.password = passwordInput.value;
    } else if (props.connection) {
      payload.connectionId = props.connection.id;
    }
    const data = (await invokeToolByChannel("tool:db:connection-test", payload)) as {
      serverVersion: string;
    };
    testResult.value = { ok: true, message: `连接成功：${data.serverVersion}` };
  } catch (error) {
    testResult.value = { ok: false, message: (error as Error).message };
  } finally {
    testing.value = false;
  }
}

async function save(): Promise<void> {
  const invalid = validate();
  if (invalid) {
    ElMessage.warning(invalid);
    return;
  }
  saving.value = true;
  try {
    const draft: DbConnectionDraft = {
      id: props.connection?.id,
      name: form.name.trim(),
      engine: form.engine,
      host: form.host.trim(),
      port: form.port,
      username: form.username.trim(),
      defaultDatabase: form.defaultDatabase.trim() || undefined,
      envTag: form.envTag,
      readOnly: form.readOnly,
      groupName: form.groupName.trim() || undefined,
      options: { timeoutSecs: form.timeoutSecs, maxRows: form.maxRows },
    };
    if (passwordTouched.value || !isEdit.value) {
      draft.password = passwordInput.value;
    }
    emit("saved", draft);
  } finally {
    saving.value = false;
  }
}
</script>

<style scoped>
.host-row {
  display: flex;
  gap: 8px;
  width: 100%;
}
.host-row .el-input {
  flex: 1;
}
.port-input {
  width: 110px;
}
.field-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.6;
}
.field-hint.warn {
  color: var(--el-color-danger);
}
.field-hint.inline {
  margin-left: 10px;
}
.advanced {
  border: none;
  margin-left: 12px;
}
.dialog-footer {
  display: flex;
  align-items: center;
  gap: 12px;
}
.footer-actions {
  margin-left: auto;
}
.test-result {
  font-size: 12px;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.test-result.ok {
  color: var(--el-color-success);
}
.test-result.fail {
  color: var(--el-color-danger);
}
</style>
