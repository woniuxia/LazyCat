<template>
  <div class="app-shell">
    <TopBar
      ref="topBarRef"
      :all-items="sidebarItems"
      :active-tool="activeTool"
      :search-meta-map="toolSearchMetaMap"
      :click-count-fn="recentClickCount"
      @select="onSelect"
      @goto-home="onSelect(HOME_ID)"
      @goto-settings="onSelect('settings')"
    />

    <!-- Tab Bar -->
    <TabBar
      v-if="showTabBar"
      :tabs="allTabs"
      :active-id="activeTool"
      @select="onTabSelect"
      @close="closeTab"
      @close-others="closeOthers"
      @close-left="closeToLeft"
      @close-right="closeToRight"
    >
      <template #actions>
        <button class="tab-bar-new-btn" @click="focusSearch">
          <el-icon><Plus /></el-icon>
          <span>新工具</span>
        </button>
      </template>
    </TabBar>

    <main class="content">
      <div class="content-inner">
        <ClipboardSuggestionBar @open-tool="onClipboardToolOpen" />

        <Transition name="panel-switch" mode="out-in">
          <HomePanel
            v-if="activeTool === HOME_ID"
            key="home"
            :all-items="visibleSidebarItems"
            :merged-home-tools="mergedHomeTools"
            :is-favorite="isFavorite"
            @open-tool="onSelect"
            @toggle-favorite="toggleFavorite"
            @reorder-favorites="reorderFavorites"
          />

          <component
            v-else-if="currentComponent"
            :is="currentComponent"
            :key="activeTool"
            v-bind="currentComponentProps"
            @open-tool="onSelect"
          />
        </Transition>
      </div>
    </main>
    <ShortcutHelpOverlay ref="shortcutHelp" />
  </div>

</template>

<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, provide } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ElMessage, ElMessageBox } from "element-plus";
import { Close, Plus } from "@element-plus/icons-vue";
import type { SidebarItem } from "./types";
import { APP_EVENTS } from "./bridge/events";
import { useFavorites } from "./composables/useFavorites";
import { useTabs } from "./composables/useTabs";
import { useMenuVisibility } from "./composables/useMenuVisibility";
import { initSettings, getSetting, setSetting } from "./composables/useSettings";
import {
  HOME_ID,
  HOME_TOOL,
  getSidebarItems,
  getAllTools,
  getAllToolMap,
  isRealToolId,
} from "./composables/toolCatalog";
import { registerHotkey, registerNamedHotkey } from "./bridge/tauri";
import { getToolComponent, ENCODE_PANEL_IDS } from "./tool-registry";
import HomePanel from "./components/HomePanel.vue";
import TopBar from "./components/TopBar.vue";
import TabBar from "./components/TabBar.vue";
import ShortcutHelpOverlay from "./components/ShortcutHelpOverlay.vue";
import ClipboardSuggestionBar from "./components/ClipboardSuggestionBar.vue";
import { useClipboardSuggestion } from "./composables/useClipboardSuggestion";
import { buildClipboardPathSuggestion, detectClipboardPath } from "./utils/clipboard-detect";
import {
  shouldHideNamedHotkeyWindow,
  type HotkeyNavigatePayload,
} from "./utils/hotkeyNavigate";

const { ensureClipboardListener, showSuggestion, setPendingToolInput } = useClipboardSuggestion();
const isTauriEnv = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const appWindow = isTauriEnv ? getCurrentWindow() : null;

const sidebarItems: SidebarItem[] = getSidebarItems();
const allTools = getAllTools();
const allToolMap = getAllToolMap();

const { openTabs, activeTabId, openTab, closeTab, closeOthers, closeToLeft, closeToRight } = useTabs();
const activeTool = activeTabId;
const hotkeyInput = ref("");
const snippetsHotkeyInput = ref("");
const vaultHotkeyInput = ref("");
const launcherHotkeyInput = ref("");
const todoHotkeyInput = ref("");
const quickCaptureHotkeyInput = ref("");
const referenceCardHotkeyInput = ref("");
const spotlightHotkeyInput = ref("");
const pendingTodoCreate = ref(false);
provide("pendingTodoCreate", pendingTodoCreate);
const shortcutHelp = ref<InstanceType<typeof ShortcutHelpOverlay> | null>(null);
const topBarRef = ref<InstanceType<typeof TopBar> | null>(null);

function onKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === "/") {
    e.preventDefault();
    shortcutHelp.value?.show();
  }
  if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key >= "1" && e.key <= "9") {
    e.preventDefault();
    const idx = parseInt(e.key, 10) - 1;
    const visibleTabs = [
      HOME_ID,
      ...openTabs.value.filter(t => t.id !== HOME_ID).map(t => t.id),
    ];
    if (idx < visibleTabs.length) {
      onTabSelect(visibleTabs[idx]);
    }
  }
}

const {
  homeTopLimit,
  toolClickHistory,
  mergedHomeTools,
  isFavorite,
  toggleFavorite,
  reorderFavorites,
  recordToolClick,
  loadFromStorage: loadFavoritesFromStorage,
} = useFavorites(allTools, isRealToolId);

function recentClickCount(toolId: string): number {
  const cutoff = Date.now() - 30 * 24 * 60 * 60 * 1000;
  return (toolClickHistory.value[toolId] ?? []).filter((ts) => ts >= cutoff).length;
}

const sortedSidebarItems = computed<SidebarItem[]>(() => {
  const withScore = sidebarItems.map((item, idx) => {
    let total: number;
    if (item.kind === "tool") {
      total = recentClickCount(item.tool.id);
    } else {
      total = item.group.tools.reduce((sum, t) => sum + recentClickCount(t.id), 0);
    }
    return { item, total, originalIndex: idx };
  });

  withScore.sort((a, b) => {
    if (a.total === 0 && b.total === 0) return a.originalIndex - b.originalIndex;
    if (a.total === 0) return 1;
    if (b.total === 0) return -1;
    return b.total - a.total;
  });

  return withScore.map(({ item }) => {
    if (item.kind === "tool") return item;
    const sortedTools = [...item.group.tools].sort((a, b) => {
      const ca = recentClickCount(a.id);
      const cb = recentClickCount(b.id);
      if (ca === 0 && cb === 0) return 0;
      return cb - ca;
    });
    return { kind: "group" as const, group: { ...item.group, tools: sortedTools } };
  });
});

const {
  visibleSidebarItems,
  getHiddenIds,
  setHiddenIds,
  getToolSearchMetaMap,
  setToolSearchMetaMap,
  loadMenuVisibility,
} =
  useMenuVisibility(sortedSidebarItems);
const toolSearchMetaMap = computed(() => getToolSearchMetaMap());

const currentTool = computed(() => {
  if (activeTool.value === HOME_ID) return HOME_TOOL;
  if (activeTool.value === "settings") return { id: "settings", name: "设置", desc: "快捷键与应用偏好设置" };
  return allToolMap.get(activeTool.value);
});

const currentComponent = computed(() => getToolComponent(activeTool.value));

const currentComponentProps = computed(() => {
  const id = activeTool.value;
  if (ENCODE_PANEL_IDS.has(id)) return { activeTool: id };
  if (id.startsWith("manual-")) return { manualId: id };
  if (id === "settings") return {
    hotkeyInput: hotkeyInput.value,
    snippetsHotkeyInput: snippetsHotkeyInput.value,
    vaultHotkeyInput: vaultHotkeyInput.value,
    launcherHotkeyInput: launcherHotkeyInput.value,
    todoHotkeyInput: todoHotkeyInput.value,
    quickCaptureHotkeyInput: quickCaptureHotkeyInput.value,
    referenceCardHotkeyInput: referenceCardHotkeyInput.value,
    spotlightHotkeyInput: spotlightHotkeyInput.value,
    homeTopLimit: homeTopLimit.value,
    sidebarItems,
    getHiddenIds,
    setHiddenIds,
    getToolSearchMetaMap,
    setToolSearchMetaMap,
    "onUpdate:hotkeyInput": (v: string) => { hotkeyInput.value = v; },
    "onUpdate:snippetsHotkeyInput": (v: string) => { snippetsHotkeyInput.value = v; },
    "onUpdate:vaultHotkeyInput": (v: string) => { vaultHotkeyInput.value = v; },
    "onUpdate:launcherHotkeyInput": (v: string) => { launcherHotkeyInput.value = v; },
    "onUpdate:todoHotkeyInput": (v: string) => { todoHotkeyInput.value = v; },
    "onUpdate:quickCaptureHotkeyInput": (v: string) => { quickCaptureHotkeyInput.value = v; },
    "onUpdate:referenceCardHotkeyInput": (v: string) => { referenceCardHotkeyInput.value = v; },
    "onUpdate:spotlightHotkeyInput": (v: string) => { spotlightHotkeyInput.value = v; },
    "onUpdate:homeTopLimit": (v: number) => { homeTopLimit.value = v; },
  };
  return {};
});

// Show tab bar when there are tabs other than home, or home is not active
const showTabBar = computed(() => {
  return openTabs.value.length > 1 || (openTabs.value.length === 1 && openTabs.value[0].id !== HOME_ID);
});

// All tabs for TabBar component (openTabs already includes home)
const allTabs = computed(() => openTabs.value);

function onSelect(id: string) {
  const name = getToolName(id);
  if (id !== HOME_ID) recordToolClick(id);
  openTab(id, name);
}

function onTabSelect(id: string) {
  activeTabId.value = id;
}

function getToolName(id: string): string {
  if (id === HOME_ID) return "首页";
  if (id === "settings") return "设置";
  return allToolMap.get(id)?.name ?? id;
}

function onClipboardToolOpen(toolId: string, toolName: string) {
  recordToolClick(toolId);
  openTab(toolId, toolName);
}

function focusSearch() {
  topBarRef.value?.focusSearch();
}

async function tryOpenClipboardPathFromToggle(): Promise<boolean> {
  try {
    const text = await navigator.clipboard.readText();
    const match = detectClipboardPath(text);
    if (!match) return false;
    showSuggestion(buildClipboardPathSuggestion(match), text);
    return true;
  } catch {
    return false;
  }
}

async function ensureInboxCaptureConsent() {
  if (getSetting("inbox_capture_consent_ack") === "true") return;
  try {
    await ElMessageBox.confirm(
      "收纳箱会在应用运行期间记录最近复制的内容，用于历史流和后续整理。你可以随时在设置中关闭、暂停或限制隐藏时采集。是否启用？",
      "启用收纳箱后台采集",
      {
        confirmButtonText: "启用",
        cancelButtonText: "暂不启用",
        type: "warning",
        closeOnClickModal: false,
        closeOnPressEscape: false,
      },
    );
    setSetting("inbox_capture_consent_ack", "true");
    setSetting("inbox_capture_enabled", "true");
    if (getSetting("inbox_capture_when_hidden") === undefined) {
      setSetting("inbox_capture_when_hidden", "true");
    }
    if (getSetting("inbox_history_retention_days") === undefined) {
      setSetting("inbox_history_retention_days", "14");
    }
  } catch {
    setSetting("inbox_capture_consent_ack", "true");
    setSetting("inbox_capture_enabled", "false");
    if (getSetting("inbox_capture_when_hidden") === undefined) {
      setSetting("inbox_capture_when_hidden", "true");
    }
    if (getSetting("inbox_history_retention_days") === undefined) {
      setSetting("inbox_history_retention_days", "14");
    }
  }
}

onMounted(async () => {
  await initSettings();
  await ensureInboxCaptureConsent();
  loadFavoritesFromStorage();
  loadMenuVisibility();
  const savedHotkey = getSetting("hotkey") ?? "";
  hotkeyInput.value = savedHotkey;
  if (savedHotkey) {
    try { await registerHotkey(savedHotkey); } catch { /* ignore in non-Tauri env */ }
  }
  const savedSnippetsHotkey = getSetting("hotkey_snippets") ?? "";
  snippetsHotkeyInput.value = savedSnippetsHotkey;
  if (savedSnippetsHotkey) {
    try { await registerNamedHotkey("snippets", savedSnippetsHotkey); } catch { /* ignore */ }
  }
  const savedVaultHotkey = getSetting("hotkey_vault") ?? "";
  vaultHotkeyInput.value = savedVaultHotkey;
  if (savedVaultHotkey) {
    try { await registerNamedHotkey("vault", savedVaultHotkey); } catch { /* ignore */ }
  }
  const savedLauncherHotkey = getSetting("hotkey_launcher") ?? "";
  launcherHotkeyInput.value = savedLauncherHotkey;
  if (savedLauncherHotkey) {
    try { await registerNamedHotkey("launcher", savedLauncherHotkey); } catch { /* ignore */ }
  }
  const savedTodoHotkey = getSetting("hotkey_todo") ?? "";
  todoHotkeyInput.value = savedTodoHotkey;
  if (savedTodoHotkey) {
    try { await registerNamedHotkey("todo", savedTodoHotkey); } catch { /* ignore */ }
  }
  const savedQuickCaptureHotkey = getSetting("hotkey_quick_capture") ?? "Ctrl+Shift+N";
  quickCaptureHotkeyInput.value = savedQuickCaptureHotkey;
  if (savedQuickCaptureHotkey) {
    try { await registerNamedHotkey("quick-capture", savedQuickCaptureHotkey); } catch { /* ignore */ }
  }
  const savedReferenceCardHotkey = getSetting("hotkey_reference_card") ?? "Ctrl+Alt+Space";
  referenceCardHotkeyInput.value = savedReferenceCardHotkey;
  if (savedReferenceCardHotkey) {
    try { await registerNamedHotkey("reference-card", savedReferenceCardHotkey); } catch { /* ignore */ }
  }
  const savedSpotlightHotkey = getSetting("hotkey_spotlight") ?? "Ctrl+Shift+Space";
  spotlightHotkeyInput.value = savedSpotlightHotkey;
  if (savedSpotlightHotkey) {
    try { await registerNamedHotkey("spotlight", savedSpotlightHotkey); } catch { /* ignore */ }
  }
  try {
    await listen(APP_EVENTS.MAIN_WINDOW_TOGGLE, async () => {
      await tryOpenClipboardPathFromToggle();
      if (getSetting("focus_search_on_show") === "true") {
        focusSearch();
      }
    });
  } catch { /* ignore in non-Tauri env */ }
  try {
    await listen<{ kind: string; toolId?: string }>(APP_EVENTS.WIDGET_NAVIGATE, (event) => {
      const { kind, toolId } = event.payload;
      if (kind === "open-tool" && toolId) {
        onSelect(toolId);
      } else if (kind === "open-todo-create") {
        pendingTodoCreate.value = true;
        onSelect("todo");
      }
    });
  } catch { /* ignore in non-Tauri env */ }
  try {
    await listen<HotkeyNavigatePayload>(APP_EVENTS.HOTKEY_NAVIGATE, async (event) => {
      const { target, text, source, itemId, projectId, view } = event.payload;
      if (shouldHideNamedHotkeyWindow(event.payload, {
        activeTool: activeTool.value,
      })) {
        try {
          await appWindow?.hide();
          return;
        } catch { /* ignore in non-Tauri env */ }
      }
      if (itemId) {
        const parsedItem = Number(itemId);
        if (Number.isFinite(parsedItem)) {
          if (target === "pm") {
            const { usePmNavigation } = await import("./composables/usePmNavigation");
            const { hasView } = await import("./composables/pmViewRegistry");
            const parsedProject = projectId ? Number(projectId) : null;
            const projectIdValue = parsedProject != null && Number.isFinite(parsedProject)
              ? parsedProject
              : null;
            const viewId = view && hasView(view) ? view : undefined;
            usePmNavigation().requestFocus(parsedItem, projectIdValue, viewId);
          } else if (target === "todo") {
            const { useTodoNavigation } = await import("./composables/useTodoNavigation");
            useTodoNavigation().requestFocus(parsedItem);
          } else if (target === "data-dictionary") {
            const { useDataDictionaryNavigation } = await import("./composables/useDataDictionaryNavigation");
            useDataDictionaryNavigation().requestFocus(parsedItem);
          }
        }
      }
      onSelect(target);
      if (text && source) {
        setPendingToolInput({
          toolId: target,
          text,
          source: source === "clipboard-suggestion" ? "clipboard-suggestion" : "inbox",
        });
      }
    });
  } catch { /* ignore in non-Tauri env */ }
  try {
    await listen<{ name: string }>(APP_EVENTS.HOSTS_APPLIED, (event) => {
      const name = event.payload?.name ?? "";
      ElMessage.success(
        name
          ? `已应用 Hosts 配置：${name}（可在 Hosts 工具撤销）`
          : "已应用 Hosts 配置（可在 Hosts 工具撤销）",
      );
    });
  } catch { /* ignore in non-Tauri env */ }
  window.addEventListener("keydown", onKeydown);
  await ensureClipboardListener();
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>
