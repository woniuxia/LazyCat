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

      <div v-if="totalProfileCount > 0" class="browser-profiles-search">
        <el-input
          v-model="searchQuery"
          clearable
          :prefix-icon="Search"
          placeholder="搜索别名、Edge 名、目录或拼音首字母"
        />
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

    <main class="browser-profiles-content" v-loading="loading">
      <div v-if="!loading && sortedProfiles.length === 0" class="browser-profiles-empty">
        未发现 Edge Profile
      </div>
      <div v-else-if="!loading && filteredProfiles.length === 0" class="browser-profiles-empty">
        没有匹配的浏览器身份
      </div>

      <template v-else>
        <section class="browser-profiles-grid">
          <div
            v-for="profile in groupedProfiles.visible"
            :key="profileKey(profile)"
            class="browser-profile-card"
            :class="{
              'is-launching': launchingKey === profileKey(profile),
              'is-disabled': edgeMissing,
            }"
            role="button"
            :tabindex="edgeMissing ? -1 : 0"
            :aria-disabled="edgeMissing || undefined"
            :aria-label="`启动 ${getBrowserProfileDisplayName(profile)}`"
            :title="profileTooltip(profile)"
            @click="launchProfile(profile)"
            @keydown.enter="onCardKeydown(profile, $event)"
            @keydown.space="onCardKeydown(profile, $event)"
          >
            <div
              class="browser-profile-badge"
              :class="`is-color-${getBrowserProfileBadgeColorIndex(profile)}`"
            >
              <span class="browser-profile-badge-initial">
                {{ getBrowserProfileBadgeInitial(profile) }}
              </span>
              <el-icon
                v-if="launchingKey === profileKey(profile)"
                class="browser-profile-badge-loading is-loading"
              >
                <Loading />
              </el-icon>
              <el-icon v-else class="browser-profile-badge-play">
                <VideoPlay />
              </el-icon>
            </div>
            <div class="browser-profile-info">
              <div class="browser-profile-title-row">
                <span class="browser-profile-name">
                  {{ getBrowserProfileDisplayName(profile) }}
                </span>
                <div class="browser-profile-card-actions">
                  <button
                    type="button"
                    class="browser-profile-icon-btn"
                    title="编辑别名"
                    aria-label="编辑别名"
                    @click.stop="editAlias(profile)"
                  >
                    <el-icon><EditPen /></el-icon>
                  </button>
                  <button
                    type="button"
                    class="browser-profile-icon-btn"
                    title="隐藏"
                    aria-label="隐藏"
                    @click.stop="setHidden(profile, true)"
                  >
                    <el-icon><Hide /></el-icon>
                  </button>
                </div>
              </div>
              <div class="browser-profile-meta">{{ profileMetaText(profile) }}</div>
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

          <div v-if="hiddenListVisible" class="browser-profiles-grid">
            <div
              v-for="profile in groupedProfiles.hidden"
              :key="profileKey(profile)"
              class="browser-profile-card is-hidden"
              :title="profileTooltip(profile)"
            >
              <div
                class="browser-profile-badge"
                :class="`is-color-${getBrowserProfileBadgeColorIndex(profile)}`"
              >
                <span class="browser-profile-badge-initial">
                  {{ getBrowserProfileBadgeInitial(profile) }}
                </span>
              </div>
              <div class="browser-profile-info">
                <div class="browser-profile-title-row">
                  <span class="browser-profile-name">
                    {{ getBrowserProfileDisplayName(profile) }}
                  </span>
                  <div class="browser-profile-card-actions">
                    <button
                      type="button"
                      class="browser-profile-icon-btn"
                      title="恢复显示"
                      aria-label="恢复显示"
                      @click.stop="setHidden(profile, false)"
                    >
                      <el-icon><View /></el-icon>
                    </button>
                    <button
                      type="button"
                      class="browser-profile-icon-btn"
                      title="编辑别名"
                      aria-label="编辑别名"
                      @click.stop="editAlias(profile)"
                    >
                      <el-icon><EditPen /></el-icon>
                    </button>
                  </div>
                </div>
                <div class="browser-profile-meta">{{ profileMetaText(profile) }}</div>
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
  Loading,
  Refresh,
  Search,
  VideoPlay,
  View,
  WarningFilled,
} from "@element-plus/icons-vue";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";
import {
  notifyBrowserProfilesChanged,
  type BrowserProfilesChangedReason,
} from "../spotlight/browser-profiles-events";
import type {
  BrowserProfileItem,
  BrowserProfilesListResponse,
} from "../types/browser-profiles";
import {
  buildBrowserProfileMetaSegments,
  filterBrowserProfiles,
  formatBrowserProfileLastLaunchedAt,
  getBrowserProfileBadgeColorIndex,
  getBrowserProfileBadgeInitial,
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
const metaNow = ref(new Date());
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
const edgeMissing = computed(() => response.value?.edgeFound === false);
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

function profileMetaText(profile: BrowserProfileItem): string {
  return buildBrowserProfileMetaSegments(profile, metaNow.value).join(" · ");
}

function profileTooltip(profile: BrowserProfileItem): string {
  const displayName = getBrowserProfileDisplayName(profile);
  const lines = [displayName];
  const edgeName = profile.edgeDisplayName.trim();
  if (edgeName && edgeName !== displayName) lines.push(`Edge 名称：${edgeName}`);
  lines.push(`目录：${profile.profileDir}`);
  lines.push(`启动次数：${profile.launchCount}`);
  lines.push(`最近启动：${formatBrowserProfileLastLaunchedAt(profile.lastLaunchedAt)}`);
  return lines.join("\n");
}

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
    metaNow.value = new Date();
  } catch (err) {
    if (seq !== requestSeq) return;
    errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (seq === requestSeq) loading.value = false;
  }
}

function notifyProfilesChanged(reason: BrowserProfilesChangedReason) {
  void notifyBrowserProfilesChanged(reason).catch(() => undefined);
}

function onCardKeydown(profile: BrowserProfileItem, event: KeyboardEvent) {
  if (event.target !== event.currentTarget) return;
  event.preventDefault();
  launchProfile(profile);
}

async function launchProfile(profile: BrowserProfileItem) {
  if (edgeMissing.value) {
    ElMessage.warning("未找到 Edge，请先选择 msedge.exe");
    return;
  }
  const key = profileKey(profile);
  if (launchingKey.value === key) return;
  launchingKey.value = key;
  try {
    await invokeToolByChannel("tool:browser-profiles:launch", {
      browser: "edge",
      profileDir: profile.profileDir,
    });
    notifyProfilesChanged("launch");
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
    notifyProfilesChanged("alias");
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
    notifyProfilesChanged("hidden");
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
    notifyProfilesChanged("edge-path");
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
  gap: 12px;
  padding: 12px 20px;
  border-bottom: 1px solid var(--lc-border-subtle);
  background: var(--lc-surface-0);
}

.browser-profiles-title {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex-shrink: 0;
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

.browser-profiles-search {
  flex: 1;
  min-width: 180px;
  max-width: 400px;
}

.browser-profiles-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  margin-left: auto;
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

.browser-profiles-content {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 14px 20px 24px;
}

.browser-profiles-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 220px;
  color: var(--lc-text-secondary);
  font-size: 14px;
}

.browser-profiles-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 10px;
}

.browser-profile-card {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  padding: 10px 12px;
  border: 1px solid var(--lc-border-subtle);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-1);
  cursor: pointer;
  user-select: none;
  transition: border-color 0.15s, background 0.15s, box-shadow 0.15s, transform 0.1s;
}

.browser-profile-card:hover {
  border-color: var(--lc-border-hover);
  background: var(--lc-accent-glow);
  box-shadow: var(--lc-shadow-sm);
}

.browser-profile-card:active {
  transform: scale(0.99);
}

.browser-profile-card:focus-visible {
  outline: 2px solid var(--lc-accent);
  outline-offset: 1px;
}

.browser-profile-card.is-disabled {
  cursor: not-allowed;
}

.browser-profile-card.is-disabled:hover {
  border-color: var(--lc-border-subtle);
  background: var(--lc-surface-1);
  box-shadow: none;
}

.browser-profile-card.is-disabled:active,
.browser-profile-card.is-hidden:active {
  transform: none;
}

.browser-profile-card.is-hidden {
  opacity: 0.72;
  cursor: default;
}

.browser-profile-card.is-hidden:hover {
  border-color: var(--lc-border);
  background: var(--lc-surface-1);
  box-shadow: none;
  opacity: 1;
}

.browser-profile-badge {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  border-radius: 50%;
  font-size: 15px;
  font-weight: 600;
}

.browser-profile-badge-initial {
  transition: opacity 0.15s;
}

.browser-profile-badge .el-icon {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  align-items: center;
  justify-content: center;
  font-size: 17px;
  opacity: 0;
  transition: opacity 0.15s;
}

.browser-profile-card:not(.is-disabled):not(.is-hidden):hover .browser-profile-badge-initial,
.browser-profile-card.is-launching .browser-profile-badge-initial {
  opacity: 0;
}

.browser-profile-card:not(.is-disabled):hover .browser-profile-badge-play,
.browser-profile-card.is-launching .browser-profile-badge-loading {
  opacity: 1;
}

.browser-profile-badge.is-color-0 { color: #0284c7; background: rgba(14, 165, 233, 0.14); }
.browser-profile-badge.is-color-1 { color: #7c3aed; background: rgba(139, 92, 246, 0.14); }
.browser-profile-badge.is-color-2 { color: #b45309; background: rgba(245, 158, 11, 0.16); }
.browser-profile-badge.is-color-3 { color: #047857; background: rgba(16, 185, 129, 0.14); }
.browser-profile-badge.is-color-4 { color: #dc2626; background: rgba(239, 68, 68, 0.12); }
.browser-profile-badge.is-color-5 { color: #db2777; background: rgba(236, 72, 153, 0.12); }
.browser-profile-badge.is-color-6 { color: #4f46e5; background: rgba(99, 102, 241, 0.14); }
.browser-profile-badge.is-color-7 { color: #0f766e; background: rgba(20, 184, 166, 0.14); }

.browser-profile-info {
  flex: 1;
  min-width: 0;
}

.browser-profile-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.browser-profile-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--lc-text);
  font-size: 13px;
  font-weight: 600;
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.browser-profile-card-actions {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.15s;
}

.browser-profile-card:hover .browser-profile-card-actions,
.browser-profile-card:focus-within .browser-profile-card-actions {
  opacity: 1;
  pointer-events: auto;
}

.browser-profile-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--lc-text-muted);
  cursor: pointer;
  font-size: 14px;
  transition: background 0.15s, color 0.15s;
}

.browser-profile-icon-btn:hover {
  background: var(--lc-accent-dim);
  color: var(--lc-text);
}

.browser-profile-icon-btn:focus-visible {
  outline: 2px solid var(--lc-accent);
  outline-offset: 1px;
}

.browser-profile-meta {
  margin-top: 2px;
  overflow: hidden;
  color: var(--lc-text-secondary);
  font-size: 12px;
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.browser-profiles-hidden {
  margin-top: 16px;
}

.browser-profiles-hidden-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  height: 32px;
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
    flex-wrap: wrap;
  }

  .browser-profiles-search {
    order: 3;
    flex-basis: 100%;
    max-width: none;
  }

  .browser-profiles-actions {
    flex-wrap: wrap;
  }
}
</style>
