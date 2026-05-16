<template>
  <div class="spotlight" @keydown="onKeydown">
    <div v-if="unlockState" class="spotlight-input-wrap">
      <SpotlightVaultUnlockInput
        ref="unlockRef"
        :entry-title="unlockState.entryTitle"
        @submit="onUnlockSubmit"
        @cancel="cancelUnlock"
      />
    </div>
    <div v-else class="spotlight-input-wrap">
      <input
        ref="inputRef"
        v-model="query"
        class="spotlight-input"
        :placeholder="placeholder"
        spellcheck="false"
        autocomplete="off"
      />
      <span v-if="scope" class="spotlight-scope-chip">{{ scopeLabel }}</span>
    </div>

    <div class="spotlight-results">
      <div v-if="loading && results.length === 0" class="spotlight-empty">加载中…</div>
      <div v-else-if="results.length === 0" class="spotlight-empty">
        {{ query.trim() ? "没有匹配的结果" : "输入关键词以搜索工具、凭据、Hosts、任务、项目" }}
      </div>
      <div
        v-for="(entry, idx) in results"
        :key="entry.item.providerId + ':' + entry.item.itemId"
        ref="rowRefs"
        class="spotlight-row"
        :class="{ 'is-active': idx === activeIndex }"
        @mouseenter="activeIndex = idx"
        @click="commitDefault(entry.item)"
      >
        <span class="spotlight-row-index">{{ idx + 1 }}</span>
        <span
          v-if="entry.item.badge"
          class="spotlight-badge"
          :class="`tone-${entry.item.badge.tone}`"
        >
          {{ entry.item.badge.short }}
        </span>
        <div class="spotlight-row-main">
          <span class="spotlight-row-name">{{ entry.item.title }}</span>
          <span v-if="entry.item.subtitle" class="spotlight-row-desc">
            {{ entry.item.subtitle }}
          </span>
        </div>
        <span
          v-if="entry.item.status"
          class="spotlight-status"
          :class="`tone-${entry.item.status.tone}`"
        >
          {{ entry.item.status.text }}
        </span>
      </div>
    </div>

    <div class="spotlight-footer">
      <span>{{ footerHint }}</span>
    </div>

    <SpotlightErrorBar
      :message="errorMessage"
      :can-retry="!!lastFailed"
      @retry="retryLast"
      @dismiss="errorMessage = null"
    />

    <SpotlightActionMenu
      :open="actionMenuOpen"
      :actions="actionMenuActions"
      :anchor-rect="actionMenuAnchor"
      @close="closeActionMenu"
      @select="onActionSelect"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import SpotlightActionMenu from "./SpotlightActionMenu.vue";
import SpotlightErrorBar from "./SpotlightErrorBar.vue";
import SpotlightVaultUnlockInput from "./SpotlightVaultUnlockInput.vue";

import { listProviders, searchItems } from "../spotlight/registry";
import "../spotlight/providers/tool";
import "../spotlight/providers/vault";
import "../spotlight/providers/hosts";
import "../spotlight/providers/todo";
import "../spotlight/providers/pm";

import { parseSpotlightQuery } from "../utils/spotlight-query";
import type {
  SpotlightAction,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightProviderId,
} from "../spotlight/types";

type ScopedItemsMap = Map<SpotlightProviderId, SpotlightItem[]>;

const RESULT_LIMIT = 12;
const SCOPE_LABEL: Record<SpotlightProviderId, string> = {
  tool: "工具",
  vault: "凭据",
  hosts: "Hosts",
  todo: "任务",
  pm: "项目",
};

const query = ref("");
const activeIndex = ref(0);
const loading = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);
const rowRefs = ref<HTMLElement[]>([]);
const unlockRef = ref<InstanceType<typeof SpotlightVaultUnlockInput> | null>(null);

const errorMessage = ref<string | null>(null);
const lastFailed = ref<null | (() => Promise<void>)>(null);

const actionMenuOpen = ref(false);
const actionMenuActions = ref<SpotlightAction[]>([]);
const actionMenuAnchor = ref<DOMRect | null>(null);
const actionMenuTargetItem = ref<SpotlightItem | null>(null);

let unlockResolver: ((value: string | null) => void) | null = null;
const unlockState = ref<{ entryTitle: string } | null>(null);

let unlistenReset: UnlistenFn | null = null;

const itemsByProvider = ref<ScopedItemsMap>(new Map());

const parsed = computed(() => parseSpotlightQuery(query.value));
const scope = computed(() => parsed.value.scope);
const scopeLabel = computed(() => (scope.value ? SCOPE_LABEL[scope.value] : ""));

const placeholder = computed(() =>
  scope.value
    ? `在 ${SCOPE_LABEL[scope.value]} 中搜索…`
    : "搜索工具 / 凭据 / Hosts / 任务 / 项目（v / h / t / p 限定）",
);

const footerHint = computed(() => {
  if (unlockState.value) return "Enter 确认 · Esc 取消";
  if (actionMenuOpen.value) return "↑↓ 选择 · Enter 执行 · Esc 收起";
  if (errorMessage.value) return "Ctrl+R 重试 · Esc 关闭";
  return "Enter 执行 · Tab 备选动作 · Esc 关闭";
});

const results = computed(() => {
  const text = parsed.value.query;
  if (!text.trim()) {
    // 空查询：按 provider 权重展示前若干工具（沿用工具高频列表）
    const tool = itemsByProvider.value.get("tool") ?? [];
    return tool.slice(0, RESULT_LIMIT).map((item) => ({ item, score: 0 }));
  }
  return searchItems(text, itemsByProvider.value, {
    scope: scope.value,
    limit: RESULT_LIMIT,
  });
});

watch(results, () => {
  if (activeIndex.value >= results.value.length) {
    activeIndex.value = results.value.length > 0 ? 0 : 0;
  }
});

async function prefetchAll() {
  loading.value = true;
  const map: ScopedItemsMap = new Map();
  const providers = listProviders();
  await Promise.allSettled(
    providers.map(async (provider) => {
      try {
        const items = await provider.prefetch();
        map.set(provider.id, items);
      } catch (err) {
        console.warn(`[Spotlight] provider ${provider.id} prefetch failed:`, err);
        map.set(provider.id, []);
      }
    }),
  );
  itemsByProvider.value = map;
  loading.value = false;
}

function buildContext(): SpotlightExecuteContext {
  return {
    query: parsed.value.query,
    requestMasterPassword: (entryTitle: string) =>
      new Promise<string | null>((resolve) => {
        unlockState.value = { entryTitle };
        unlockResolver = resolve;
        // 子组件 onMounted 内会自动 focus，这里不再重复
      }),
  };
}

function onUnlockSubmit(password: string) {
  const resolver = unlockResolver;
  unlockResolver = null;
  // unlockState 暂保持，由调用方决定是否清空（成功后清空，失败后保留）
  resolver?.(password);
}

function cancelUnlock() {
  const resolver = unlockResolver;
  unlockResolver = null;
  unlockState.value = null;
  resolver?.(null);
  nextTick(() => inputRef.value?.focus());
}

async function applyResult(result: SpotlightExecuteResult) {
  if (result.errorMessage) {
    errorMessage.value = result.errorMessage;
    if (unlockState.value) {
      unlockRef.value?.reportError?.(result.errorMessage);
    }
    return;
  }
  errorMessage.value = null;
  lastFailed.value = null;
  if (unlockState.value) unlockState.value = null;
  if (result.toast) {
    // 简化：使用 console；完整 toast 接入由 ElMessage / Notification 提供，但 Spotlight 在独立窗口内
    console.info("[Spotlight]", result.toast.message);
  }
  if (result.closeSpotlight) {
    await closeWindow();
  }
}

async function runWithRunner(fn: () => Promise<SpotlightExecuteResult>) {
  lastFailed.value = fn;
  try {
    const result = await fn();
    await applyResult(result);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    errorMessage.value = msg;
    if (unlockState.value) unlockRef.value?.reportError?.(msg);
  }
}

async function commitDefault(item: SpotlightItem) {
  const provider = listProviders().find((p) => p.id === item.providerId);
  if (!provider) return;
  await runWithRunner(() => provider.defaultAction(item, buildContext()));
}

function openActionMenu(item: SpotlightItem) {
  const provider = listProviders().find((p) => p.id === item.providerId);
  if (!provider || !provider.buildActions) return;
  actionMenuActions.value = provider.buildActions(item);
  actionMenuTargetItem.value = item;
  const row = rowRefs.value[activeIndex.value];
  actionMenuAnchor.value = row?.getBoundingClientRect() ?? null;
  actionMenuOpen.value = true;
}

function closeActionMenu() {
  actionMenuOpen.value = false;
  actionMenuTargetItem.value = null;
  nextTick(() => inputRef.value?.focus());
}

async function onActionSelect(action: SpotlightAction) {
  const item = actionMenuTargetItem.value;
  if (!item) return;
  const provider = listProviders().find((p) => p.id === item.providerId);
  if (!provider) return;
  actionMenuOpen.value = false;
  await runWithRunner(async () => {
    const ctx = buildContext();
    if (provider.executeAction) {
      return provider.executeAction(item, action.id, ctx);
    }
    return provider.defaultAction(item, ctx);
  });
}

async function retryLast() {
  if (!lastFailed.value) return;
  const fn = lastFailed.value;
  errorMessage.value = null;
  await runWithRunner(fn);
}

function onKeydown(e: KeyboardEvent) {
  if (unlockState.value) return; // 解锁条自管理键盘
  if (actionMenuOpen.value) return; // ActionMenu 自管理键盘

  if (e.key === "Escape") {
    e.preventDefault();
    if (errorMessage.value) {
      errorMessage.value = null;
      return;
    }
    void closeWindow();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (results.value.length === 0) return;
    activeIndex.value = (activeIndex.value + 1) % results.value.length;
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    if (results.value.length === 0) return;
    activeIndex.value =
      (activeIndex.value - 1 + results.value.length) % results.value.length;
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    const entry = results.value[activeIndex.value];
    if (entry) void commitDefault(entry.item);
    return;
  }
  if (e.key === "Tab") {
    e.preventDefault();
    const entry = results.value[activeIndex.value];
    if (entry) openActionMenu(entry.item);
    return;
  }
  if (e.ctrlKey && (e.key === "r" || e.key === "R")) {
    e.preventDefault();
    void retryLast();
    return;
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  if (e.key >= "1" && e.key <= "9") {
    const idx = parseInt(e.key, 10) - 1;
    const entry = results.value[idx];
    if (entry) {
      e.preventDefault();
      void commitDefault(entry.item);
    }
  }
}

async function closeWindow() {
  try {
    await invoke("spotlight_close");
  } catch {
    /* ignore */
  }
}

onMounted(async () => {
  await nextTick();
  inputRef.value?.focus();
  inputRef.value?.select();
  await prefetchAll();

  try {
    unlistenReset = await listen("spotlight-reset", () => {
      query.value = "";
      activeIndex.value = 0;
      errorMessage.value = null;
      lastFailed.value = null;
      unlockState.value = null;
      actionMenuOpen.value = false;
      void prefetchAll();
      nextTick(() => {
        inputRef.value?.focus();
        inputRef.value?.select();
      });
    });
  } catch {
    /* ignore */
  }
});

onBeforeUnmount(() => {
  unlistenReset?.();
});
</script>

<style scoped>
.spotlight {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  background: #ffffff;
  border-radius: 12px;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.18);
  overflow: hidden;
  font-family: inherit;
}

.spotlight-input-wrap {
  flex-shrink: 0;
  padding: 0 20px;
  min-height: 64px;
  display: flex;
  align-items: center;
  gap: 8px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
}

.spotlight-input {
  flex: 1;
  height: 64px;
  border: none;
  outline: none;
  font-size: 16px;
  color: #303133;
  background: transparent;
}

.spotlight-input::placeholder {
  color: #c0c4cc;
}

.spotlight-scope-chip {
  flex-shrink: 0;
  font-size: 11px;
  padding: 3px 10px;
  border-radius: 999px;
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
}

.spotlight-results {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.spotlight-empty {
  padding: 24px 20px;
  color: #909399;
  font-size: 13px;
  text-align: center;
}

.spotlight-row {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 56px;
  padding: 0 20px;
  cursor: pointer;
  transition: background-color 0.12s ease;
}

.spotlight-row.is-active {
  background: #f3f6fb;
}

.spotlight-row-index {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  background: #f0f2f5;
  color: #606266;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-variant-numeric: tabular-nums;
}

.spotlight-row.is-active .spotlight-row-index {
  background: #409eff;
  color: #fff;
}

.spotlight-badge {
  flex-shrink: 0;
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 6px;
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
  white-space: nowrap;
}

.spotlight-badge.tone-primary {
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
}

.spotlight-badge.tone-warn {
  background: rgba(245, 158, 11, 0.14);
  color: #b45309;
}

.spotlight-badge.tone-info {
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
}

.spotlight-badge.tone-success {
  background: rgba(34, 197, 94, 0.14);
  color: #15803d;
}

.spotlight-badge.tone-danger {
  background: rgba(245, 108, 108, 0.14);
  color: #c45656;
}

.spotlight-badge.tone-muted {
  background: rgba(144, 147, 153, 0.14);
  color: #606266;
}

.spotlight-row-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.spotlight-row-name {
  font-size: 14px;
  color: #303133;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-row-desc {
  font-size: 12px;
  color: #909399;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-status {
  flex-shrink: 0;
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 999px;
  white-space: nowrap;
}

.spotlight-status.tone-success {
  background: rgba(34, 197, 94, 0.14);
  color: #15803d;
}

.spotlight-status.tone-warn {
  background: rgba(245, 158, 11, 0.16);
  color: #b45309;
}

.spotlight-status.tone-danger {
  background: rgba(245, 108, 108, 0.14);
  color: #c45656;
}

.spotlight-status.tone-info {
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
}

.spotlight-status.tone-muted {
  background: rgba(144, 147, 153, 0.14);
  color: #606266;
}

.spotlight-status.tone-primary {
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
}

.spotlight-footer {
  flex-shrink: 0;
  padding: 6px 16px;
  font-size: 11px;
  color: #909399;
  background: #fafbfc;
  border-top: 1px solid rgba(0, 0, 0, 0.04);
}
</style>
