import { defineAsyncComponent, type Component } from "vue";

/**
 * Maps tool IDs to their panel components.
 * Uses defineAsyncComponent for code-splitting.
 */
const toolRegistry: Record<string, Component> = {
  base64: defineAsyncComponent(() => import("./components/EncodePanel.vue")),
  url: defineAsyncComponent(() => import("./components/EncodePanel.vue")),
  md5: defineAsyncComponent(() => import("./components/EncodePanel.vue")),
  qr: defineAsyncComponent(() => import("./components/EncodePanel.vue")),
  hash: defineAsyncComponent(() => import("./components/EncodePanel.vue")),
  rsa: defineAsyncComponent(() => import("./components/RsaPanel.vue")),
  aes: defineAsyncComponent(() => import("./components/AesPanel.vue")),
  formatter: defineAsyncComponent(() => import("./components/FormatterPanel.vue")),
  "json-process": defineAsyncComponent(() => import("./components/JsonProcessPanel.vue")),
  "json-schema": defineAsyncComponent(() => import("./components/JsonSchemaPanel.vue")),
  "data-dictionary": defineAsyncComponent(() => import("./components/DataDictionaryPanel.vue")),
  "csv-json": defineAsyncComponent(() => import("./components/CsvJsonPanel.vue")),
  "java-bean-js": defineAsyncComponent(() => import("./components/JavaBeanJsPanel.vue")),
  "mybatis-helper": defineAsyncComponent(() => import("./components/MybatisPanel.vue")),
  "text-process": defineAsyncComponent(() => import("./components/TextProcessPanel.vue")),
  "naming-case": defineAsyncComponent(() => import("./components/NamingCasePanel.vue")),
  "config-convert": defineAsyncComponent(() => import("./components/ConfigConvertPanel.vue")),
  "sql-entity": defineAsyncComponent(() => import("./components/SqlEntityPanel.vue")),
  "http-status": defineAsyncComponent(() => import("./components/HttpStatusPanel.vue")),
  "chmod-calc": defineAsyncComponent(() => import("./components/ChmodCalcPanel.vue")),
  "date-calc": defineAsyncComponent(() => import("./components/DateCalcPanel.vue")),
  bcrypt: defineAsyncComponent(() => import("./components/BcryptPanel.vue")),
  regex: defineAsyncComponent(() => import("./components/RegexPanel.vue")),
  network: defineAsyncComponent(() => import("./components/NetworkPanel.vue")),
  "request-forward": defineAsyncComponent(() => import("./components/RequestForwardPanel.vue")),
  "api-mock": defineAsyncComponent(() => import("./components/ApiMockPanel.vue")),
  hosts: defineAsyncComponent(() => import("./components/HostsPanel.vue")),
  ports: defineAsyncComponent(() => import("./components/PortsPanel.vue")),
  dns: defineAsyncComponent(() => import("./components/DnsPanel.vue")),
  env: defineAsyncComponent(() => import("./components/EnvPanel.vue")),
  "split-merge": defineAsyncComponent(() => import("./components/SplitMergePanel.vue")),
  pdf: defineAsyncComponent(() => import("./components/PdfPanel.vue")),
  image: defineAsyncComponent(() => import("./components/ImagePanel.vue")),
  "calc-draft": defineAsyncComponent(() => import("./components/CalcDraftPanel.vue")),
  timestamp: defineAsyncComponent(() => import("./components/TimestampPanel.vue")),
  uuid: defineAsyncComponent(() => import("./components/UuidPanel.vue")),
  password: defineAsyncComponent(() => import("./components/PasswordPanel.vue")),
  cron: defineAsyncComponent(() => import("./components/CronPanel.vue")),
  jwt: defineAsyncComponent(() => import("./components/JwtPanel.vue")),
  "base-converter": defineAsyncComponent(() => import("./components/BaseConverterPanel.vue")),
  color: defineAsyncComponent(() => import("./components/ColorPanel.vue")),
  "escape-unescape": defineAsyncComponent(() => import("./components/EscapeUnescapePanel.vue")),
  diff: defineAsyncComponent(() => import("./components/DiffPanel.vue")),
  markdown: defineAsyncComponent(() => import("./components/MarkdownPanel.vue")),
  "nginx-helper": defineAsyncComponent(() => import("./components/NginxPanel.vue")),
  snippets: defineAsyncComponent(() => import("./components/SnippetPanel.vue")),
  vault: defineAsyncComponent(() => import("./components/VaultPanel.vue")),
  launcher: defineAsyncComponent(() => import("./components/LauncherPanel.vue")),
  "browser-profiles": defineAsyncComponent(() => import("./components/BrowserProfilesPanel.vue")),
  todo: defineAsyncComponent(() => import("./components/todo/TodoPanel.vue")),
  pomodoro: defineAsyncComponent(() => import("./components/PomodoroPanel.vue")),
  pm: defineAsyncComponent(() => import("./components/pm/PmPanel.vue")),
  "weekly-work": defineAsyncComponent(() => import("./components/WeeklyWorkPanel.vue")),
  inbox: defineAsyncComponent(() => import("./components/InboxPanel.vue")),
  maven: defineAsyncComponent(() => import("./components/MavenPanel.vue")),
  hotkey: defineAsyncComponent(() => import("./components/HotkeyPanel.vue")),
  widget: defineAsyncComponent(() => import("./components/WidgetPanel.vue")),
  "release-package": defineAsyncComponent(() => import("./components/ReleasePackagePanel.vue")),
  "action-center": defineAsyncComponent(() => import("./components/ActionCenterPanel.vue")),
  settings: defineAsyncComponent(() => import("./components/SettingsPanel.vue")),
};

export function getToolComponent(id: string): Component | undefined {
  // Manual panels use a prefix
  if (id.startsWith("manual-")) {
    return defineAsyncComponent(() => import("./components/ManualPanel.vue"));
  }
  return toolRegistry[id];
}

/** IDs for which EncodePanel needs to receive activeTool prop */
export const ENCODE_PANEL_IDS = new Set(["base64", "url", "md5", "qr", "hash"]);
