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
    <div v-if="showTabBar" class="tab-bar-wrap">
      <button
        class="tab-item"
        :class="{ 'is-active': activeTool === HOME_ID, 'is-pinned': true }"
        @click="onTabSelect(HOME_ID)"
      >
        <span class="tab-name">首页</span>
      </button>
      <button
        v-for="tab in openTabs.filter(t => t.id !== HOME_ID)"
        :key="tab.id"
        class="tab-item"
        :class="{ 'is-active': activeTool === tab.id }"
        @click="onTabSelect(tab.id)"
      >
        <span class="tab-name">{{ tab.name }}</span>
        <span
          v-if="!tab.pinned"
          class="tab-close"
          @click.stop="closeTab(tab.id)"
        >
          <el-icon><Close /></el-icon>
        </span>
      </button>
      <button class="tab-bar-new-btn" @click="focusSearch">
        <el-icon><Plus /></el-icon>
        <span>新工具</span>
      </button>
    </div>

    <main class="content">
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
        />
      </Transition>
    </main>
    <ShortcutHelpOverlay ref="shortcutHelp" />
  </div>

</template>

<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { Close, Plus } from "@element-plus/icons-vue";
import type { ToolDef, SidebarItem } from "./types";
import { useFavorites } from "./composables/useFavorites";
import { useTabs } from "./composables/useTabs";
import { useMenuVisibility } from "./composables/useMenuVisibility";
import { initSettings, getSetting, setSetting } from "./composables/useSettings";
import { registerHotkey, registerNamedHotkey } from "./bridge/tauri";
import { getToolComponent, ENCODE_PANEL_IDS } from "./tool-registry";
import HomePanel from "./components/HomePanel.vue";
import TopBar from "./components/TopBar.vue";
import ShortcutHelpOverlay from "./components/ShortcutHelpOverlay.vue";
import ClipboardSuggestionBar from "./components/ClipboardSuggestionBar.vue";
import { useClipboardSuggestion } from "./composables/useClipboardSuggestion";

const { detectClipboard } = useClipboardSuggestion();

const sidebarItems: SidebarItem[] = [
  { kind: "tool", tool: { id: "formatter", name: "代码格式化", desc: "JSON/XML/HTML/Java/SQL 自动格式化" } },
  { kind: "tool", tool: { id: "snippets", name: "代码片段", desc: "代码片段收藏与管理" } },
  { kind: "tool", tool: { id: "calc-draft", name: "计算草稿", desc: "草稿式计算，保留历史记录" } },
  { kind: "tool", tool: { id: "regex", name: "正则工具", desc: "表达式生成与测试" } },
  { kind: "tool", tool: { id: "diff", name: "文本对比", desc: "双栏文本差异对比" } },
  { kind: "tool", tool: { id: "markdown", name: "Markdown", desc: "Markdown 编辑与实时预览" } },
  { kind: "tool", tool: { id: "launcher", name: "快捷启动", desc: "常用程序快速启动与管理" } },
  {
    kind: "group",
    group: {
      id: "encode",
      name: "编解码",
      tools: [
        { id: "base64", name: "Base64", desc: "Base64 编码与解码" },
        { id: "url", name: "URL 编解码", desc: "URL Encode / Decode" },
        { id: "md5", name: "MD5", desc: "计算 MD5 摘要" },
        { id: "hash", name: "SHA/HMAC", desc: "SHA-1/256/512 与 HMAC-SHA256" },
        { id: "qr", name: "二维码生成", desc: "根据文本生成二维码" }
      ]
    }
  },
  {
    kind: "group",
    group: {
      id: "crypto",
      name: "加密与安全",
      tools: [
        { id: "rsa", name: "RSA 加解密", desc: "RSA 公私钥加解密" },
        { id: "aes", name: "AES/DES", desc: "AES / DES / 3DES 加解密" },
        { id: "jwt", name: "JWT 解析", desc: "离线解析 JWT Token" },
        { id: "uuid", name: "UUID/GUID", desc: "UUID 与 GUID 生成" },
        { id: "password", name: "密码工具", desc: "随机密码生成与强度分析" },
        { id: "bcrypt", name: "Bcrypt", desc: "Bcrypt 哈希生成与验证" },
        { id: "vault", name: "密码管理", desc: "应用/服务器/数据库密码加密存储" }
      ]
    }
  },
  {
    kind: "group",
    group: {
      id: "text",
      name: "数据转换",
      tools: [
        { id: "json-process", name: "JSON 处理", desc: "JSON 格式化/压缩/XML/YAML 互转" },
        { id: "json-schema", name: "JSON Schema", desc: "JSON Schema 校验与样例生成" },
        { id: "csv-json", name: "CSV/JSON", desc: "CSV 转 JSON" },
        { id: "java-bean-js", name: "JavaBean 转 JS", desc: "Java Bean 转 JSON 与 JS Object" },
        { id: "mybatis-helper", name: "MyBatis 助手", desc: "动态 SQL 渲染与占位符展开" },
        { id: "base-converter", name: "进制转换", desc: "二/八/十/十六进制转换" },
        { id: "color", name: "颜色转换", desc: "颜色格式互转与对比度检查" },
        { id: "escape-unescape", name: "转义/反转义", desc: "JSON/HTML/SQL/JS 字符串转义与反转义" },
        { id: "text-process", name: "文本处理", desc: "文本清洗、过滤提取与结果统计" },
        { id: "naming-case", name: "命名转换", desc: "camelCase/snake_case/PascalCase 互转" },
        { id: "config-convert", name: "配置互转", desc: "Properties/YAML/TOML/.env 格式互转" },
        { id: "sql-entity", name: "SQL 转实体类", desc: "CREATE TABLE 转 Java/TS/Go/Python 实体" },
      ]
    }
  },
  {
    kind: "group",
    group: {
      id: "network",
      name: "网络与系统",
      tools: [
        { id: "network", name: "IP/端口连通", desc: "TCP 与 HTTP 连通性测试" },
        { id: "dns", name: "DNS 查询", desc: "域名解析与记录查询" },
        { id: "capture", name: "抓包工具", desc: "数据包捕获与协议分析" },
        { id: "hosts", name: "Hosts 管理", desc: "多配置保存与切换" },
        { id: "ports", name: "端口占用", desc: "端口占用与进程分析" },
        { id: "env", name: "环境检测", desc: "检测 Node 与 Java 版本" },
        { id: "nginx-helper", name: "Nginx 助手", desc: "静态站点 + API 反代配置生成与校验" },
        { id: "hotkey", name: "快捷键检测", desc: "全局快捷键冲突检测" },
        { id: "http-status", name: "HTTP 状态码", desc: "HTTP 状态码速查与说明" },
        { id: "chmod-calc", name: "chmod 计算器", desc: "Linux 文件权限数字/符号互转" },
      ]
    }
  },
  {
    kind: "group",
    group: {
      id: "files",
      name: "文件与媒体",
      tools: [
        { id: "split-merge", name: "切分与合并", desc: "大文件切片与合并" },
        { id: "pdf", name: "PDF 工具", desc: "PDF 合并、拆分与信息查看" },
        { id: "image", name: "图片转换", desc: "格式转换、缩放、裁剪、压缩" }
      ]
    }
  },
  {
    kind: "group",
    group: {
      id: "calc",
      name: "时间工具",
      tools: [
        { id: "timestamp", name: "时间戳转换", desc: "时间戳与日期互转" },
        { id: "cron", name: "Cron 工具", desc: "Cron 表达式生成与预览" },
        { id: "date-calc", name: "日期计算器", desc: "日期间隔与日期加减计算" }
      ]
    }
  },
  {
    kind: "group",
    group: {
      id: "manuals",
      name: "离线手册",
      tools: [
        { id: "manual-vue3", name: "Vue 3 手册", desc: "Vue 3 中文开发手册" },
        { id: "manual-element-plus", name: "Element Plus", desc: "Element Plus 组件文档" },
        { id: "manual-mdn-js", name: "JavaScript", desc: "MDN JavaScript 中文参考手册" }
      ]
    }
  }
];
const HOME_ID = "home";
const HOME_TOOL: ToolDef = { id: HOME_ID, name: "首页", desc: "收藏工具与近一个月高频工具入口" };

const allTools = sidebarItems.flatMap((item) =>
  item.kind === "group" ? item.group.tools : [item.tool]
);
const allToolMap = new Map(allTools.map((tool) => [tool.id, tool]));
function isRealToolId(id: string) { return allToolMap.has(id); }

const { openTabs, activeTabId, openTab, closeTab, closeOthers, closeToLeft, closeToRight } = useTabs();
const activeTool = activeTabId;
const themeMode = ref<"system" | "dark" | "light">("system");
const hotkeyInput = ref("");
const snippetsHotkeyInput = ref("");
const vaultHotkeyInput = ref("");
const launcherHotkeyInput = ref("");
const shortcutHelp = ref<InstanceType<typeof ShortcutHelpOverlay> | null>(null);
const topBarRef = ref<InstanceType<typeof TopBar> | null>(null);

function onKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === "/") {
    e.preventDefault();
    shortcutHelp.value?.show();
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
    themeMode: themeMode.value,
    hotkeyInput: hotkeyInput.value,
    snippetsHotkeyInput: snippetsHotkeyInput.value,
    vaultHotkeyInput: vaultHotkeyInput.value,
    launcherHotkeyInput: launcherHotkeyInput.value,
    homeTopLimit: homeTopLimit.value,
    sidebarItems,
    getHiddenIds,
    setHiddenIds,
    getToolSearchMetaMap,
    setToolSearchMetaMap,
    "onUpdate:themeMode": (v: "system" | "dark" | "light") => { themeMode.value = v; },
    "onUpdate:hotkeyInput": (v: string) => { hotkeyInput.value = v; },
    "onUpdate:snippetsHotkeyInput": (v: string) => { snippetsHotkeyInput.value = v; },
    "onUpdate:vaultHotkeyInput": (v: string) => { vaultHotkeyInput.value = v; },
    "onUpdate:launcherHotkeyInput": (v: string) => { launcherHotkeyInput.value = v; },
    "onUpdate:homeTopLimit": (v: number) => { homeTopLimit.value = v; },
  };
  return {};
});

// Show tab bar when there are tabs other than home, or home is not active
const showTabBar = computed(() => {
  return openTabs.value.length > 1 || (openTabs.value.length === 1 && openTabs.value[0].id !== HOME_ID);
});

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

function resolveTheme(mode: "system" | "dark" | "light"): boolean {
  if (mode === "system") return window.matchMedia("(prefers-color-scheme: dark)").matches;
  return mode === "dark";
}

function applyTheme(dark: boolean) {
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

const systemMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
function onSystemThemeChange() {
  if (themeMode.value === "system") applyTheme(resolveTheme("system"));
}

watch(themeMode, (mode) => {
  applyTheme(resolveTheme(mode));
  setSetting("theme", mode);
});

onMounted(async () => {
  await initSettings();
  const savedTheme = getSetting("theme") as "system" | "dark" | "light" | null;
  if (savedTheme === "system" || savedTheme === "dark" || savedTheme === "light") {
    themeMode.value = savedTheme;
  } else if (savedTheme === null || savedTheme === undefined) {
    themeMode.value = "system";
  } else {
    themeMode.value = savedTheme === "light" ? "light" : "dark";
  }
  applyTheme(resolveTheme(themeMode.value));
  systemMediaQuery.addEventListener("change", onSystemThemeChange);
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
  try {
    await listen<string>("hotkey-navigate", (event) => {
      if (event.payload === "snippets") onSelect("snippets");
      else if (event.payload === "vault") onSelect("vault");
      else if (event.payload === "launcher") onSelect("launcher");
    });
  } catch { /* ignore in non-Tauri env */ }
  window.addEventListener("keydown", onKeydown);

  // 剪贴板智能检测：窗口获焦时自动检测
  try {
    await listen("tauri://focus", async () => {
      if (getSetting("clipboard_detection") === "false") return;
      await detectClipboard();
    });
  } catch { /* ignore in non-Tauri env */ }
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  systemMediaQuery.removeEventListener("change", onSystemThemeChange);
});
</script>
