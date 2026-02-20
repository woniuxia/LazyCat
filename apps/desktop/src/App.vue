<template>
  <div class="shell">
    <SidebarNav :items="sortedSidebarItems" :active-tool="activeTool" @select="onSelect" />

    <main class="content">
      <div class="tool-header">
        <h1 class="tool-title">{{ currentTool?.name }}</h1>
        <el-button
          v-if="activeTool !== HOME_ID && isRealToolId(activeTool)"
          text
          :type="isFavorite(activeTool) ? 'warning' : 'info'"
          @click="toggleFavorite(activeTool)"
        >
          {{ isFavorite(activeTool) ? "取消收藏" : "收藏" }}
        </el-button>
      </div>
      <p class="tool-desc">{{ currentTool?.desc }}</p>

      <HomePanel
        v-if="activeTool === HOME_ID"
        :favorite-tools="favoriteTools"
        :top-monthly-tools="topMonthlyTools"
        :home-top-limit="homeTopLimit"
        :is-favorite="isFavorite"
        @open-tool="onSelect"
        @toggle-favorite="toggleFavorite"
        @update:home-top-limit="homeTopLimit = $event"
      />

      <component
        v-else-if="currentComponent"
        :is="currentComponent"
        :key="activeTool"
        v-bind="currentComponentProps"
      />
    </main>
    <ShortcutHelpOverlay ref="shortcutHelp" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import type { ToolDef, SidebarItem } from "./types";
import { useFavorites } from "./composables/useFavorites";
import { initSettings, getSetting, setSetting } from "./composables/useSettings";
import { registerHotkey } from "./bridge/tauri";
import { getToolComponent, ENCODE_PANEL_IDS } from "./tool-registry";
import HomePanel from "./components/HomePanel.vue";
import SidebarNav from "./components/SidebarNav.vue";
import ShortcutHelpOverlay from "./components/ShortcutHelpOverlay.vue";

const sidebarItems: SidebarItem[] = [
  { kind: "tool", tool: { id: "formatter", name: "代码格式化", desc: "JSON/XML/HTML/Java/SQL 自动识别" } },
  { kind: "tool", tool: { id: "calc-draft", name: "计算草稿", desc: "草稿式计算，回车复制结果并保留历史" } },
  { kind: "tool", tool: { id: "regex", name: "正则工具", desc: "表达式生成与测试" } },
  { kind: "tool", tool: { id: "diff", name: "文本对比", desc: "双栏文本差异对比" } },
  { kind: "tool", tool: { id: "markdown", name: "Markdown", desc: "Markdown 编辑与实时预览" } },
  {
    kind: "group",
    group: {
      id: "encode",
      name: "编解码",
      tools: [
        { id: "base64", name: "Base64", desc: "Base64 编码与解码" },
        { id: "url", name: "URL 编解码", desc: "URL Encode / Decode" },
        { id: "md5", name: "MD5", desc: "计算 MD5 摘要" },
        { id: "hash", name: "SHA/HMAC", desc: "SHA-1/256/512 与 HMAC-SHA256 散列" },
        { id: "qr", name: "二维码生成", desc: "根据文本生成二维码" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "crypto",
      name: "加密与安全",
      tools: [
        { id: "rsa", name: "RSA 加解密", desc: "RSA 公私钥加解密" },
        { id: "aes", name: "AES/DES", desc: "AES 与 DES/3DES 加解密" },
        { id: "jwt", name: "JWT 解析", desc: "离线解析 JWT Token" },
        { id: "uuid", name: "UUID/GUID/密码", desc: "标识与随机密码生成" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "text",
      name: "数据转换",
      tools: [
        { id: "json-xml", name: "JSON/XML", desc: "JSON 与 XML 双向转换" },
        { id: "json-yaml", name: "JSON/YAML", desc: "JSON 转 YAML" },
        { id: "csv-json", name: "CSV/JSON", desc: "CSV 转 JSON" },
        { id: "base-converter", name: "进制转换", desc: "二/八/十/十六进制互转" },
        { id: "color", name: "颜色转换", desc: "HEX/RGB/HSL 互转与预览" },
        { id: "text-process", name: "文本处理", desc: "按行去重与排序" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "network",
      name: "网络与系统",
      tools: [
        { id: "network", name: "IP/端口连通", desc: "TCP 连通性测试" },
        { id: "dns", name: "DNS 查询", desc: "域名解析与记录查询" },
        { id: "hosts", name: "Hosts 管理", desc: "多配置保存与切换" },
        { id: "ports", name: "端口占用", desc: "端口与进程查询" },
        { id: "env", name: "环境检测", desc: "Node 与 Java 版本检测" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "files",
      name: "文件与媒体",
      tools: [
        { id: "split-merge", name: "切割与合并", desc: "大文件切片与合并" },
        { id: "image", name: "图片转换", desc: "格式转换、缩放、裁剪、压缩" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "calc",
      name: "时间工具",
      tools: [
        { id: "timestamp", name: "时间戳转换", desc: "时间戳与日期互转" },
        { id: "cron", name: "Cron 工具", desc: "Cron 表达式生成与预览" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "manuals",
      name: "离线手册",
      tools: [
        { id: "manual-vue3", name: "Vue 3 手册", desc: "Vue 3 中文开发手册" },
        { id: "manual-element-plus", name: "Element Plus", desc: "Element Plus 组件库文档" },
      ],
    },
  },
];

const HOME_ID = "home";
const HOME_TOOL: ToolDef = { id: HOME_ID, name: "首页", desc: "收藏页面与最近一个月高频功能入口" };

const allTools = sidebarItems.flatMap((item) =>
  item.kind === "group" ? item.group.tools : [item.tool]
);
const allToolMap = new Map(allTools.map((tool) => [tool.id, tool]));
function isRealToolId(id: string) { return allToolMap.has(id); }

const activeTool = ref(HOME_ID);
const themeMode = ref<"system" | "dark" | "light">("system");
const hotkeyInput = ref("");
const shortcutHelp = ref<InstanceType<typeof ShortcutHelpOverlay> | null>(null);

function onKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === "/") {
    e.preventDefault();
    shortcutHelp.value?.show();
  }
}

const {
  homeTopLimit,
  toolClickHistory,
  favoriteTools,
  topMonthlyTools,
  isFavorite,
  toggleFavorite,
  recordToolClick,
  loadFromStorage: loadFavoritesFromStorage,
} = useFavorites(allTools, isRealToolId);

/** 近 30 天内某工具的点击次数 */
function recentClickCount(toolId: string): number {
  const cutoff = Date.now() - 30 * 24 * 60 * 60 * 1000;
  return (toolClickHistory.value[toolId] ?? []).filter((ts) => ts >= cutoff).length;
}

/** 按点击热度排序的侧边栏：一级按子项合计点击降序，二级按点击降序；无点击的保持原序 */
const sortedSidebarItems = computed<SidebarItem[]>(() => {
  // 为每个一级条目计算总点击数
  const withScore = sidebarItems.map((item, idx) => {
    let total: number;
    if (item.kind === "tool") {
      total = recentClickCount(item.tool.id);
    } else {
      total = item.group.tools.reduce((sum, t) => sum + recentClickCount(t.id), 0);
    }
    return { item, total, originalIndex: idx };
  });

  // 稳定排序：有点击的按点击数降序，无点击的保持原始顺序
  withScore.sort((a, b) => {
    if (a.total === 0 && b.total === 0) return a.originalIndex - b.originalIndex;
    if (a.total === 0) return 1;
    if (b.total === 0) return -1;
    return b.total - a.total;
  });

  return withScore.map(({ item }) => {
    if (item.kind === "tool") return item;
    // 二级菜单按点击数降序排序
    const sortedTools = [...item.group.tools].sort((a, b) => {
      const ca = recentClickCount(a.id);
      const cb = recentClickCount(b.id);
      if (ca === 0 && cb === 0) return 0;
      return cb - ca;
    });
    return { kind: "group" as const, group: { ...item.group, tools: sortedTools } };
  });
});

const currentTool = computed(() => {
  if (activeTool.value === HOME_ID) return HOME_TOOL;
  if (activeTool.value === "settings") return { id: "settings", name: "设置", desc: "快捷键与应用偏好设置" };
  return allToolMap.get(activeTool.value);
});

const currentComponent = computed(() => getToolComponent(activeTool.value));

const currentComponentProps = computed(() => {
  const id = activeTool.value;
  // EncodePanel needs activeTool prop
  if (ENCODE_PANEL_IDS.has(id)) return { activeTool: id };
  // ManualPanel needs manualId prop
  if (id.startsWith("manual-")) return { manualId: id };
  // SettingsPanel needs themeMode and hotkeyInput with two-way binding
  if (id === "settings") return {
    themeMode: themeMode.value,
    hotkeyInput: hotkeyInput.value,
    "onUpdate:themeMode": (v: "system" | "dark" | "light") => { themeMode.value = v; },
    "onUpdate:hotkeyInput": (v: string) => { hotkeyInput.value = v; },
  };
  return {};
});

function onSelect(id: string) {
  if (id !== HOME_ID) recordToolClick(id);
  activeTool.value = id;
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
    // Legacy: "dark" or "light" string from old boolean storage
    themeMode.value = savedTheme === "light" ? "light" : "dark";
  }
  applyTheme(resolveTheme(themeMode.value));
  systemMediaQuery.addEventListener("change", onSystemThemeChange);
  loadFavoritesFromStorage();
  const savedHotkey = getSetting("hotkey") ?? "";
  hotkeyInput.value = savedHotkey;
  if (savedHotkey) {
    try { await registerHotkey(savedHotkey); } catch { /* ignore in non-Tauri env */ }
  }
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  systemMediaQuery.removeEventListener("change", onSystemThemeChange);
});
</script>
