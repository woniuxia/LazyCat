<template>
  <div
    class="spotlight"
    role="dialog"
    aria-label="Spotlight"
    :aria-busy="isLoadingView || executing"
  >
    <SpotlightVaultUnlockInput
      v-if="unlockState"
      ref="unlockRef"
      :entry-title="unlockState.entryTitle"
      @unlocked="onUnlocked"
      @cancel="cancelUnlock"
    />
    <div v-else class="spotlight-input-wrap">
      <input
        ref="inputRef"
        v-model="query"
        class="spotlight-input"
        spellcheck="false"
        autocomplete="off"
        :readonly="executing"
        role="combobox"
        aria-label="搜索工具、动作和数据"
        aria-autocomplete="list"
        aria-controls="spotlight-results"
        :aria-activedescendant="activeResultId || undefined"
        :aria-expanded="results.length > 0"
      />
      <span
        v-if="isLoadingView"
        class="spotlight-inline-status"
        role="status"
        aria-label="正在更新结果"
      >
        <Loading aria-hidden="true" />
      </span>
      <span v-if="scope" class="spotlight-scope-chip">{{ scopeLabel }}</span>
    </div>

    <div
      id="spotlight-results"
      ref="resultsRef"
      class="spotlight-results"
      role="listbox"
      aria-label="Spotlight 搜索结果"
      :aria-busy="isLoadingView"
    >
      <div v-if="isLoadingView && results.length === 0" class="spotlight-empty" role="status">
        加载中…
      </div>
      <div v-else-if="results.length === 0" class="spotlight-empty" role="status">
        {{
          query.trim() ? "没有匹配的结果" : "输入关键词以搜索工具、动作、凭据、Hosts、任务、项目"
        }}
      </div>
      <div
        v-for="(entry, idx) in results"
        :key="entry.item.providerId + ':' + entry.item.itemId"
        ref="rowRefs"
        :id="resultId(idx)"
        class="spotlight-row"
        :class="{ 'is-active': idx === activeIndex, 'is-disabled': executing }"
        role="option"
        :aria-selected="idx === activeIndex"
        :aria-disabled="executing"
        @pointermove="activeIndex = idx"
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
      @dismiss="dismissError"
    />

    <SpotlightSuccessBar :message="successMessage" />

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
import { Loading } from "@element-plus/icons-vue";

import SpotlightActionMenu from "./SpotlightActionMenu.vue";
import SpotlightErrorBar from "./SpotlightErrorBar.vue";
import SpotlightSuccessBar from "./SpotlightSuccessBar.vue";
import SpotlightVaultUnlockInput from "./SpotlightVaultUnlockInput.vue";

import { APP_EVENTS } from "../bridge/events";
import { invokeToolByChannel } from "../bridge/tauri";
import { listProviders, getDescriptor, searchItems } from "../spotlight/registry";
import { rankEmptyItems, usageRefKey } from "../spotlight/ranking";
import "../spotlight/providers/tool";
import "../spotlight/providers/vault";
import "../spotlight/providers/hosts";
import "../spotlight/providers/todo";
import "../spotlight/providers/pm";
import "../spotlight/providers/data-dictionary";
import { browserProfilesProvider } from "../spotlight/providers/browser-profiles";
import "../spotlight/providers/suggestion";
import "../spotlight/providers/launcher";
import "../spotlight/providers/action-center";
import { listenBrowserProfilesChanged } from "../spotlight/browser-profiles-events";
import {
  BROWSER_PROFILES_PROVIDER_ID,
  beginBrowserProfilesLocalRefresh,
  canWriteBrowserProfiles,
  captureBrowserProfilesPrefetchVersion,
  createBrowserProfilesRefreshGuard,
  replaceBrowserProfilesItems,
} from "../spotlight/browser-profiles-refresh";
import * as configStore from "../spotlight/config-store";
import {
  createQueryTimeResultGuard,
  mergeSpotlightProviderItems,
  shouldRunQueryProvider,
} from "../spotlight/search";

import {
  parseSpotlightQuery,
  parseQuickCommand,
  parseKeywordCommand,
} from "../utils/spotlight-query";
import { nextSpotlightActiveIndex } from "../utils/spotlight-active-index";
import { nextSpotlightScrollTop } from "../utils/spotlight-scroll";
import { calculateExpression, getCalcPreview } from "../utils/calc";
import { initSettings, getSetting } from "../composables/useSettings";
import {
  createClipboardSuggestionRefreshCoordinator,
  mergeClipboardSuggestionItems,
} from "../spotlight/clipboard-suggestions";
import { createTodoDraft } from "../spotlight/providers/todo";
import {
  resolveKeywordInvocation,
  executeKeywordItem,
  executeKeywordItemAction,
  buildKeywordItemActions,
  isKeywordItem,
} from "../spotlight/keyword-resolver";
import type {
  KeywordCommandInvocation,
  SpotlightAction,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightProviderId,
  SpotlightView,
} from "../spotlight/types";
import type { UsageRef, UsageSummary } from "../types/usage";

type ScopedItemsMap = Map<SpotlightProviderId, SpotlightItem[]>;
const USAGE_SUMMARY_BATCH_SIZE = 256;

const RESULT_LIMIT = 9;

const query = ref("");
const activeIndex = ref(0);
const loading = ref(false);
const executing = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);
const resultsRef = ref<HTMLElement | null>(null);
const rowRefs = ref<HTMLElement[]>([]);
const unlockRef = ref<InstanceType<typeof SpotlightVaultUnlockInput> | null>(null);

const errorMessage = ref<string | null>(null);
const lastFailed = ref<null | (() => Promise<void>)>(null);
const successMessage = ref<string | null>(null);
let successTimeoutId: number | null = null;
// 当前 success bar 是否在等待自动关窗(由 timeout 触发 closeWindow)
let pendingClose = false;

const actionMenuOpen = ref(false);
const actionMenuActions = ref<SpotlightAction[]>([]);
const actionMenuAnchor = ref<DOMRect | null>(null);
const actionMenuTargetItem = ref<SpotlightItem | null>(null);

let unlockResolver: ((value: boolean) => void) | null = null;
const unlockState = ref<{ entryTitle: string } | null>(null);

let unlistenReset: UnlistenFn | null = null;
let unlistenBrowserProfilesChanged: UnlistenFn | null = null;
let browserProfilesListenerDisposed = false;
let unsubConfig: (() => void) | null = null;

const itemsByProvider = ref<ScopedItemsMap>(new Map());
const queryItemsByProvider = ref<ScopedItemsMap>(new Map());
const queryLoading = ref(false);
const queryGuard = createQueryTimeResultGuard();
const view = ref<SpotlightView | null>(null);
const browserProfilesRefreshGuard = createBrowserProfilesRefreshGuard();
const usageSummaries = ref<Map<string, UsageSummary>>(new Map());
let usageRequestSeq = 0;
let lastUsageSignature = "";

const clipboardSuggestionItems = ref<SpotlightItem[]>([]);
const clipboardSuggestionRefresh = createClipboardSuggestionRefreshCoordinator((items) => {
  clipboardSuggestionItems.value = items;
});

async function refreshClipboardSuggestions() {
  await clipboardSuggestionRefresh.refresh(async () => {
    try {
      await initSettings();
    } catch {
      /* ignore */
    }
    if (getSetting("clipboard_detection") === "false") return null;
    return navigator.clipboard.readText();
  });
}

const parsed = computed(() => parseSpotlightQuery(query.value, view.value?.aliasMap));
const keywordInvocation = computed<KeywordCommandInvocation | null>(() =>
  parseKeywordCommand(query.value, view.value?.keywordIndex),
);
const quickCommand = computed(() => {
  if (keywordInvocation.value) return null;
  return parseQuickCommand(query.value, view.value?.enabledQuickCommands);
});
const scope = computed(() => {
  if (keywordInvocation.value || quickCommand.value) return null;
  return parsed.value.scope;
});
const scopeLabel = computed(() => {
  if (!scope.value) return "";
  const v = view.value;
  if (v) return v.providers.find((p) => p.id === scope.value)?.name ?? "";
  return getDescriptor(scope.value)?.name ?? "";
});

const enabledProviderIds = computed(() => {
  const v = view.value;
  if (!v) return null;
  return new Set(v.providers.filter((p) => p.enabled).map((p) => p.id));
});

const searchableItemsByProvider = computed(() => {
  const merged = mergeSpotlightProviderItems(itemsByProvider.value, queryItemsByProvider.value);
  const next = new Map(merged);
  next.set(
    "suggestion",
    mergeClipboardSuggestionItems(merged.get("suggestion") ?? [], clipboardSuggestionItems.value),
  );
  return next;
});

async function refreshUsageSummaries(itemsByProvider: ScopedItemsMap) {
  const refs = new Map<string, UsageRef>();
  for (const items of itemsByProvider.values()) {
    for (const item of items) {
      const usageRef = item.ranking?.usageRef;
      if (usageRef) refs.set(usageRefKey(usageRef), usageRef);
    }
  }
  const signature = [...refs.keys()].sort().join("\n");
  if (signature === lastUsageSignature) return;
  lastUsageSignature = signature;
  const requestSeq = ++usageRequestSeq;
  if (refs.size === 0) {
    usageSummaries.value = new Map();
    return;
  }
  try {
    const values = [...refs.values()];
    const batches: UsageRef[][] = [];
    for (let index = 0; index < values.length; index += USAGE_SUMMARY_BATCH_SIZE) {
      batches.push(values.slice(index, index + USAGE_SUMMARY_BATCH_SIZE));
    }
    const responses = await Promise.all(
      batches.map((batch) => invokeToolByChannel("tool:usage:summaries", { refs: batch })) as Array<
        Promise<{ items: Array<UsageRef & { summary: UsageSummary }> }>
      >,
    );
    if (requestSeq !== usageRequestSeq) return;
    usageSummaries.value = new Map(
      responses
        .flatMap((response) => response.items)
        .map((item) => [usageRefKey(item), item.summary]),
    );
  } catch (error) {
    if (requestSeq === usageRequestSeq) {
      console.warn("[Spotlight] load usage summaries failed:", error);
      usageSummaries.value = new Map();
    }
  }
}

watch(
  searchableItemsByProvider,
  (items) => {
    void refreshUsageSummaries(items);
  },
  { immediate: true },
);

const isLoadingView = computed(() => loading.value || keywordLoading.value || queryLoading.value);

// ── keyword 命令异步结果缓存 ─────────────────────────────────────────
//
// keyword 模式下结果可能依赖 IPC(local-ip / hash / vault-tag / snippet-tag)。
// 使用 nonce 防止过期请求覆盖最新结果。同一 query 命中同一 keyword 时缓存复用,
// 避免 ;uuid 这种"重新生成"的展示在每次渲染都变化。

const keywordItems = ref<SpotlightItem[]>([]);
const keywordLoading = ref(false);
const keywordError = ref<string | null>(null);
let keywordRequestNonce = 0;
let lastKeywordSignature = "";

function buildKeywordSignature(inv: KeywordCommandInvocation | null): string {
  if (!inv) return "";
  return `${inv.command.id}|${inv.args}`;
}

async function refreshKeywordItems(inv: KeywordCommandInvocation | null) {
  const signature = buildKeywordSignature(inv);
  if (!inv) {
    keywordItems.value = [];
    keywordLoading.value = false;
    keywordError.value = null;
    lastKeywordSignature = "";
    return;
  }
  if (signature === lastKeywordSignature) return;
  lastKeywordSignature = signature;
  const nonce = ++keywordRequestNonce;
  keywordLoading.value = true;
  keywordError.value = null;
  try {
    const items = await resolveKeywordInvocation(inv);
    if (nonce !== keywordRequestNonce) return;
    keywordItems.value = items;
  } catch (err) {
    if (nonce !== keywordRequestNonce) return;
    keywordError.value = err instanceof Error ? err.message : String(err);
    keywordItems.value = [];
  } finally {
    if (nonce === keywordRequestNonce) {
      keywordLoading.value = false;
    }
  }
}

watch(
  keywordInvocation,
  (next) => {
    void refreshKeywordItems(next);
  },
  { immediate: false },
);

const footerHint = computed(() => {
  if (unlockState.value) return "输入主密码 · 正确即复制 · Esc 取消";
  if (actionMenuOpen.value) return "↑↓ 选择 · Enter 执行 · Esc 收起";
  if (executing.value) return "执行中… · Esc 隐藏";
  if (errorMessage.value) return "Ctrl+R 重试 · Esc 关闭";
  return "Enter 执行 · Tab 备选动作 · Alt+1-9 直选 · Esc 关闭";
});

const results = computed(() => {
  if (keywordInvocation.value) {
    // keyword 命令模式:直接展示 resolver 返回的 items,跳过常规检索流
    const items = keywordItems.value;
    if (items.length === 0) {
      if (keywordLoading.value) return [];
      const hintItem: SpotlightItem = {
        providerId: "__keyword__",
        itemId: `kw-empty:${keywordInvocation.value.command.id}`,
        title: keywordError.value
          ? `加载失败:${keywordError.value}`
          : `;${keywordInvocation.value.command.keyword}`,
        subtitle: keywordError.value ? "Esc 关闭或重试" : "没有可用结果",
        badge: { short: "提示", tone: "muted" },
        searchFields: [],
        payload: { __keyword: true, keywordItemKind: "hint" },
      };
      return [{ item: hintItem, score: 0 }];
    }
    return items.map((item) => ({ item, score: 0 }));
  }
  if (quickCommand.value?.kind === "todo-create") {
    const text = quickCommand.value.text;
    const item: SpotlightItem = {
      providerId: "todo",
      itemId: text ? `todo-create:${text}` : "todo-create:empty",
      title: text ? `+ 新建任务：${text}` : "+ 新建任务…",
      subtitle: text ? "Enter 创建" : "输入要新建的任务标题",
      badge: { short: "新建", tone: "success" },
      searchFields: [],
      payload: { quickCommand: "todo-create", text },
    };
    return [{ item, score: 0 }];
  }
  if (quickCommand.value?.kind === "calc") {
    const text = quickCommand.value.text;
    if (!text) {
      const item: SpotlightItem = {
        providerId: "tool",
        itemId: "calc:empty",
        title: "计算器",
        subtitle: "输入表达式，支持 + - * / ( ) %、×÷ 与中英文标点",
        badge: { short: "算", tone: "info" },
        searchFields: [],
        payload: { quickCommand: "calc", text: "" },
      };
      return [{ item, score: 0 }];
    }
    try {
      const result = calculateExpression(text);
      const item: SpotlightItem = {
        providerId: "tool",
        itemId: `calc:${text}`,
        title: `${text} = ${result.displayValue}`,
        subtitle: "Enter 复制结果到剪贴板",
        badge: { short: "算", tone: "info" },
        searchFields: [],
        payload: {
          quickCommand: "calc",
          text,
          raw: result.rawValue,
          display: result.displayValue,
        },
      };
      return [{ item, score: 0 }];
    } catch (err) {
      const preview = getCalcPreview(text);
      if (preview) {
        const item: SpotlightItem = {
          providerId: "tool",
          itemId: `calc:${text}:preview`,
          title: `${text} ≈ ${preview}`,
          subtitle: "公式未完成,继续输入或按 Enter 计算",
          badge: { short: "算", tone: "muted" },
          searchFields: [],
          payload: { quickCommand: "calc", text },
        };
        return [{ item, score: 0 }];
      }
      const msg = err instanceof Error ? err.message : String(err);
      const item: SpotlightItem = {
        providerId: "tool",
        itemId: `calc:${text}:error`,
        title: text,
        subtitle: msg,
        badge: { short: "算", tone: "danger" },
        searchFields: [],
        payload: { quickCommand: "calc", text },
      };
      return [{ item, score: 0 }];
    }
  }
  const text = parsed.value.query;
  if (!text.trim()) {
    const providers =
      view.value?.providers.filter((provider) => provider.enabled) ?? listProviders();
    return rankEmptyItems(
      searchableItemsByProvider.value,
      providers,
      usageSummaries.value,
      RESULT_LIMIT,
    );
  }
  return searchItems(text, searchableItemsByProvider.value, {
    scope: scope.value,
    limit: RESULT_LIMIT,
    enabledIds: enabledProviderIds.value ?? undefined,
    usageSummaries: usageSummaries.value,
  });
});

function resultId(index: number): string {
  return `spotlight-result-${index}`;
}

const activeResultId = computed(() => {
  return results.value[activeIndex.value] ? resultId(activeIndex.value) : undefined;
});

watch(results, () => {
  activeIndex.value = nextSpotlightActiveIndex({
    currentIndex: activeIndex.value,
    resultCount: results.value.length,
    queryChanged: false,
  });
});

watch(
  [() => parsed.value.query, scope, view, keywordInvocation, quickCommand],
  () => {
    void refreshQueryProviders();
  },
  { immediate: false },
);

// 用户继续输入新查询时,清除遗留的错误条/成功条与失败重试,避免红条/绿条粘连
// success bar 在等待自动关窗时被取消,意味着用户继续使用 spotlight,不再自动关
watch([query, scope], ([nextQuery, nextScope], [prevQuery, prevScope]) => {
  const changed = nextQuery !== prevQuery || nextScope !== prevScope;
  if (!changed) return;
  activeIndex.value = nextSpotlightActiveIndex({
    currentIndex: activeIndex.value,
    resultCount: results.value.length,
    queryChanged: true,
  });
  if (errorMessage.value || lastFailed.value) {
    errorMessage.value = null;
    lastFailed.value = null;
  }
  if (successMessage.value) {
    clearSuccessBar();
  }
});

async function prefetchAll() {
  // 保留 itemsByProvider 旧数据,渲染基于旧数据继续可用;只在首次加载时显示 loading
  const hadAnyData = itemsByProvider.value.size > 0;
  if (!hadAnyData) loading.value = true;
  const v = view.value;
  const providers = v ? v.providers.filter((p) => p.enabled) : listProviders();
  await Promise.allSettled(
    providers.map(async (provider) => {
      const browserProfilesPrefetchVersion =
        provider.id === BROWSER_PROFILES_PROVIDER_ID
          ? captureBrowserProfilesPrefetchVersion(browserProfilesRefreshGuard)
          : null;
      try {
        const items = await provider.prefetch();
        if (
          provider.id === BROWSER_PROFILES_PROVIDER_ID &&
          !canWriteBrowserProfiles(browserProfilesRefreshGuard, browserProfilesPrefetchVersion!)
        ) {
          return;
        }
        // 单 provider 完成后立即写回,渐进式更新而非等所有 provider
        const next = new Map(itemsByProvider.value);
        next.set(provider.id, items);
        itemsByProvider.value = next;
      } catch (err) {
        console.warn(`[Spotlight] provider ${provider.id} prefetch failed:`, err);
        if (
          provider.id === BROWSER_PROFILES_PROVIDER_ID &&
          !canWriteBrowserProfiles(browserProfilesRefreshGuard, browserProfilesPrefetchVersion!)
        ) {
          return;
        }
        // 失败时保留上一次该 provider 的数据,而不是覆盖为空
        if (!itemsByProvider.value.has(provider.id)) {
          const next = new Map(itemsByProvider.value);
          next.set(provider.id, []);
          itemsByProvider.value = next;
        }
      }
    }),
  );
  // 清理已禁用的 provider 残留数据,避免空查询合并时露出
  const enabledIds = new Set(providers.map((p) => p.id));
  if ([...itemsByProvider.value.keys()].some((id) => !enabledIds.has(id))) {
    const next = new Map<SpotlightProviderId, SpotlightItem[]>();
    for (const [id, items] of itemsByProvider.value) {
      if (enabledIds.has(id)) next.set(id, items);
    }
    itemsByProvider.value = next;
  }
  loading.value = false;
}

async function refreshBrowserProfilesProvider() {
  const version = beginBrowserProfilesLocalRefresh(browserProfilesRefreshGuard);
  try {
    const items = await browserProfilesProvider.prefetch();
    if (!canWriteBrowserProfiles(browserProfilesRefreshGuard, version)) return;
    itemsByProvider.value = replaceBrowserProfilesItems(itemsByProvider.value, items);
    lastUsageSignature = "";
    void refreshUsageSummaries(searchableItemsByProvider.value);
    activeIndex.value = nextSpotlightActiveIndex({
      currentIndex: activeIndex.value,
      resultCount: results.value.length,
      queryChanged: false,
    });
  } catch (err) {
    if (!canWriteBrowserProfiles(browserProfilesRefreshGuard, version)) return;
    console.warn("[Spotlight] refresh browser profiles failed:", err);
  }
}

async function refreshQueryProviders() {
  if (keywordInvocation.value || quickCommand.value) {
    queryItemsByProvider.value = new Map();
    queryLoading.value = false;
    return;
  }

  const text = parsed.value.query;
  const currentScope = scope.value;
  const requestSeq = queryGuard.next(text, currentScope);
  // 远程 provider 的结果只属于当前查询，避免新查询期间展示旧结果。
  queryItemsByProvider.value = new Map();
  const baseProviders = view.value
    ? view.value.providers.filter((provider) => provider.enabled)
    : listProviders();
  const providers = baseProviders.filter((provider) => {
    if (!provider.search) return false;
    if (currentScope && provider.id !== currentScope) return false;
    return shouldRunQueryProvider(text, currentScope, provider.id);
  });

  if (providers.length === 0) {
    queryItemsByProvider.value = new Map();
    queryLoading.value = false;
    return;
  }

  queryLoading.value = true;
  const next = new Map<SpotlightProviderId, SpotlightItem[]>();
  await Promise.allSettled(
    providers.map(async (provider) => {
      try {
        const items = await provider.search!(text, { scope: currentScope });
        next.set(provider.id, items);
      } catch (err) {
        console.warn(`[Spotlight] provider ${provider.id} query search failed:`, err);
        next.set(provider.id, []);
      }
    }),
  );

  if (!queryGuard.isCurrent(requestSeq, text, currentScope)) return;
  queryItemsByProvider.value = next;
  queryLoading.value = false;
}

function buildContext(): SpotlightExecuteContext {
  return {
    query: parsed.value.query,
    ensureVaultUnlocked: (entryTitle: string) =>
      new Promise<boolean>((resolve) => {
        unlockState.value = { entryTitle };
        unlockResolver = resolve;
        // 子组件 onMounted 内会自动 focus，这里不再重复
      }),
  };
}

function onUnlocked() {
  const resolver = unlockResolver;
  unlockResolver = null;
  unlockState.value = null;
  resolver?.(true);
}

function cancelUnlock() {
  const resolver = unlockResolver;
  unlockResolver = null;
  unlockState.value = null;
  resolver?.(false);
  nextTick(() => focusInput());
}

function focusInput(retries = 3) {
  if (unlockState.value) return;
  if (actionMenuOpen.value) return;
  const el = inputRef.value;
  if (!el) {
    if (retries > 0) setTimeout(() => focusInput(retries - 1), 16);
    return;
  }
  el.focus();
  if (document.activeElement !== el && retries > 0) {
    setTimeout(() => focusInput(retries - 1), 16);
    return;
  }
  try {
    el.select();
  } catch {
    /* ignore */
  }
}

function scrollActiveResultIntoView() {
  nextTick(() => {
    const container = resultsRef.value;
    const row = container?.querySelector<HTMLElement>(`#${resultId(activeIndex.value)}`);
    if (!container || !row) return;

    const containerRect = container.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    const viewportHeight = containerRect.height;
    if (viewportHeight <= 0 || rowRect.height <= 0) return;
    const nextScrollTop = nextSpotlightScrollTop({
      scrollTop: container.scrollTop,
      viewportHeight,
      itemTop: rowRect.top - containerRect.top + container.scrollTop,
      itemHeight: rowRect.height,
    });
    if (nextScrollTop !== container.scrollTop) {
      container.scrollTop = nextScrollTop;
    }
  });
}

function onWindowFocus() {
  focusInput();
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
  const shouldClose = !!result.closeSpotlight;
  if (result.toast?.message) {
    showSuccessBar(result.toast.message, shouldClose);
    return;
  }
  if (shouldClose) {
    await closeWindow();
  }
}

const SUCCESS_BAR_CLOSE_DELAY_MS = 800;
const SUCCESS_BAR_LINGER_MS = 1500;

function clearSuccessBar() {
  if (successTimeoutId != null) {
    window.clearTimeout(successTimeoutId);
    successTimeoutId = null;
  }
  pendingClose = false;
  successMessage.value = null;
}

function showSuccessBar(message: string, willClose: boolean) {
  if (successTimeoutId != null) window.clearTimeout(successTimeoutId);
  successMessage.value = message;
  pendingClose = willClose;
  const delay = willClose ? SUCCESS_BAR_CLOSE_DELAY_MS : SUCCESS_BAR_LINGER_MS;
  successTimeoutId = window.setTimeout(async () => {
    const close = pendingClose;
    successMessage.value = null;
    successTimeoutId = null;
    pendingClose = false;
    if (close) await closeWindow();
  }, delay);
}

async function runWithRunner(fn: () => Promise<SpotlightExecuteResult>) {
  if (executing.value) return;
  executing.value = true;
  lastFailed.value = fn;
  try {
    const result = await fn();
    await applyResult(result);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    errorMessage.value = msg;
    if (unlockState.value) unlockRef.value?.reportError?.(msg);
  } finally {
    executing.value = false;
  }
}

async function commitDefault(item: SpotlightItem) {
  if (executing.value) return;
  if (isKeywordItem(item)) {
    await runWithRunner(() => executeKeywordItem(item, buildContext()));
    return;
  }
  if (item.payload?.quickCommand === "todo-create") {
    const text = String(item.payload?.text ?? "").trim();
    if (!text) {
      errorMessage.value = "请输入要新建的任务标题";
      lastFailed.value = null;
      return;
    }
    await runWithRunner(() => createTodoDraft(text));
    return;
  }
  if (item.payload?.quickCommand === "calc") {
    const raw = String(item.payload?.raw ?? "");
    const display = String(item.payload?.display ?? "");
    if (!raw) {
      // 空、预览或错误状态：给出明确反馈
      const text = String(item.payload?.text ?? "").trim();
      errorMessage.value = text ? "公式尚未完成,请继续输入" : "请输入要计算的表达式";
      lastFailed.value = null;
      return;
    }
    await runWithRunner(async () => {
      try {
        await navigator.clipboard.writeText(raw);
        const message =
          display && display !== raw ? `已复制 ${raw}（显示 ${display}）` : `已复制 ${raw}`;
        return {
          closeSpotlight: true,
          toast: { message, type: "success" as const },
        };
      } catch {
        return { errorMessage: "复制到剪贴板失败" };
      }
    });
    return;
  }
  const provider = listProviders().find((p) => p.id === item.providerId);
  if (!provider) return;
  await runWithRunner(() => provider.defaultAction(item, buildContext()));
}

function openActionMenu(item: SpotlightItem) {
  if (executing.value) return;
  if (isKeywordItem(item)) {
    const actions = buildKeywordItemActions(item);
    if (actions.length === 0) return;
    actionMenuActions.value = actions as SpotlightAction[];
    actionMenuTargetItem.value = item;
    const row = rowRefs.value[activeIndex.value];
    actionMenuAnchor.value = row?.getBoundingClientRect() ?? null;
    actionMenuOpen.value = true;
    return;
  }
  const provider = listProviders().find((p) => p.id === item.providerId);
  if (!provider || !provider.buildActions) return;
  const actions = provider.buildActions(item);
  if (actions.length === 0) return;
  actionMenuActions.value = actions;
  actionMenuTargetItem.value = item;
  const row = rowRefs.value[activeIndex.value];
  actionMenuAnchor.value = row?.getBoundingClientRect() ?? null;
  actionMenuOpen.value = true;
}

function closeActionMenu() {
  actionMenuOpen.value = false;
  actionMenuTargetItem.value = null;
  nextTick(() => focusInput());
}

async function onActionSelect(action: SpotlightAction) {
  const item = actionMenuTargetItem.value;
  if (!item) return;
  actionMenuOpen.value = false;
  if (isKeywordItem(item)) {
    await runWithRunner(() => executeKeywordItemAction(item, action.id, buildContext()));
    return;
  }
  const provider = listProviders().find((p) => p.id === item.providerId);
  if (!provider) return;
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

function dismissError() {
  errorMessage.value = null;
  lastFailed.value = null;
}

function onKeydown(e: KeyboardEvent) {
  if (unlockState.value) return; // 解锁条自管理键盘
  if (actionMenuOpen.value) return; // ActionMenu 自管理键盘

  if (e.key === "Escape") {
    e.preventDefault();
    if (errorMessage.value) {
      dismissError();
      return;
    }
    void closeWindow();
    return;
  }
  if (executing.value) {
    e.preventDefault();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (results.value.length === 0) return;
    activeIndex.value = (activeIndex.value + 1) % results.value.length;
    scrollActiveResultIntoView();
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    if (results.value.length === 0) return;
    activeIndex.value = (activeIndex.value - 1 + results.value.length) % results.value.length;
    scrollActiveResultIntoView();
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
  if (e.altKey && !e.ctrlKey && !e.metaKey && e.key >= "1" && e.key <= "9") {
    e.preventDefault();
    const idx = parseInt(e.key, 10) - 1;
    const entry = results.value[idx];
    if (entry) void commitDefault(entry.item);
    return;
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
  try {
    unlistenReset = await listen(APP_EVENTS.SPOTLIGHT_RESET, () => {
      query.value = "";
      activeIndex.value = 0;
      errorMessage.value = null;
      lastFailed.value = null;
      clearSuccessBar();
      unlockState.value = null;
      actionMenuOpen.value = false;
      queryItemsByProvider.value = new Map();
      queryLoading.value = false;
      lastUsageSignature = "";
      // 窗口显示兜底:重新拉取配置,防止跨窗口广播丢失
      void configStore.ensureLoaded(true).then((v) => {
        view.value = v;
        void prefetchAll();
        void refreshQueryProviders();
        void refreshClipboardSuggestions();
      });
      nextTick(() => focusInput());
    });
  } catch {
    /* ignore */
  }

  browserProfilesListenerDisposed = false;
  void listenBrowserProfilesChanged(() => {
    void refreshBrowserProfilesProvider();
  })
    .then((unlisten) => {
      if (browserProfilesListenerDisposed) {
        unlisten();
        return;
      }
      unlistenBrowserProfilesChanged = unlisten;
    })
    .catch(() => {
      /* Tauri event listener is best-effort; APP_EVENTS.SPOTLIGHT_RESET remains the fallback. */
    });

  window.addEventListener("focus", onWindowFocus);
  window.addEventListener("keydown", onKeydown);

  await nextTick();
  focusInput();
  try {
    view.value = await configStore.ensureLoaded();
  } catch {
    view.value = configStore.getView();
  }
  void configStore.startListening();
  unsubConfig = configStore.subscribe(async (nextView) => {
    const prevIds = new Set(view.value?.providers.filter((p) => p.enabled).map((p) => p.id) ?? []);
    const nextIds = new Set(nextView.providers.filter((p) => p.enabled).map((p) => p.id));
    view.value = nextView;
    const enabledChanged =
      prevIds.size !== nextIds.size ||
      [...prevIds].some((id) => !nextIds.has(id)) ||
      [...nextIds].some((id) => !prevIds.has(id));
    if (enabledChanged) {
      await prefetchAll();
    }
    void refreshQueryProviders();
  });
  await prefetchAll();
  void refreshQueryProviders();
  void refreshClipboardSuggestions();
});

onBeforeUnmount(() => {
  unlistenReset?.();
  browserProfilesListenerDisposed = true;
  unlistenBrowserProfilesChanged?.();
  unlistenBrowserProfilesChanged = null;
  unsubConfig?.();
  if (successTimeoutId != null) {
    window.clearTimeout(successTimeoutId);
    successTimeoutId = null;
  }
  window.removeEventListener("focus", onWindowFocus);
  window.removeEventListener("keydown", onKeydown);
});
</script>

<style scoped>
.spotlight {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  background: var(--lc-surface-0);
  border-radius: var(--lc-radius-md);
  box-shadow: var(--lc-shadow-lg);
  overflow: hidden;
  font-family: inherit;
  color: var(--lc-text);
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
  color: var(--lc-text);
  background: transparent;
}

.spotlight-input:read-only {
  color: var(--lc-text-secondary);
  cursor: wait;
}

.spotlight-inline-status {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  color: var(--lc-accent);
}

.spotlight-inline-status svg {
  width: 16px;
  height: 16px;
  animation: spotlight-spin 0.85s linear infinite;
}

.spotlight-scope-chip {
  flex-shrink: 0;
  font-size: 11px;
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--lc-accent-dim);
  color: var(--lc-accent-dark, #0284c7);
}

.spotlight-results {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 0;
}

.spotlight-empty {
  padding: 24px 20px;
  color: var(--lc-text-secondary);
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
  background: var(--lc-accent-dim);
}

.spotlight-row.is-disabled {
  cursor: wait;
  opacity: 0.72;
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
  background: var(--lc-accent);
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
  color: var(--lc-text);
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-row-desc {
  font-size: 12px;
  color: var(--lc-text-secondary);
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
  color: var(--lc-text-muted);
  background: var(--lc-surface-1);
  border-top: 1px solid rgba(0, 0, 0, 0.04);
}

@keyframes spotlight-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .spotlight-inline-status svg {
    animation: none;
  }
  .spotlight-row {
    transition: none;
  }
}
</style>
