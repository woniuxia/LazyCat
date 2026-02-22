<template>
  <div class="vault-panel">
    <!-- Lock screen -->
    <VaultLockScreen
      v-if="!unlocked"
      :mode="vaultSetup ? 'unlock' : 'setup'"
      @unlocked="onUnlocked"
    />

    <!-- Main 2-column layout: nav + table -->
    <div v-else class="vault-main">
      <!-- Left: Navigation tree -->
      <aside class="vault-nav">
        <div
          class="vault-nav-item vault-nav-item--root"
          :class="{ 'is-active': !activeEnv && !activeCategory }"
          @click="clearFilter"
        >
          <span>全部</span>
          <span class="vault-nav-item__count">{{ entries.length }}</span>
        </div>

        <template v-for="env in ENV_LIST" :key="env.value">
          <div
            class="vault-nav-item vault-nav-item--env"
            :class="{ 'is-active': activeEnv === env.value && !activeCategory }"
            @click="onClickEnv(env.value)"
          >
            <span class="vault-nav-item__dot" :class="env.cls" />
            <span class="vault-nav-item__label">{{ env.value }}</span>
            <span class="vault-nav-item__count">{{ envCount(env.value) }}</span>
          </div>
          <div
            v-for="cat in CAT_LIST"
            :key="cat.value"
            class="vault-nav-item vault-nav-item--cat"
            :class="{ 'is-active': activeEnv === env.value && activeCategory === cat.value }"
            @click.stop="onClickCat(env.value, cat.value)"
          >
            <span>{{ cat.label }}</span>
            <span class="vault-nav-item__count">{{ envCatCount(env.value, cat.value) }}</span>
          </div>
        </template>

        <div class="vault-nav__spacer" />

        <div class="vault-nav__actions">
          <el-button text size="small" @click="showChangePassword = true">修改密码</el-button>
          <el-button text size="small" @click="onLock">锁定</el-button>
        </div>
      </aside>

      <!-- Right: Table area -->
      <div class="vault-table-area">
        <div class="vault-table-area__header">
          <el-input
            v-model="keyword"
            placeholder="搜索凭据..."
            clearable
            class="vault-table-area__search"
          />
          <el-button type="primary" @click="onCreateEntry">新建</el-button>
        </div>

        <div class="vault-table-area__body" v-if="filteredEntries.length">
          <el-table
            :data="filteredEntries"
            stripe
            size="small"
            style="width: 100%"
            height="100%"
            :header-cell-style="{ background: 'var(--lc-surface-1)', color: 'var(--lc-text-secondary)', fontSize: '12px' }"
          >
            <el-table-column label="环境" width="70" align="center">
              <template #default="{ row }">
                <span
                  v-if="row.environment"
                  class="vault-env-tag"
                  :class="envClass(row.environment)"
                >{{ row.environment }}</span>
              </template>
            </el-table-column>

            <el-table-column label="类型" width="80" align="center">
              <template #default="{ row }">
                {{ categoryLabel(row.category) }}
              </template>
            </el-table-column>

            <el-table-column label="名称" min-width="140" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.title || '(未命名)' }}
              </template>
            </el-table-column>

            <el-table-column label="账号" min-width="140" show-overflow-tooltip prop="account" />

            <el-table-column label="密码" width="120">
              <template #default="{ row }">
                <span
                  v-if="revealedPasswords.has(row.id)"
                  class="vault-pw-text"
                  title="点击复制"
                  @click.stop="onCopyPassword(row)"
                >{{ revealedPasswords.get(row.id) || '(空)' }}</span>
                <span
                  v-else
                  class="vault-pw-mask"
                  title="点击显示密码"
                  @click.stop="onTogglePassword(row)"
                >******</span>
              </template>
            </el-table-column>

            <el-table-column label="其他信息" min-width="200" show-overflow-tooltip prop="summary" />

            <el-table-column label="操作" width="120" fixed="right" align="center">
              <template #default="{ row }">
                <el-button text size="small" @click="onEditEntry(row)">编辑</el-button>
                <el-button text size="small" type="danger" @click="onDeleteEntry(row)">删除</el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
        <div v-else class="vault-table-area__empty">
          <p v-if="keyword">没有匹配的凭据</p>
          <template v-else>
            <p>还没有凭据</p>
            <el-button type="primary" text @click="onCreateEntry">创建第一条</el-button>
          </template>
        </div>
      </div>
    </div>

    <VaultEntryDialog ref="entryDialog" @saved="onEntrySaved" />

    <!-- Change password dialog -->
    <el-dialog v-model="showChangePassword" title="修改主密码" width="400px" :close-on-click-modal="false">
      <el-form label-position="top">
        <el-form-item label="当前密码">
          <el-input v-model="changePw.current" type="password" show-password />
        </el-form-item>
        <el-form-item label="新密码">
          <el-input v-model="changePw.newPw" type="password" show-password />
        </el-form-item>
        <el-form-item label="确认新密码">
          <el-input v-model="changePw.confirm" type="password" show-password />
        </el-form-item>
        <p v-if="changePwError" style="color: var(--el-color-danger); font-size: 13px; margin: 0;">{{ changePwError }}</p>
      </el-form>
      <template #footer>
        <el-button @click="showChangePassword = false">取消</el-button>
        <el-button type="primary" :loading="changePwLoading" @click="onChangePassword">确认修改</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onBeforeUnmount } from "vue";
import { ElMessageBox, ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import VaultLockScreen from "./VaultLockScreen.vue";
import VaultEntryDialog from "./VaultEntryDialog.vue";

interface VaultListEntry {
  id: number;
  category: string;
  title: string;
  environment: string;
  account: string;
  summary: string;
  createdAt: string;
  updatedAt: string;
}

interface VaultDetail {
  id: number;
  category: string;
  title: string;
  environment: string;
  fields: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

const vaultSetup = ref(false);
const unlocked = ref(false);
const entries = ref<VaultListEntry[]>([]);
const keyword = ref("");
const activeEnv = ref("");
const activeCategory = ref("");
const entryDialog = ref<InstanceType<typeof VaultEntryDialog> | null>(null);

const ENV_LIST = [
  { value: "生产", cls: "is-prod" },
  { value: "测试", cls: "is-dev" },
  { value: "本地", cls: "is-dev" },
] as const;

const CAT_LIST = [
  { value: "app", label: "应用系统" },
  { value: "server", label: "服务器" },
  { value: "database", label: "数据库" },
] as const;

// Change password
const showChangePassword = ref(false);
const changePwLoading = ref(false);
const changePwError = ref("");
const changePw = reactive({ current: "", newPw: "", confirm: "" });

// Auto-lock timer
let activityTimer: ReturnType<typeof setInterval> | null = null;

// Password reveal/copy state
const revealedPasswords = reactive(new Map<number, string>());
let pwClipboardTimer: ReturnType<typeof setTimeout> | null = null;

const filteredEntries = computed(() => {
  let list = entries.value;
  if (activeEnv.value) {
    list = list.filter((e) => e.environment === activeEnv.value);
  }
  if (activeCategory.value) {
    list = list.filter((e) => e.category === activeCategory.value);
  }
  if (keyword.value) {
    const kw = keyword.value.toLowerCase();
    list = list.filter((e) =>
      e.title.toLowerCase().includes(kw) ||
      e.account.toLowerCase().includes(kw) ||
      e.summary.toLowerCase().includes(kw) ||
      e.environment.toLowerCase().includes(kw) ||
      categoryLabel(e.category).toLowerCase().includes(kw)
    );
  }
  return list;
});

function envCount(env: string) {
  return entries.value.filter((e) => e.environment === env).length;
}

function envCatCount(env: string, cat: string) {
  return entries.value.filter((e) => e.environment === env && e.category === cat).length;
}

function clearFilter() {
  activeEnv.value = "";
  activeCategory.value = "";
}

function onClickEnv(env: string) {
  if (activeEnv.value === env && !activeCategory.value) {
    clearFilter();
    return;
  }
  activeEnv.value = env;
  activeCategory.value = "";
}

function onClickCat(env: string, cat: string) {
  activeEnv.value = env;
  activeCategory.value = cat;
}

function categoryLabel(cat: string) {
  const map: Record<string, string> = { app: "应用系统", server: "服务器", database: "数据库" };
  return map[cat] || cat;
}

function envClass(env: string) {
  if (env === "生产") return "is-prod";
  if (env === "测试" || env === "本地") return "is-dev";
  return "";
}

async function checkStatus() {
  try {
    const res = (await invokeToolByChannel("tool:vault:status", {})) as {
      setup: boolean;
      unlocked: boolean;
    };
    vaultSetup.value = res.setup;
    unlocked.value = res.unlocked;
    if (res.unlocked) {
      await loadEntries();
    }
  } catch {
    // IPC not available
  }
}

async function onUnlocked() {
  unlocked.value = true;
  vaultSetup.value = true;
  await loadEntries();
}

async function loadEntries() {
  try {
    const res = (await invokeToolByChannel("tool:vault:list", {})) as VaultListEntry[];
    entries.value = res;
  } catch (err) {
    handleVaultError(err);
  }
}

function onCreateEntry() {
  entryDialog.value?.show();
}

async function onTogglePassword(entry: VaultListEntry) {
  try {
    const res = (await invokeToolByChannel("tool:vault:get", { id: entry.id })) as VaultDetail;
    const pw = String(res.fields?.password ?? "");
    revealedPasswords.set(entry.id, pw);
  } catch (err) {
    handleVaultError(err);
  }
}

async function onCopyPassword(entry: VaultListEntry) {
  const pw = revealedPasswords.get(entry.id);
  if (!pw) return;
  try {
    await navigator.clipboard.writeText(pw);
    ElMessage.success("密码已复制");
    if (pwClipboardTimer) clearTimeout(pwClipboardTimer);
    pwClipboardTimer = setTimeout(async () => {
      try {
        const current = await navigator.clipboard.readText();
        if (current === pw) await navigator.clipboard.writeText("");
      } catch { /* ignore */ }
    }, 30_000);
  } catch { /* ignore */ }
}

async function onEditEntry(entry: VaultListEntry) {
  try {
    const res = (await invokeToolByChannel("tool:vault:get", { id: entry.id })) as VaultDetail;
    entryDialog.value?.show({
      id: res.id,
      category: res.category,
      title: res.title,
      environment: res.environment,
      fields: res.fields,
    });
  } catch (err) {
    handleVaultError(err);
  }
}

async function onDeleteEntry(entry: VaultListEntry) {
  try {
    await ElMessageBox.confirm("确定要删除这条凭据吗？此操作不可撤销。", "删除确认", {
      confirmButtonText: "删除",
      cancelButtonText: "取消",
      type: "warning",
    });
    await invokeToolByChannel("tool:vault:delete", { id: entry.id });
    await loadEntries();
  } catch {
    // cancelled
  }
}

async function onEntrySaved() {
  await loadEntries();
}

async function onLock() {
  try {
    await invokeToolByChannel("tool:vault:lock", {});
  } catch {
    // ignore
  }
  unlocked.value = false;
  entries.value = [];
  revealedPasswords.clear();
}

async function onChangePassword() {
  changePwError.value = "";
  if (!changePw.current || !changePw.newPw || !changePw.confirm) {
    changePwError.value = "请填写所有字段";
    return;
  }
  if (changePw.newPw !== changePw.confirm) {
    changePwError.value = "两次输入的新密码不一致";
    return;
  }
  if (changePw.newPw.length < 4) {
    changePwError.value = "密码长度不能少于 4 位";
    return;
  }
  changePwLoading.value = true;
  try {
    await invokeToolByChannel("tool:vault:change-password", {
      currentPassword: changePw.current,
      newPassword: changePw.newPw,
    });
    showChangePassword.value = false;
    changePw.current = "";
    changePw.newPw = "";
    changePw.confirm = "";
  } catch (err) {
    const msg = (err as Error).message || "";
    if (msg.includes("wrong_password")) {
      changePwError.value = "当前密码错误";
    } else {
      changePwError.value = msg || "修改失败";
    }
  } finally {
    changePwLoading.value = false;
  }
}

function handleVaultError(err: unknown) {
  const msg = (err as Error).message || "";
  if (msg.includes("vault_locked") || msg.includes("vault_locked_timeout")) {
    unlocked.value = false;
    entries.value = [];
    revealedPasswords.clear();
  }
}

// Periodic status check for auto-lock
function startAutoLockCheck() {
  activityTimer = setInterval(async () => {
    if (!unlocked.value) return;
    try {
      const res = (await invokeToolByChannel("tool:vault:status", {})) as {
        setup: boolean;
        unlocked: boolean;
      };
      if (!res.unlocked) {
        unlocked.value = false;
        entries.value = [];
        revealedPasswords.clear();
      }
    } catch {
      // ignore
    }
  }, 30_000);
}

function hideRevealedPasswords() {
  if (revealedPasswords.size > 0) {
    revealedPasswords.clear();
  }
}

onMounted(() => {
  checkStatus();
  startAutoLockCheck();
  document.addEventListener("click", hideRevealedPasswords);
});

onBeforeUnmount(() => {
  if (activityTimer) clearInterval(activityTimer);
  document.removeEventListener("click", hideRevealedPasswords);
});
</script>

<style scoped>
.vault-panel {
  width: 100%;
  height: 100%;
  display: flex;
}

.vault-main {
  display: grid;
  grid-template-columns: 180px 1fr;
  width: 100%;
  height: 100%;
  gap: 0;
}

/* --- Navigation tree --- */
.vault-nav {
  display: flex;
  flex-direction: column;
  padding: 12px 8px;
  border-right: 1px solid var(--lc-border);
  background: var(--lc-surface-1);
  overflow-y: auto;
}

.vault-nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 12px;
  border-radius: var(--lc-radius-sm);
  cursor: pointer;
  font-size: 13px;
  color: var(--lc-text-secondary);
  transition: background 150ms ease, color 150ms ease;
  user-select: none;
}

.vault-nav-item:hover {
  background: var(--lc-surface-2);
  color: var(--lc-text);
}

.vault-nav-item.is-active {
  background: var(--lc-accent-dim);
  color: var(--lc-accent);
  font-weight: 600;
}

.vault-nav-item--root {
  margin-bottom: 4px;
  font-weight: 500;
}

.vault-nav-item--env {
  margin-top: 2px;
}

.vault-nav-item--cat {
  padding-left: 34px;
  font-size: 12px;
}

.vault-nav-item__dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--lc-surface-3);
}

.vault-nav-item__dot.is-prod {
  background: #e35d5d;
}

.vault-nav-item__dot.is-dev {
  background: #49b45e;
}

.vault-nav-item__label {
  flex: 1;
}

.vault-nav-item__count {
  font-size: 11px;
  min-width: 18px;
  text-align: center;
  opacity: 0.6;
}

.vault-nav__spacer {
  flex: 1;
}

.vault-nav__actions {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-top: 12px;
  border-top: 1px solid var(--lc-border);
}

/* --- Table area --- */
.vault-table-area {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.vault-table-area__header {
  display: flex;
  gap: 8px;
  padding: 12px;
  border-bottom: 1px solid var(--lc-border);
  background: var(--lc-surface-1);
  flex-shrink: 0;
}

.vault-table-area__search {
  flex: 1;
}

.vault-table-area__body {
  flex: 1;
  overflow: hidden;
}

.vault-table-area__empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--lc-text-secondary);
  font-size: 13px;
  gap: 8px;
}

/* --- Environment tags --- */
.vault-env-tag {
  display: inline-block;
  padding: 0 6px;
  border-radius: 3px;
  font-size: 10px;
  font-weight: 600;
  line-height: 18px;
  color: #fff;
  background: var(--lc-surface-2);
}

.vault-env-tag.is-prod {
  background: #e35d5d;
}

.vault-env-tag.is-dev {
  background: #49b45e;
}

/* --- Password cell --- */
.vault-pw-mask,
.vault-pw-text {
  display: inline-block;
  font-size: 12px;
  line-height: 22px;
  height: 22px;
  vertical-align: middle;
  cursor: pointer;
}

.vault-pw-mask {
  color: var(--lc-text-secondary);
  letter-spacing: 2px;
}

.vault-pw-mask:hover {
  color: var(--lc-accent);
}

.vault-pw-text {
  color: var(--lc-text);
  max-width: 100px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.vault-pw-text:hover {
  color: var(--lc-accent);
}
</style>
