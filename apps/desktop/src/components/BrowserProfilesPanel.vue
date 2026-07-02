<template>
  <div class="browser-profiles-panel">
    <header class="browser-profiles-toolbar">
      <div class="browser-profiles-title">
        <h2>浏览器身份</h2>
        <div class="browser-profiles-status" :class="{ 'is-found': response?.edgeFound }">
          <el-icon>
            <CircleCheck v-if="response?.edgeFound" />
            <WarningFilled v-else />
          </el-icon>
          <span>{{ edgeStatusText }}</span>
        </div>
      </div>

      <div class="browser-profiles-actions">
        <span class="browser-profiles-count">
          {{ profileCountText }}
        </span>
        <el-button :icon="Refresh" :loading="loading" @click="loadProfiles">刷新</el-button>
        <el-button
          :icon="FolderOpened"
          :type="response && !response.edgeFound ? 'primary' : 'default'"
          @click="chooseEdgePath"
        >
          选择 msedge.exe
        </el-button>
      </div>
    </header>

    <section v-if="errorMessage || responseWarnings.length || response?.edgeFound === false" class="browser-profiles-alerts">
      <div v-if="errorMessage" class="browser-profiles-alert is-error">
        {{ errorMessage }}
      </div>
      <div v-for="warning in responseWarnings" :key="warning" class="browser-profiles-alert">
        {{ warning }}
      </div>
      <div v-if="response?.edgeFound === false" class="browser-profiles-probed">
        <div class="browser-profiles-probed-title">未找到 Edge，可选择本机 msedge.exe。已检查：</div>
        <div v-for="path in response.probedEdgePaths" :key="path" class="browser-profiles-path">
          {{ path }}
        </div>
      </div>
    </section>

    <section v-if="totalProfileCount > 0" class="browser-profiles-filter">
      <el-input
        v-model="searchQuery"
        clearable
        :prefix-icon="Search"
        placeholder="搜索别名、Edge 名、Profile 目录或拼音首字母"
      />
    </section>

    <main class="browser-profiles-content" v-loading="loading">
      <div v-if="!loading && sortedProfiles.length === 0" class="browser-profiles-empty">
        未发现 Edge Profile
      </div>
      <div v-else-if="!loading && filteredProfiles.length === 0" class="browser-profiles-empty">
        没有匹配的浏览器身份
      </div>

      <template v-else>
        <section class="browser-profiles-list">
          <div
            v-for="profile in groupedProfiles.visible"
            :key="profileKey(profile)"
            class="browser-profile-row"
          >
            <div class="browser-profile-main">
              <div class="browser-profile-name" :title="getBrowserProfileDisplayName(profile)">
                {{ getBrowserProfileDisplayName(profile) }}
              </div>
              <div class="browser-profile-meta">
                <span v-if="profile.edgeDisplayName && profile.edgeDisplayName !== getBrowserProfileDisplayName(profile)">
                  Edge：{{ profile.edgeDisplayName }}
                </span>
                <span>{{ profile.profileDir }}</span>
              </div>
            </div>
            <div class="browser-profile-stats">
              <span>{{ profile.launchCount }} 次</span>
              <span>{{ formatBrowserProfileLastLaunchedAt(profile.lastLaunchedAt) }}</span>
            </div>
            <div class="browser-profile-actions">
              <el-button
                type="primary"
                size="small"
                :icon="VideoPlay"
                :loading="launchingKey === profileKey(profile)"
                :disabled="response?.edgeFound === false"
                @click="launchProfile(profile)"
              >
                启动
              </el-button>
              <el-button size="small" :icon="EditPen" @click="editAlias(profile)">别名</el-button>
              <el-button size="small" :icon="Hide" @click="setHidden(profile, true)">隐藏</el-button>
            </div>
          </div>
        </section>

        <section v-if="groupedProfiles.hidden.length" class="browser-profiles-hidden">
          <button
            class="browser-profiles-hidden-toggle"
            :class="{ 'is-searching': hasSearchQuery }"
            @click="toggleHiddenProfiles"
          >
            <span>{{ hasSearchQuery ? "隐藏身份匹配" : "已隐藏 Profile" }}</span>
            <span>{{ groupedProfiles.hidden.length }}</span>
          </button>

          <div v-if="hiddenListVisible" class="browser-profiles-list">
            <div
              v-for="profile in groupedProfiles.hidden"
              :key="profileKey(profile)"
              class="browser-profile-row is-hidden"
            >
              <div class="browser-profile-main">
                <div class="browser-profile-name" :title="getBrowserProfileDisplayName(profile)">
                  {{ getBrowserProfileDisplayName(profile) }}
                </div>
                <div class="browser-profile-meta">
                  <span v-if="profile.edgeDisplayName && profile.edgeDisplayName !== getBrowserProfileDisplayName(profile)">
                    Edge：{{ profile.edgeDisplayName }}
                  </span>
                  <span>{{ profile.profileDir }}</span>
                </div>
              </div>
              <div class="browser-profile-stats">
                <span>{{ profile.launchCount }} 次</span>
                <span>{{ formatBrowserProfileLastLaunchedAt(profile.lastLaunchedAt) }}</span>
              </div>
              <div class="browser-profile-actions">
                <el-button size="small" :icon="View" @click="setHidden(profile, false)">恢复</el-button>
                <el-button size="small" :icon="EditPen" @click="editAlias(profile)">别名</el-button>
              </div>
            </div>
          </div>
        </section>
      </template>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  CircleCheck,
  EditPen,
  FolderOpened,
  Hide,
  Refresh,
  Search,
  VideoPlay,
  View,
  WarningFilled,
} from "@element-plus/icons-vue";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  BrowserProfileItem,
  BrowserProfilesListResponse,
} from "../types/browser-profiles";
import {
  filterBrowserProfiles,
  formatBrowserProfileLastLaunchedAt,
  getBrowserProfileDisplayName,
  sortBrowserProfiles,
  splitBrowserProfilesByHidden,
} from "../utils/browserProfiles";

const loading = ref(false);
const launchingKey = ref("");
const response = ref<BrowserProfilesListResponse | null>(null);
const errorMessage = ref("");
const hiddenExpanded = ref(false);
const searchQuery = ref("");
let requestSeq = 0;

const sortedProfiles = computed(() => sortBrowserProfiles(response.value?.profiles ?? []));
const filteredProfiles = computed(() =>
  filterBrowserProfiles(sortedProfiles.value, searchQuery.value),
);
const groupedProfiles = computed(() => splitBrowserProfilesByHidden(filteredProfiles.value));
const responseWarnings = computed(() => response.value?.warnings ?? []);
const hasSearchQuery = computed(() => searchQuery.value.trim().length > 0);
const hiddenListVisible = computed(() => hiddenExpanded.value || hasSearchQuery.value);
const totalProfileCount = computed(() => response.value?.profiles.length ?? 0);
const profileCountText = computed(() => {
  if (hasSearchQuery.value) {
    return `${filteredProfiles.value.length} / ${totalProfileCount.value} 个匹配`;
  }
  const hiddenCount = groupedProfiles.value.hidden.length;
  return hiddenCount
    ? `${groupedProfiles.value.visible.length} 个常用 / ${hiddenCount} 个隐藏`
    : `${groupedProfiles.value.visible.length} 个常用`;
});
const edgeStatusText = computed(() => {
  if (!response.value) return "正在检测 Edge";
  return response.value.edgeFound ? "已找到 Edge" : "未找到 Edge";
});

async function loadProfiles() {
  const seq = ++requestSeq;
  loading.value = true;
  errorMessage.value = "";
  try {
    const result = (await invokeToolByChannel(
      "tool:browser-profiles:list",
      {},
    )) as BrowserProfilesListResponse;
    if (seq !== requestSeq) return;
    response.value = result;
  } catch (err) {
    if (seq !== requestSeq) return;
    errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (seq === requestSeq) loading.value = false;
  }
}

async function launchProfile(profile: BrowserProfileItem) {
  const key = profileKey(profile);
  launchingKey.value = key;
  try {
    await invokeToolByChannel("tool:browser-profiles:launch", {
      browser: "edge",
      profileDir: profile.profileDir,
    });
    ElMessage.success(`已打开 Edge：${getBrowserProfileDisplayName(profile)}`);
    await loadProfiles();
  } catch (err) {
    ElMessage.error(`启动失败：${messageOf(err)}`);
  } finally {
    if (launchingKey.value === key) launchingKey.value = "";
  }
}

async function editAlias(profile: BrowserProfileItem) {
  try {
    const { value } = await ElMessageBox.prompt("输入浏览器身份别名", "编辑别名", {
      inputValue: profile.alias,
      confirmButtonText: "保存",
      cancelButtonText: "取消",
      inputPlaceholder: getBrowserProfileDisplayName({
        ...profile,
        alias: "",
      }),
    });
    await invokeToolByChannel("tool:browser-profiles:save-alias", {
      browser: "edge",
      profileDir: profile.profileDir,
      alias: String(value ?? "").trim(),
    });
    ElMessage.success("别名已保存");
    await loadProfiles();
  } catch (err) {
    if (isCancel(err)) return;
    ElMessage.error(`保存失败：${messageOf(err)}`);
  }
}

async function setHidden(profile: BrowserProfileItem, hidden: boolean) {
  try {
    await invokeToolByChannel("tool:browser-profiles:set-hidden", {
      browser: "edge",
      profileDir: profile.profileDir,
      hidden,
    });
    ElMessage.success(hidden ? "已隐藏" : "已恢复");
    await loadProfiles();
  } catch (err) {
    ElMessage.error(`操作失败：${messageOf(err)}`);
  }
}

async function chooseEdgePath() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Microsoft Edge", extensions: ["exe"] }],
    });
    const edgePath = Array.isArray(selected) ? selected[0] : selected;
    if (!edgePath) return;
    await invokeToolByChannel("tool:browser-profiles:set-edge-path", {
      edgePath,
    });
    ElMessage.success("Edge 路径已保存");
    await loadProfiles();
  } catch (err) {
    if (isCancel(err)) return;
    ElMessage.error(`保存 Edge 路径失败：${messageOf(err)}`);
  }
}

function toggleHiddenProfiles() {
  if (hasSearchQuery.value) return;
  hiddenExpanded.value = !hiddenExpanded.value;
}

function profileKey(profile: BrowserProfileItem): string {
  return `${profile.browser}:${profile.profileDir}`;
}

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function isCancel(err: unknown): boolean {
  return String(err).toLowerCase().includes("cancel");
}

onMounted(() => {
  loadProfiles();
});
</script>

<style scoped>
.browser-profiles-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  background: var(--lc-surface-0);
  color: var(--lc-text);
}

.browser-profiles-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--lc-border-subtle);
  background: var(--lc-surface-0);
}

.browser-profiles-title {
  display: flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
}

.browser-profiles-title h2 {
  margin: 0;
  font-size: 18px;
  line-height: 1.2;
  font-weight: 650;
  color: var(--lc-text);
}

.browser-profiles-status {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 24px;
  padding: 0 8px;
  border-radius: var(--lc-radius-sm);
  background: color-mix(in srgb, var(--lc-warning, #f59e0b) 10%, transparent);
  color: var(--lc-text-secondary);
  font-size: 12px;
  white-space: nowrap;
}

.browser-profiles-status.is-found {
  background: color-mix(in srgb, var(--lc-success, #22c55e) 10%, transparent);
  color: var(--lc-success, #22c55e);
}

.browser-profiles-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.browser-profiles-count {
  color: var(--lc-text-secondary);
  font-size: 13px;
  white-space: nowrap;
}

.browser-profiles-alerts {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 20px;
  border-bottom: 1px solid var(--lc-border-subtle);
  background: var(--lc-surface-1);
}

.browser-profiles-alert,
.browser-profiles-probed {
  padding: 10px 12px;
  border: 1px solid var(--lc-border-subtle);
  border-radius: var(--lc-radius-sm);
  background: var(--lc-surface-0);
  color: var(--lc-text-secondary);
  font-size: 13px;
}

.browser-profiles-alert.is-error {
  border-color: color-mix(in srgb, var(--lc-danger, #ef4444) 35%, transparent);
  color: var(--lc-danger, #ef4444);
}

.browser-profiles-probed-title {
  margin-bottom: 6px;
  color: var(--lc-text);
  font-weight: 500;
}

.browser-profiles-path {
  font-family: var(--lc-font-mono);
  font-size: 12px;
  line-height: 1.6;
  word-break: break-all;
}

.browser-profiles-filter {
  padding: 12px 20px;
  border-bottom: 1px solid var(--lc-border-subtle);
  background: var(--lc-surface-0);
}

.browser-profiles-filter :deep(.el-input) {
  max-width: 520px;
}

.browser-profiles-content {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 16px 20px 24px;
}

.browser-profiles-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 220px;
  color: var(--lc-text-secondary);
  font-size: 14px;
}

.browser-profiles-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.browser-profile-row {
  display: grid;
  grid-template-columns: minmax(180px, 1fr) 190px auto;
  align-items: center;
  gap: 14px;
  min-height: 68px;
  padding: 12px 14px;
  border: 1px solid var(--lc-border-subtle);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-1);
}

.browser-profile-row.is-hidden {
  opacity: 0.78;
}

.browser-profile-main {
  min-width: 0;
}

.browser-profile-name {
  overflow: hidden;
  color: var(--lc-text);
  font-size: 15px;
  font-weight: 600;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.browser-profile-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 10px;
  margin-top: 5px;
  color: var(--lc-text-secondary);
  font-size: 12px;
  line-height: 1.4;
}

.browser-profile-stats {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: var(--lc-text-secondary);
  font-size: 12px;
  text-align: right;
  white-space: nowrap;
}

.browser-profile-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  min-width: 0;
}

.browser-profiles-hidden {
  margin-top: 18px;
}

.browser-profiles-hidden-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  height: 36px;
  margin-bottom: 8px;
  padding: 0 12px;
  border: 1px solid var(--lc-border-subtle);
  border-radius: var(--lc-radius-sm);
  background: var(--lc-surface-1);
  color: var(--lc-text-secondary);
  cursor: pointer;
  font-size: 13px;
}

.browser-profiles-hidden-toggle:hover {
  color: var(--lc-text);
  border-color: var(--lc-border);
}

.browser-profiles-hidden-toggle.is-searching {
  cursor: default;
}

.browser-profiles-hidden-toggle.is-searching:hover {
  color: var(--lc-text-secondary);
  border-color: var(--lc-border-subtle);
}

@media (max-width: 900px) {
  .browser-profiles-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .browser-profiles-actions {
    width: 100%;
    flex-wrap: wrap;
  }

  .browser-profiles-filter :deep(.el-input) {
    max-width: none;
  }

  .browser-profile-row {
    grid-template-columns: 1fr;
    align-items: stretch;
  }

  .browser-profile-stats {
    flex-direction: row;
    text-align: left;
  }

  .browser-profile-actions {
    justify-content: flex-start;
    flex-wrap: wrap;
  }
}
</style>
