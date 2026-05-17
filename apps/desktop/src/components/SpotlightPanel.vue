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

import SpotlightActionMenu from "./SpotlightActionMenu.vue";
import SpotlightErrorBar from "./SpotlightErrorBar.vue";
import SpotlightSuccessBar from "./SpotlightSuccessBar.vue";
import SpotlightVaultUnlockInput from "./SpotlightVaultUnlockInput.vue";

import { listProviders, getDescriptor, searchItems } from "../spotlight/registry";
import "../spotlight/providers/tool";
import "../spotlight/providers/vault";
import "../spotlight/providers/hosts";
import "../spotlight/providers/todo";
import "../spotlight/providers/pm";
import "../spotlight/providers/suggestion";
import "../spotlight/providers/launcher";
import * as configStore from "../spotlight/config-store";

import { parseSpotlightQuery, parseQuickCommand } from "../utils/spotlight-query";
import { calculateExpression, getCalcPreview } from "../utils/calc";
import { detectClipboardContent } from "../utils/clipboard-detect";
import { isRealToolId } from "../composables/toolCatalog";
import { initSettings, getSetting } from "../composables/useSettings";
import { createTodoDraft } from "../spotlight/providers/todo";
import type {
  SpotlightAction,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightProviderId,
  SpotlightView,
} from "../spotlight/types";

type ScopedItemsMap = Map<SpotlightProviderId, SpotlightItem[]>;

const RESULT_LIMIT = 12;

const query = ref("");
const activeIndex = ref(0);
const loading = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);
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

let unlockResolver: ((value: string | null) => void) | null = null;
const unlockState = ref<{ entryTitle: string } | null>(null);

let unlistenReset: UnlistenFn | null = null;
let unsubConfig: (() => void) | null = null;

const itemsByProvider = ref<ScopedItemsMap>(new Map());
const view = ref<SpotlightView | null>(null);

interface ClipboardSuggestion {
  toolId: string;
  toolName: string;
  text: string;
  preview: string;
}
const clipboardSuggestion = ref<ClipboardSuggestion | null>(null);

async function refreshClipboardSuggestion() {
  try {
    await initSettings();
  } catch {
    /* ignore */
  }
  if (getSetting("clipboard_detection") === "false") {
    clipboardSuggestion.value = null;
    return;
  }
  let text: string;
  try {
    text = await navigator.clipboard.readText();
  } catch {
    clipboardSuggestion.value = null;
    return;
  }
  if (!text) {
    clipboardSuggestion.value = null;
    return;
  }
  const detected = detectClipboardContent(text);
  const toolAction = detected?.actions.find((a) => a.kind === "tool");
  if (!toolAction || !isRealToolId(toolAction.toolId)) {
    clipboardSuggestion.value = null;
    return;
  }
  const oneLine = text.replace(/\n/g, " ").trim();
  const preview = oneLine.length > 32 ? oneLine.slice(0, 32) + "…" : oneLine;
  clipboardSuggestion.value = {
    toolId: toolAction.toolId,
    toolName: toolAction.toolName,
    text,
    preview,
  };
}

const parsed = computed(() =>
  parseSpotlightQuery(query.value, view.value?.aliasMap),
);
const quickCommand = computed(() =>
  parseQuickCommand(query.value, view.value?.enabledQuickCommands),
);
const scope = computed(() => (quickCommand.value ? null : parsed.value.scope));
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

const placeholder = computed(() => {
  if (scope.value) {
    return `在 ${scopeLabel.value || "该类型"} 中搜索…`;
  }
  const v = view.value;
  const enabledNames = v
    ? v.providers.filter((p) => p.enabled && !p.hiddenInSettings).map((p) => p.name)
    : [];
  if (enabledNames.length === 0) {
    return "所有数据源已禁用,前往设置启用";
  }
  return `搜索 ${enabledNames.join(" / ")}`;
});

const footerHint = computed(() => {
  if (unlockState.value) return "Enter 确认 · Esc 取消";
  if (actionMenuOpen.value) return "↑↓ 选择 · Enter 执行 · Esc 收起";
  if (errorMessage.value) return "Ctrl+R 重试 · Esc 关闭";
  return "Enter 执行 · Tab 备选动作 · Alt+1-9 直选 · Esc 关闭";
});

const results = computed(() => {
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
    // 空查询:合并多 provider 高频项作为首屏
    // - tool 自身已按 收藏/高频/其他 排序,作为主干
    // - 其它 provider 各取少量 top 项(按 prefetch 返回顺序,已排好)
    // - 整体按 provider.weight * item.weight 倒序,让重度使用的 launcher/vault 也能上首屏
    const SLOT_PER_OTHER = 2;
    const SLOT_TOOL = 8;
    const baseLimit =
      clipboardSuggestion.value && !scope.value ? RESULT_LIMIT - 1 : RESULT_LIMIT;
    const collected: { item: SpotlightItem; score: number }[] = [];
    const seen = new Set<string>();
    const providerWeight = (id: SpotlightProviderId) =>
      view.value?.providers.find((p) => p.id === id)?.weight ??
      getDescriptor(id)?.weight ??
      1;
    for (const [pid, items] of itemsByProvider.value) {
      const slot = pid === "tool" ? SLOT_TOOL : SLOT_PER_OTHER;
      const pw = providerWeight(pid);
      const top = items.slice(0, slot);
      for (const item of top) {
        const key = item.providerId + ":" + item.itemId;
        if (seen.has(key)) continue;
        seen.add(key);
        collected.push({ item, score: pw * (item.weight ?? 1) });
      }
    }
    collected.sort((a, b) => b.score - a.score);
    const baseEntries = collected.slice(0, baseLimit);
    if (scope.value || !clipboardSuggestion.value) return baseEntries;
    const s = clipboardSuggestion.value;
    const suggestionItem: SpotlightItem = {
      providerId: "suggestion",
      itemId: `suggestion:${s.toolId}`,
      title: `${s.toolName}（剪贴板：${s.preview}）`,
      subtitle: "Enter 打开并预填剪贴板内容",
      badge: { short: "建议", tone: "warn" },
      searchFields: [],
      payload: { toolId: s.toolId, text: s.text },
    };
    return [{ item: suggestionItem, score: 0 }, ...baseEntries].slice(0, RESULT_LIMIT);
  }
  return searchItems(text, itemsByProvider.value, {
    scope: scope.value,
    limit: RESULT_LIMIT,
    enabledIds: enabledProviderIds.value ?? undefined,
  });
});

watch(results, () => {
  if (activeIndex.value >= results.value.length) {
    activeIndex.value = results.value.length > 0 ? 0 : 0;
  }
});

// 用户继续输入新查询时,清除遗留的错误条/成功条与失败重试,避免红条/绿条粘连
// success bar 在等待自动关窗时被取消,意味着用户继续使用 spotlight,不再自动关
watch(query, (next, prev) => {
  if (next === prev) return;
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
  const providers = v
    ? v.providers.filter((p) => p.enabled)
    : listProviders();
  await Promise.allSettled(
    providers.map(async (provider) => {
      try {
        const items = await provider.prefetch();
        // 单 provider 完成后立即写回,渐进式更新而非等所有 provider
        const next = new Map(itemsByProvider.value);
        next.set(provider.id, items);
        itemsByProvider.value = next;
      } catch (err) {
        console.warn(`[Spotlight] provider ${provider.id} prefetch failed:`, err);
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
      errorMessage.value = text
        ? "公式尚未完成,请继续输入"
        : "请输入要计算的表达式";
      lastFailed.value = null;
      return;
    }
    await runWithRunner(async () => {
      try {
        await navigator.clipboard.writeText(raw);
        const message =
          display && display !== raw
            ? `已复制 ${raw}（显示 ${display}）`
            : `已复制 ${raw}`;
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
  nextTick(() => focusInput());
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
    unlistenReset = await listen("spotlight-reset", () => {
      query.value = "";
      activeIndex.value = 0;
      errorMessage.value = null;
      lastFailed.value = null;
      clearSuccessBar();
      unlockState.value = null;
      actionMenuOpen.value = false;
      // 窗口显示兜底:重新拉取配置,防止跨窗口广播丢失
      void configStore.ensureLoaded(true).then((v) => {
        view.value = v;
        void prefetchAll();
        void refreshClipboardSuggestion();
      });
      nextTick(() => focusInput());
    });
  } catch {
    /* ignore */
  }

  window.addEventListener("focus", onWindowFocus);

  await nextTick();
  focusInput();
  try {
    view.value = await configStore.ensureLoaded();
  } catch {
    view.value = configStore.getView();
  }
  void configStore.startListening();
  unsubConfig = configStore.subscribe(async (nextView) => {
    const prevIds = new Set(
      view.value?.providers.filter((p) => p.enabled).map((p) => p.id) ?? [],
    );
    const nextIds = new Set(nextView.providers.filter((p) => p.enabled).map((p) => p.id));
    view.value = nextView;
    const enabledChanged =
      prevIds.size !== nextIds.size ||
      [...prevIds].some((id) => !nextIds.has(id)) ||
      [...nextIds].some((id) => !prevIds.has(id));
    if (enabledChanged) {
      await prefetchAll();
    }
  });
  await prefetchAll();
  void refreshClipboardSuggestion();
});

onBeforeUnmount(() => {
  unlistenReset?.();
  unsubConfig?.();
  if (successTimeoutId != null) {
    window.clearTimeout(successTimeoutId);
    successTimeoutId = null;
  }
  window.removeEventListener("focus", onWindowFocus);
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
