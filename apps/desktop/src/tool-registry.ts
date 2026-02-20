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
  "json-xml": defineAsyncComponent(() => import("./components/JsonXmlPanel.vue")),
  "json-yaml": defineAsyncComponent(() => import("./components/JsonYamlPanel.vue")),
  "csv-json": defineAsyncComponent(() => import("./components/CsvJsonPanel.vue")),
  "text-process": defineAsyncComponent(() => import("./components/TextProcessPanel.vue")),
  regex: defineAsyncComponent(() => import("./components/RegexPanel.vue")),
  network: defineAsyncComponent(() => import("./components/NetworkPanel.vue")),
  hosts: defineAsyncComponent(() => import("./components/HostsPanel.vue")),
  ports: defineAsyncComponent(() => import("./components/PortsPanel.vue")),
  dns: defineAsyncComponent(() => import("./components/DnsPanel.vue")),
  env: defineAsyncComponent(() => import("./components/EnvPanel.vue")),
  "split-merge": defineAsyncComponent(() => import("./components/SplitMergePanel.vue")),
  image: defineAsyncComponent(() => import("./components/ImagePanel.vue")),
  "calc-draft": defineAsyncComponent(() => import("./components/CalcDraftPanel.vue")),
  timestamp: defineAsyncComponent(() => import("./components/TimestampPanel.vue")),
  uuid: defineAsyncComponent(() => import("./components/UuidPanel.vue")),
  cron: defineAsyncComponent(() => import("./components/CronPanel.vue")),
  jwt: defineAsyncComponent(() => import("./components/JwtPanel.vue")),
  "base-converter": defineAsyncComponent(() => import("./components/BaseConverterPanel.vue")),
  color: defineAsyncComponent(() => import("./components/ColorPanel.vue")),
  diff: defineAsyncComponent(() => import("./components/DiffPanel.vue")),
  markdown: defineAsyncComponent(() => import("./components/MarkdownPanel.vue")),
  hotkey: defineAsyncComponent(() => import("./components/HotkeyPanel.vue")),
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
