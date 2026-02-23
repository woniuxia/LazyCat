<template>
  <div class="vault-panel">
    <!-- Lock screen -->
    <Transition name="fade" mode="out-in">
      <VaultLockScreen
        v-if="!unlocked"
        key="lock"
        :mode="vaultSetup ? 'unlock' : 'setup'"
        @unlocked="onUnlocked"
      />

      <!-- Main 2-column layout: nav + list -->
      <div v-else key="main" class="vault-main">
        <!-- Left: Navigation tree -->
        <aside class="vault-nav">
          <div class="vault-nav-header">
            <h3 class="vault-nav-title">密码库</h3>
            <span class="vault-nav-count">{{ entries.length }} 条凭据</span>
          </div>

          <div class="vault-nav-section">
            <div
              class="vault-nav-item"
              :class="{ 'is-active': !activeEnv && !activeCategory }"
              @click="clearFilter"
            >
              <span class="vault-nav-icon">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="3" y="3" width="18" height="18" rx="2" />
                  <path d="M3 9h18" />
                </svg>
              </span>
              <span class="vault-nav-label">全部</span>
              <span class="vault-nav-badge">{{ entries.length }}</span>
            </div>
          </div>

          <div class="vault-nav-section">
            <div class="vault-nav-section-title">按环境</div>
            <div
              v-for="env in ENV_LIST"
              :key="env.value"
              class="vault-nav-item"
              :class="{ 'is-active': activeEnv === env.value }"
              @click="onClickEnv(env.value)"
            >
              <span class="vault-nav-dot" :class="env.cls" />
              <span class="vault-nav-label">{{ env.value }}</span>
              <span class="vault-nav-badge">{{ envCount(env.value) }}</span>
            </div>
          </div>

          <div class="vault-nav-section">
            <div class="vault-nav-section-title">按分类</div>
            <div
              v-for="cat in CAT_LIST"
              :key="cat.value"
              class="vault-nav-item"
              :class="{ 'is-active': activeCategory === cat.value }"
              @click="onClickCat(cat.value)"
            >
              <span class="vault-nav-icon">
                <svg v-if="cat.value === 'app'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="2" y="3" width="20" height="14" rx="2" />
                  <path d="M8 21h8" />
                  <path d="M12 17v4" />
                </svg>
                <svg v-else-if="cat.value === 'server'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="2" y="2" width="20" height="8" rx="2" />
                  <rect x="2" y="14" width="20" height="8" rx="2" />
                  <path d="M6 6h.01" />
                  <path d="M6 18h.01" />
                </svg>
                <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <ellipse cx="12" cy="5" rx="9" ry="3" />
                  <path d="M3 5v14a9 3 0 0 0 18 0V5" />
                </svg>
              </span>
              <span class="vault-nav-label">{{ cat.label }}</span>
              <span class="vault-nav-badge">{{ catCount(cat.value) }}</span>
            </div>
          </div>

          <div class="vault-nav-spacer" />

          <div class="vault-nav-actions">
            <button class="vault-nav-btn" @click="showChangePassword = true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="11" width="18" height="11" rx="2" />
                <path d="M7 11V7a5 5 0 0 1 10 0v4" />
              </svg>
              <span>修改密码</span>
            </button>
            <button class="vault-nav-btn" @click="onLock">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="11" width="18" height="11" rx="2" />
                <path d="M7 11V7a5 5 0 0 1 9.9-1" />
              </svg>
              <span>锁定</span>
            </button>
          </div>
        </aside>

        <!-- Right: List area -->
        <div class="vault-content">
          <div class="vault-toolbar">
            <div class="vault-search">
              <svg class="vault-search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.3-4.3" />
              </svg>
              <input
                v-model="keyword"
                type="text"
                placeholder="搜索凭据..."
                class="vault-search-input"
              />
              <button v-if="keyword" class="vault-search-clear" @click="keyword = ''">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M18 6 6 18" />
                  <path d="m6 6 12 12" />
                </svg>
              </button>
            </div>
            <button class="vault-btn-primary" @click="onCreateEntry">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 5v14" />
                <path d="M5 12h14" />
              </svg>
              <span>新建</span>
            </button>
          </div>

          <div v-if="filteredEntries.length" class="vault-list-container">
            <div class="vault-list-header">
              <div class="vault-list-col env">环境</div>
              <div class="vault-list-col type">类型</div>
              <div class="vault-list-col name">名称</div>
              <div class="vault-list-col account">账号</div>
              <div class="vault-list-col password">密码</div>
              <div class="vault-list-col actions"></div>
            </div>

            <div class="vault-list-body">
              <div
                v-for="(entry, index) in filteredEntries"
                :key="entry.id"
                class="vault-list-item"
                :style="{ animationDelay: `${index * 30}ms` }"
              >
                <div class="vault-list-col env">
                  <span v-if="entry.environment" class="vault-tag" :class="envClass(entry.environment)">
                    <span class="vault-tag-dot" />
                    {{ entry.environment }}
                  </span>
                  <span v-else class="vault-tag-placeholder">—</span>
                </div>

                <div class="vault-list-col type">
                  <span class="vault-type-label">{{ categoryLabel(entry.category) }}</span>
                </div>

                <div class="vault-list-col name">
                  <span class="vault-entry-title" :title="entry.title">{{ entry.title || '(未命名)' }}</span>
                  <span v-if="entry.summary" class="vault-entry-summary" :title="entry.summary">{{ entry.summary }}</span>
                </div>

                <div class="vault-list-col account">
                  <button
                    class="vault-account-btn"
                    :class="{ 'is-copying': copyFeedbackAccount === entry.id }"
                    :disabled="!entry.account"
                    @click.stop="onCopyAccount(entry)"
                  >
                    <span class="vault-account-text">{{ entry.account || '—' }}</span>
                  </button>
                </div>

                <div class="vault-list-col password">
                  <div
                    class="vault-password-cell"
                    :class="{ 'is-copying': copyFeedbackRow === entry.id, 'is-revealed': revealedPasswords.has(entry.id) }"
                  >
                    <button
                      class="vault-password-text-btn"
                      @click.stop="onTogglePassword(entry)"
                    >
                      <Transition name="pw-text" mode="out-in">
                        <span
                          v-if="revealedPasswords.has(entry.id)"
                          key="revealed"
                          class="vault-password-text"
                        >{{ revealedPasswords.get(entry.id) || '(空)' }}</span>
                        <span v-else key="masked" class="vault-password-dots">••••••</span>
                      </Transition>
                    </button>
                    <button
                      class="vault-password-copy-btn"
                      title="复制密码"
                      @click.stop="onDirectCopyPassword(entry)"
                    >
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <rect width="14" height="14" x="8" y="8" rx="2" />
                        <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
                      </svg>
                    </button>
                  </div>
                </div>

                <div class="vault-list-col actions">
                  <div class="vault-actions-group">
                    <button class="vault-action-btn" title="编辑" @click="onEditEntry(entry)">
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
                      </svg>
                    </button>
                    <button class="vault-action-btn danger" title="删除" @click="onDeleteEntry(entry)">
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M3 6h18" />
                        <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                        <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div v-else class="vault-empty">
            <div class="vault-empty-icon">
              <svg viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="1.5">
                <rect x="8" y="14" width="32" height="28" rx="4" />
                <path d="M16 14V10a8 8 0 0 1 16 0v4" />
                <circle cx="24" cy="28" r="3" />
                <path d="M24 31v4" />
              </svg>
            </div>
            <p v-if="keyword" class="vault-empty-text">没有匹配的凭据</p>
            <template v-else>
              <p class="vault-empty-title">还没有凭据</p>
              <p class="vault-empty-desc">创建第一条凭据来开始使用密码库</p>
              <button class="vault-btn-primary" @click="onCreateEntry">创建第一条</button>
            </template>
          </div>
        </div>
      </div>
    </Transition>

    <VaultEntryDialog ref="entryDialog" @saved="onEntrySaved" />

    <!-- Change password dialog -->
    <el-dialog v-model="showChangePassword" title="修改主密码" width="400px" :close-on-click-modal="false" class="vault-dialog">
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
        <p v-if="changePwError" class="vault-error-text">{{ changePwError }}</p>
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
  { value: "本地", cls: "is-local" },
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
const copyFeedbackRow = ref<number | null>(null);
const copyFeedbackAccount = ref<number | null>(null);

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
  // 如果选择了分类，显示该分类下该环境的数量
  if (activeCategory.value) {
    return entries.value.filter((e) => e.environment === env && e.category === activeCategory.value).length;
  }
  return entries.value.filter((e) => e.environment === env).length;
}

function catCount(cat: string) {
  // 如果选择了环境，显示该环境下该分类的数量
  if (activeEnv.value) {
    return entries.value.filter((e) => e.category === cat && e.environment === activeEnv.value).length;
  }
  return entries.value.filter((e) => e.category === cat).length;
}

function clearFilter() {
  activeEnv.value = "";
  activeCategory.value = "";
}

function onClickEnv(env: string) {
  if (activeEnv.value === env) {
    activeEnv.value = "";
    return;
  }
  activeEnv.value = env;
}

function onClickCat(cat: string) {
  if (activeCategory.value === cat) {
    activeCategory.value = "";
    return;
  }
  activeCategory.value = cat;
}

function categoryLabel(cat: string) {
  const map: Record<string, string> = { app: "应用系统", server: "服务器", database: "数据库" };
  return map[cat] || cat;
}

function envClass(env: string) {
  if (env === "生产") return "is-prod";
  if (env === "测试") return "is-dev";
  if (env === "本地") return "is-local";
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
  if (revealedPasswords.has(entry.id)) {
    revealedPasswords.delete(entry.id);
    return;
  }
  try {
    const res = (await invokeToolByChannel("tool:vault:get", { id: entry.id })) as VaultDetail;
    const pw = String(res.fields?.password ?? "");
    revealedPasswords.set(entry.id, pw);
  } catch (err) {
    handleVaultError(err);
  }
}

async function onDirectCopyPassword(entry: VaultListEntry) {
  try {
    // 如果密码已经揭示，直接使用缓存的密码
    let pw = revealedPasswords.get(entry.id);
    if (!pw) {
      const res = (await invokeToolByChannel("tool:vault:get", { id: entry.id })) as VaultDetail;
      pw = String(res.fields?.password ?? "");
    }
    if (!pw) {
      ElMessage.warning("密码为空");
      return;
    }
    await navigator.clipboard.writeText(pw);
    copyFeedbackRow.value = entry.id;
    setTimeout(() => {
      if (copyFeedbackRow.value === entry.id) {
        copyFeedbackRow.value = null;
      }
    }, 1500);
    ElMessage.success("密码已复制");
    if (pwClipboardTimer) clearTimeout(pwClipboardTimer);
    pwClipboardTimer = setTimeout(async () => {
      try {
        const current = await navigator.clipboard.readText();
        if (current === pw) await navigator.clipboard.writeText("");
      } catch {
        try { await navigator.clipboard.writeText(""); } catch { /* ignore */ }
      }
    }, 30_000);
  } catch (err) {
    handleVaultError(err);
    ElMessage.error("复制失败");
  }
}

async function onCopyAccount(entry: VaultListEntry) {
  if (!entry.account) return;
  try {
    await navigator.clipboard.writeText(entry.account);
    copyFeedbackAccount.value = entry.id;
    setTimeout(() => {
      if (copyFeedbackAccount.value === entry.id) {
        copyFeedbackAccount.value = null;
      }
    }, 1500);
    ElMessage.success("账号已复制");
  } catch {
    ElMessage.error("无法写入剪贴板");
  }
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
  } catch {
    return;
  }
  try {
    await invokeToolByChannel("tool:vault:delete", { id: entry.id });
    await loadEntries();
  } catch (err) {
    handleVaultError(err);
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
  } else if (msg) {
    ElMessage.error(msg);
  }
}

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

function hideRevealedPasswords(event: MouseEvent) {
  if (revealedPasswords.size === 0) return;
  const target = event.target as HTMLElement;
  if (target.closest('.vault-password-cell, .vault-password-copy-btn')) return;
  revealedPasswords.clear();
}

onMounted(() => {
  checkStatus();
  startAutoLockCheck();
  document.addEventListener("click", hideRevealedPasswords);
});

onBeforeUnmount(() => {
  if (activityTimer) clearInterval(activityTimer);
  if (pwClipboardTimer) clearTimeout(pwClipboardTimer);
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
  grid-template-columns: 220px 1fr;
  width: 100%;
  height: 100%;
  gap: 0;
}

/* --- Navigation --- */
.vault-nav {
  display: flex;
  flex-direction: column;
  padding: 20px 12px;
  border-right: 1px solid var(--lc-border);
  background: var(--lc-surface-0);
  overflow-y: auto;
}

.vault-nav-header {
  padding: 0 8px 16px;
  margin-bottom: 8px;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.vault-nav-title {
  margin: 0 0 4px;
  font-family: var(--lc-font-display);
  font-size: 16px;
  font-weight: 600;
  color: var(--lc-text);
}

.vault-nav-count {
  font-size: 12px;
  color: var(--lc-text-muted);
}

.vault-nav-section {
  margin-bottom: 4px;
}

.vault-nav-section-title {
  padding: 12px 8px 8px;
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--lc-text-muted);
}

.vault-nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: var(--lc-radius-sm);
  cursor: pointer;
  font-size: 13px;
  color: var(--lc-text-secondary);
  transition: all 150ms var(--lc-ease);
  user-select: none;
}

.vault-nav-item:hover {
  background: var(--lc-surface-1);
  color: var(--lc-text);
}

.vault-nav-item.is-active {
  background: var(--lc-accent-dim);
  color: var(--lc-accent);
}

.vault-nav-icon {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  opacity: 0.7;
}

.vault-nav-icon svg {
  width: 100%;
  height: 100%;
}

.vault-nav-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--lc-surface-3);
}

.vault-nav-dot.is-prod {
  background: #f87171;
}

.vault-nav-dot.is-dev {
  background: #34d399;
}

.vault-nav-dot.is-local {
  background: #60a5fa;
}

.vault-nav-label {
  flex: 1;
}

.vault-nav-badge {
  font-size: 11px;
  min-width: 18px;
  padding: 2px 6px;
  text-align: center;
  background: var(--lc-surface-2);
  border-radius: 10px;
  color: var(--lc-text-secondary);
}

.vault-nav-item.is-active .vault-nav-badge {
  background: var(--lc-accent);
  color: var(--lc-bg);
}

.vault-nav-spacer {
  flex: 1;
}

.vault-nav-actions {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding-top: 12px;
  border-top: 1px solid var(--lc-border-subtle);
}

.vault-nav-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border: none;
  border-radius: var(--lc-radius-sm);
  background: transparent;
  color: var(--lc-text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: all 150ms var(--lc-ease);
}

.vault-nav-btn:hover {
  background: var(--lc-surface-1);
  color: var(--lc-text);
}

.vault-nav-btn svg {
  width: 14px;
  height: 14px;
}

/* --- Content Area --- */
.vault-content {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--lc-surface-0);
}

/* --- Toolbar --- */
.vault-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--lc-border);
  flex-shrink: 0;
}

.vault-search {
  flex: 1;
  position: relative;
  max-width: 320px;
}

.vault-search-icon {
  position: absolute;
  left: 12px;
  top: 50%;
  transform: translateY(-50%);
  width: 16px;
  height: 16px;
  color: var(--lc-text-muted);
  pointer-events: none;
}

.vault-search-input {
  width: 100%;
  padding: 10px 36px 10px 38px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-1);
  color: var(--lc-text);
  font-size: 14px;
  transition: all 150ms var(--lc-ease);
}

.vault-search-input::placeholder {
  color: var(--lc-text-muted);
}

.vault-search-input:hover {
  border-color: var(--lc-border-hover);
}

.vault-search-input:focus {
  outline: none;
  border-color: var(--lc-accent);
  box-shadow: 0 0 0 3px var(--lc-accent-dim);
}

.vault-search-clear {
  position: absolute;
  right: 10px;
  top: 50%;
  transform: translateY(-50%);
  width: 18px;
  height: 18px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: var(--lc-surface-3);
  color: var(--lc-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 150ms var(--lc-ease);
}

.vault-search-clear:hover {
  background: var(--lc-text-muted);
  color: var(--lc-bg);
}

.vault-search-clear svg {
  width: 12px;
  height: 12px;
}

.vault-btn-primary {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 16px;
  border: none;
  border-radius: var(--lc-radius-md);
  background: var(--lc-accent);
  color: var(--lc-bg);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 150ms var(--lc-ease);
}

.vault-btn-primary:hover {
  background: var(--lc-accent-light);
  transform: translateY(-1px);
}

.vault-btn-primary:active {
  transform: translateY(0);
}

.vault-btn-primary svg {
  width: 16px;
  height: 16px;
}

/* --- List Container --- */
.vault-list-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.vault-list-header {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 12px 20px;
  border-bottom: 1px solid var(--lc-border);
  background: var(--lc-surface-1);
  font-size: 12px;
  font-weight: 500;
  color: var(--lc-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.vault-list-body {
  flex: 1;
  overflow-y: auto;
  padding: 8px 12px;
}

.vault-list-item {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 14px 8px;
  margin-bottom: 4px;
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-0);
  border: 1px solid transparent;
  animation: itemEnter 300ms var(--lc-ease) both;
  transition: all 150ms var(--lc-ease);
}

@keyframes itemEnter {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.vault-list-item:hover {
  background: var(--lc-surface-1);
  border-color: var(--lc-border);
}

.vault-list-item.is-copying {
  background: var(--lc-accent-dim);
  border-color: var(--lc-accent);
}

.vault-list-col {
  padding: 0 8px;
  text-align: center;
  display: flex;
  align-items: center;
  justify-content: center;
}

.vault-list-col.env {
  width: 90px;
  flex-shrink: 0;
}

.vault-list-col.type {
  width: 80px;
  flex-shrink: 0;
}

.vault-list-col.name {
  width: 320px;
  flex-shrink: 0;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.vault-list-col.account {
  width: 160px;
  flex-shrink: 0;
  min-width: 0;
}

.vault-list-col.password {
  width: 220px;
  flex-shrink: 0;
}

.vault-list-col.actions {
  width: 80px;
  flex-shrink: 0;
}

/* --- Environment Tag --- */
.vault-tag {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 20px;
  font-size: 12px;
  font-weight: 500;
  background: var(--lc-surface-2);
  color: var(--lc-text-secondary);
}

.vault-tag.is-prod {
  background: rgba(248, 113, 113, 0.15);
  color: #f87171;
}

.vault-tag.is-dev {
  background: rgba(52, 211, 153, 0.15);
  color: #34d399;
}

.vault-tag.is-local {
  background: rgba(96, 165, 250, 0.15);
  color: #60a5fa;
}

.vault-tag-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}

.vault-tag-placeholder {
  color: var(--lc-text-muted);
}

/* --- Type Label --- */
.vault-type-label {
  font-size: 12px;
  color: var(--lc-text-secondary);
}

/* --- Entry Title & Summary --- */
.vault-entry-title {
  display: block;
  width: 100%;
  font-size: 14px;
  font-weight: 500;
  color: var(--lc-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: center;
}

.vault-entry-summary {
  display: block;
  width: 100%;
  font-size: 12px;
  color: var(--lc-text-muted);
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: center;
}

/* --- Account Button --- */
.vault-account-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 10px;
  border: 1px solid transparent;
  border-radius: var(--lc-radius-sm);
  background: transparent;
  color: var(--lc-text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: all 150ms var(--lc-ease);
  max-width: 100%;
}

.vault-account-btn:hover:not(:disabled) {
  border-color: var(--lc-border);
  background: var(--lc-surface-1);
}

.vault-account-btn.is-copying {
  background: var(--lc-accent-dim);
  border-color: var(--lc-accent);
}

.vault-account-btn:disabled {
  cursor: default;
  opacity: 0.5;
}

.vault-account-text {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--lc-font-body);
  letter-spacing: 0.3px;
}

/* --- Password Cell --- */
.vault-password-cell {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
  width: 160px;
  height: 32px;
  padding: 0 6px 0 12px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-sm);
  background: var(--lc-surface-1);
  color: var(--lc-text-muted);
  font-size: 13px;
  transition: all 150ms var(--lc-ease);
  box-sizing: border-box;
  vertical-align: middle;
  position: relative;
}

.vault-password-cell:hover {
  border-color: var(--lc-accent);
}

.vault-password-cell.is-copying {
  background: var(--lc-accent-dim);
  border-color: var(--lc-accent);
}

.vault-password-cell.is-revealed {
  background: var(--lc-surface-2);
  color: var(--lc-text);
}

.vault-password-text-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: none;
  background: transparent;
  color: inherit;
  font-size: inherit;
  cursor: pointer;
  min-width: 0;
}

.vault-password-text {
  max-width: 100px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--lc-font-mono);
  font-size: 12px;
}

.vault-password-dots {
  letter-spacing: 2px;
  font-family: var(--lc-font-mono);
}

.vault-password-copy-btn {
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: var(--lc-radius-sm);
  background: transparent;
  color: var(--lc-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: all 150ms var(--lc-ease);
  flex-shrink: 0;
}

.vault-password-cell:hover .vault-password-copy-btn {
  opacity: 1;
}

.vault-password-copy-btn:hover {
  background: var(--lc-accent);
  color: var(--lc-bg);
}

.vault-password-copy-btn svg {
  width: 14px;
  height: 14px;
}

.pw-text-enter-active,
.pw-text-leave-active {
  transition: opacity 100ms var(--lc-ease);
}
.pw-text-enter-from,
.pw-text-leave-to {
  opacity: 0;
}

/* --- Actions --- */
.vault-actions-group {
  display: flex;
  gap: 4px;
  justify-content: center;
  opacity: 0;
  transition: opacity 150ms var(--lc-ease);
}

.vault-list-item:hover .vault-actions-group {
  opacity: 1;
}

.vault-action-btn {
  width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  border-radius: var(--lc-radius-sm);
  background: transparent;
  color: var(--lc-text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 150ms var(--lc-ease);
}

.vault-action-btn:hover {
  background: var(--lc-surface-2);
  color: var(--lc-text);
}

.vault-action-btn.danger:hover {
  background: rgba(248, 113, 113, 0.15);
  color: #f87171;
}

.vault-action-btn svg {
  width: 14px;
  height: 14px;
}

/* --- Empty State --- */
.vault-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: var(--lc-text-secondary);
}

.vault-empty-icon {
  width: 64px;
  height: 64px;
  margin-bottom: 16px;
  color: var(--lc-text-muted);
}

.vault-empty-icon svg {
  width: 100%;
  height: 100%;
}

.vault-empty-title {
  margin: 0 0 4px;
  font-size: 16px;
  font-weight: 500;
  color: var(--lc-text);
}

.vault-empty-desc {
  margin: 0 0 20px;
  font-size: 14px;
  color: var(--lc-text-muted);
}

.vault-empty-text {
  margin: 0;
  font-size: 14px;
  color: var(--lc-text-muted);
}

/* --- Error Text --- */
.vault-error-text {
  color: var(--el-color-danger);
  font-size: 13px;
  margin: 0;
}

/* --- Dialog --- */
.vault-dialog :deep(.el-dialog__header) {
  margin-right: 0;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--lc-border);
}

.vault-dialog :deep(.el-dialog__title) {
  font-family: var(--lc-font-display);
  font-size: 18px;
  font-weight: 600;
  color: var(--lc-text);
}

/* --- Transitions --- */
.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--lc-duration-slow) var(--lc-ease);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

</style>
