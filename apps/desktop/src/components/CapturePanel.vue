<template>
  <div class="capture-panel">
    <!-- Npcap 未安装提示 -->
    <div v-if="npcapChecked && !npcapInstalled" class="npcap-notice">
      <div class="npcap-notice__icon">
        <svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M12 9v4m0 4h.01M5.07 19h13.86c1.54 0 2.5-1.67 1.73-3L13.73 4c-.77-1.33-2.69-1.33-3.46 0L3.34 16c-.77 1.33.19 3 1.73 3z"/>
        </svg>
      </div>
      <div class="npcap-notice__body">
        <h3>需要安装 Npcap 驱动</h3>
        <p>数据包捕获依赖 Npcap 驱动程序，请先下载安装后再使用本工具。</p>
        <div class="npcap-notice__actions">
          <el-button type="primary" @click="openNpcapDownload">前往 Npcap 官网下载</el-button>
          <el-button @click="recheckNpcap">安装后刷新检测</el-button>
        </div>
      </div>
    </div>

    <!-- 功能未启用提示 -->
    <div v-else-if="featureDisabled" class="npcap-notice">
      <div class="npcap-notice__icon">
        <svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M13 10V3L4 14h7v7l9-11h-7z"/>
        </svg>
      </div>
      <div class="npcap-notice__body">
        <h3>抓包功能未启用</h3>
        <p>当前构建未包含 capture feature。需要安装 Npcap SDK 并使用 <code>--features capture</code> 重新构建。</p>
      </div>
    </div>

    <!-- 主界面 -->
    <template v-else-if="npcapChecked && npcapInstalled">
      <!-- 工具栏 -->
      <div class="capture-toolbar">
        <el-select
          v-model="selectedInterface"
          placeholder="选择网卡"
          style="width: 280px"
          :disabled="capturing"
        >
          <el-option
            v-for="iface in interfaces"
            :key="iface.name"
            :label="iface.description || iface.name"
            :value="iface.name"
          >
            <div class="iface-option">
              <span class="iface-option__name">{{ iface.description || iface.name }}</span>
              <span class="iface-option__addrs">{{ iface.addresses.join(', ') }}</span>
            </div>
          </el-option>
        </el-select>

        <el-input
          v-model="bpfFilter"
          placeholder="BPF 过滤器 (如: tcp port 80)"
          style="flex: 1; min-width: 180px"
          :disabled="capturing"
          clearable
          @keyup.enter="startCapture"
        >
          <template #prefix>
            <span style="font-family: var(--lc-font-mono); font-size: 12px; color: var(--lc-text-muted)">BPF</span>
          </template>
        </el-input>

        <div class="capture-toolbar__btns">
          <el-button
            v-if="!capturing"
            type="primary"
            :disabled="!selectedInterface"
            @click="startCapture"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" style="margin-right:4px"><polygon points="5,3 19,12 5,21"/></svg>
            开始捕获
          </el-button>
          <el-button
            v-else
            type="danger"
            @click="stopCapture"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" style="margin-right:4px"><rect x="4" y="4" width="16" height="16" rx="2"/></svg>
            停止
          </el-button>
          <el-button :disabled="packets.length === 0" @click="clearPackets">清空</el-button>
          <el-button :disabled="packets.length === 0" @click="exportPcap">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" style="margin-right:4px">
              <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/>
            </svg>
            导出
          </el-button>
        </div>
      </div>

      <!-- 数据包列表 -->
      <div class="capture-table-wrapper" ref="tableWrapperRef">
        <div class="capture-table-header">
          <div class="ct-col ct-col--idx">#</div>
          <div class="ct-col ct-col--time">时间</div>
          <div class="ct-col ct-col--src">源地址</div>
          <div class="ct-col ct-col--dst">目标地址</div>
          <div class="ct-col ct-col--proto">协议</div>
          <div class="ct-col ct-col--len">长度</div>
          <div class="ct-col ct-col--info">信息</div>
        </div>
        <div
          class="capture-table-body"
          ref="tableBodyRef"
          @scroll="onScroll"
        >
          <div :style="{ height: totalHeight + 'px', position: 'relative' }">
            <div
              v-for="item in visiblePackets"
              :key="item.index"
              class="ct-row"
              :class="[
                protocolClass(item.protocol),
                { 'ct-row--selected': selectedPacketIndex === item.index }
              ]"
              :style="{ position: 'absolute', top: (item._vOffset ?? 0) + 'px', width: '100%' }"
              @click="selectPacket(item)"
            >
              <div class="ct-col ct-col--idx">{{ item.index }}</div>
              <div class="ct-col ct-col--time">{{ item.timestamp.toFixed(6) }}</div>
              <div class="ct-col ct-col--src">{{ item.src }}</div>
              <div class="ct-col ct-col--dst">{{ item.dst }}</div>
              <div class="ct-col ct-col--proto">
                <span class="proto-badge" :class="protocolClass(item.protocol)">{{ item.protocol }}</span>
              </div>
              <div class="ct-col ct-col--len">{{ item.length }}</div>
              <div class="ct-col ct-col--info">{{ item.info }}</div>
            </div>
          </div>
        </div>
      </div>

      <!-- 数据包详情 -->
      <div v-if="selectedPacket" class="capture-detail">
        <div class="capture-detail__header">
          <span>数据包 #{{ selectedPacket.index }} 详情</span>
          <el-button size="small" text @click="selectedPacketIndex = null">关闭</el-button>
        </div>
        <div class="capture-detail__body">
          <div class="detail-section">
            <div class="detail-section__title">概要</div>
            <div class="detail-row">
              <span class="detail-label">协议</span>
              <span class="detail-value">
                <span class="proto-badge" :class="protocolClass(selectedPacket.protocol)">{{ selectedPacket.protocol }}</span>
              </span>
            </div>
            <div class="detail-row">
              <span class="detail-label">源地址</span>
              <span class="detail-value mono">{{ selectedPacket.src }}</span>
            </div>
            <div class="detail-row">
              <span class="detail-label">目标地址</span>
              <span class="detail-value mono">{{ selectedPacket.dst }}</span>
            </div>
            <div class="detail-row">
              <span class="detail-label">长度</span>
              <span class="detail-value">{{ selectedPacket.length }} bytes</span>
            </div>
            <div class="detail-row">
              <span class="detail-label">时间</span>
              <span class="detail-value mono">{{ selectedPacket.timestamp.toFixed(6) }}s</span>
            </div>
            <div class="detail-row">
              <span class="detail-label">信息</span>
              <span class="detail-value mono">{{ selectedPacket.info }}</span>
            </div>
          </div>
          <div class="detail-section">
            <div class="detail-section__title">Hex Dump</div>
            <pre class="hex-dump">{{ formatHexDump(selectedPacket.rawHex) }}</pre>
          </div>
        </div>
      </div>

      <!-- 状态栏 -->
      <div class="capture-statusbar">
        <span v-if="capturing" class="status-dot status-dot--active"></span>
        <span v-else class="status-dot"></span>
        <span>已捕获: {{ formatNumber(packets.length) }} 包</span>
        <span class="status-sep">|</span>
        <span>运行时间: {{ formattedDuration }}</span>
        <span class="status-sep">|</span>
        <span>数据量: {{ formatBytes(totalBytes) }}</span>
        <span v-if="captureRate > 0" class="status-sep">|</span>
        <span v-if="captureRate > 0">速率: ~{{ captureRate }} 包/秒</span>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from "vue";
import { invoke, Channel } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";

// ─── Types ───────────────────────────────────────────
interface InterfaceInfo {
  name: string;
  description: string;
  addresses: string[];
}

interface PacketInfo {
  index: number;
  timestamp: number;
  src: string;
  dst: string;
  protocol: string;
  length: number;
  info: string;
  rawHex: string;
  _vOffset?: number;
}

// ─── State ───────────────────────────────────────────
const npcapChecked = ref(false);
const npcapInstalled = ref(false);
const featureDisabled = ref(false);
const interfaces = ref<InterfaceInfo[]>([]);
const selectedInterface = ref("");
const bpfFilter = ref("");
const capturing = ref(false);
const packets = ref<PacketInfo[]>([]);
const selectedPacketIndex = ref<number | null>(null);
const sessionId = ref("");
const totalBytes = ref(0);
const startTime = ref(0);
const durationSecs = ref(0);
let durationTimer: ReturnType<typeof setInterval> | null = null;

// Virtual scroll
const ROW_HEIGHT = 28;
const tableBodyRef = ref<HTMLElement | null>(null);
const tableWrapperRef = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewportHeight = ref(400);
let autoScroll = true;

// ─── Computed ────────────────────────────────────────
const totalHeight = computed(() => packets.value.length * ROW_HEIGHT);

const visiblePackets = computed(() => {
  const start = Math.floor(scrollTop.value / ROW_HEIGHT);
  const count = Math.ceil(viewportHeight.value / ROW_HEIGHT) + 2;
  const startIdx = Math.max(0, start - 1);
  const endIdx = Math.min(packets.value.length, startIdx + count + 2);
  const result: PacketInfo[] = [];
  for (let i = startIdx; i < endIdx; i++) {
    const pkt = packets.value[i];
    result.push({ ...pkt, _vOffset: i * ROW_HEIGHT });
  }
  return result;
});

const selectedPacket = computed(() => {
  if (selectedPacketIndex.value === null) return null;
  return packets.value.find((p) => p.index === selectedPacketIndex.value) ?? null;
});

const formattedDuration = computed(() => {
  const s = Math.floor(durationSecs.value);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
});

const captureRate = computed(() => {
  if (durationSecs.value < 1) return 0;
  return Math.round(packets.value.length / durationSecs.value);
});

// ─── Methods ─────────────────────────────────────────
function protocolClass(proto: string) {
  const p = proto.toUpperCase();
  if (p === "TCP" || p === "HTTP" || p === "HTTPS") return "proto-tcp";
  if (p === "UDP" || p === "DHCP") return "proto-udp";
  if (p.startsWith("ICMP")) return "proto-icmp";
  if (p === "ARP") return "proto-arp";
  if (p === "DNS") return "proto-dns";
  return "proto-other";
}

function formatNumber(n: number): string {
  return n.toLocaleString();
}

function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / 1024 / 1024).toFixed(1)} MB`;
}

function formatHexDump(hex: string): string {
  if (!hex) return "";
  const bytes = hex.split(" ");
  const lines: string[] = [];
  for (let i = 0; i < bytes.length; i += 16) {
    const chunk = bytes.slice(i, i + 16);
    const offset = i.toString(16).padStart(4, "0");
    const hexPart = chunk.join(" ").padEnd(47, " ");
    const ascii = chunk
      .map((b) => {
        const code = parseInt(b, 16);
        return code >= 32 && code <= 126 ? String.fromCharCode(code) : ".";
      })
      .join("");
    lines.push(`${offset}  ${hexPart}  ${ascii}`);
  }
  return lines.join("\n");
}

function selectPacket(pkt: PacketInfo) {
  selectedPacketIndex.value = selectedPacketIndex.value === pkt.index ? null : pkt.index;
}

function onScroll() {
  if (!tableBodyRef.value) return;
  scrollTop.value = tableBodyRef.value.scrollTop;
  // Detect if user scrolled away from bottom
  const el = tableBodyRef.value;
  autoScroll = el.scrollTop + el.clientHeight >= el.scrollHeight - ROW_HEIGHT * 2;
}

function scrollToBottom() {
  if (!tableBodyRef.value || !autoScroll) return;
  tableBodyRef.value.scrollTop = tableBodyRef.value.scrollHeight;
}

async function recheckNpcap() {
  try {
    npcapInstalled.value = await invoke<boolean>("check_npcap_installed");
    if (npcapInstalled.value) {
      await loadInterfaces();
    }
  } catch {
    npcapInstalled.value = false;
  }
}

async function openNpcapDownload() {
  try {
    await invokeToolByChannel("tool:vault:open-url", { url: "https://npcap.com/#download" });
  } catch {
    // Fallback: try window.open (may be blocked in Tauri)
  }
}

async function loadInterfaces() {
  try {
    interfaces.value = await invoke<InterfaceInfo[]>("list_capture_interfaces");
    if (interfaces.value.length > 0) {
      // Auto-select first interface with addresses
      const withAddr = interfaces.value.find((i) => i.addresses.length > 0);
      selectedInterface.value = withAddr?.name ?? interfaces.value[0].name;
    }
  } catch (e: any) {
    if (typeof e === "string" && e.includes("capture feature")) {
      featureDisabled.value = true;
    }
  }
}

async function startCapture() {
  if (!selectedInterface.value || capturing.value) return;

  // Generate a unique session ID
  sessionId.value = `cap_${Date.now()}`;
  capturing.value = true;
  startTime.value = Date.now();
  durationSecs.value = 0;
  autoScroll = true;

  durationTimer = setInterval(() => {
    durationSecs.value = (Date.now() - startTime.value) / 1000;
  }, 200);

  const channel = new Channel<{
    event: string;
    data: any;
  }>();

  channel.onmessage = (message) => {
    if (message.event === "packets") {
      const items = message.data.items as PacketInfo[];
      packets.value.push(...items);
      totalBytes.value = packets.value.reduce((sum, p) => sum + p.length, 0);
      nextTick(scrollToBottom);
    } else if (message.event === "error") {
      console.error("Capture error:", message.data.message);
      // If it's a buffer limit error, auto-stop
      if (capturing.value) {
        stopCapture();
      }
    } else if (message.event === "stats") {
      durationSecs.value = message.data.durationSecs ?? durationSecs.value;
    }
  };

  try {
    await invoke("start_capture", {
      sessionId: sessionId.value,
      interface: selectedInterface.value,
      filter: bpfFilter.value,
      onPacket: channel,
    });
  } catch (e: any) {
    capturing.value = false;
    if (durationTimer) clearInterval(durationTimer);
    console.error("Failed to start capture:", e);
  }
}

async function stopCapture() {
  if (!capturing.value) return;
  capturing.value = false;
  if (durationTimer) {
    clearInterval(durationTimer);
    durationTimer = null;
  }

  try {
    const stats = await invoke<{ totalPackets: number; durationSecs: number; bytesCaptured: number }>(
      "stop_capture",
      { sessionId: sessionId.value }
    );
    durationSecs.value = stats.durationSecs;
  } catch (e) {
    console.error("Failed to stop capture:", e);
  }
}

async function clearPackets() {
  packets.value = [];
  selectedPacketIndex.value = null;
  totalBytes.value = 0;
  durationSecs.value = 0;
  // Clear session data on backend
  if (sessionId.value) {
    try {
      await invoke("clear_capture_session", { sessionId: sessionId.value });
    } catch {
      // ignore
    }
  }
}

async function exportPcap() {
  if (packets.value.length === 0 || !sessionId.value) return;

  const filePath = await save({
    title: "导出 pcap 文件",
    defaultPath: `capture_${new Date().toISOString().slice(0, 19).replace(/[:-]/g, "")}.pcap`,
    filters: [{ name: "PCAP", extensions: ["pcap"] }],
  });

  if (!filePath) return;

  try {
    await invoke("export_pcap", {
      sessionId: sessionId.value,
      path: filePath,
    });
  } catch (e) {
    console.error("Failed to export pcap:", e);
  }
}

// ─── Lifecycle ───────────────────────────────────────
onMounted(async () => {
  // Observe viewport height for virtual scroll
  if (tableBodyRef.value) {
    viewportHeight.value = tableBodyRef.value.clientHeight;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        viewportHeight.value = entry.contentRect.height;
      }
    });
    ro.observe(tableBodyRef.value);
  }

  try {
    npcapInstalled.value = await invoke<boolean>("check_npcap_installed");
    npcapChecked.value = true;
    if (npcapInstalled.value) {
      await loadInterfaces();
    }
  } catch (e: any) {
    // If the command itself fails, it might mean feature is disabled
    // check_npcap_installed should work without capture feature since it just checks filesystem
    npcapChecked.value = true;
    if (typeof e === "string" && e.includes("capture feature")) {
      featureDisabled.value = true;
    }
  }
});

onUnmounted(async () => {
  if (capturing.value) {
    await stopCapture();
  }
  if (durationTimer) {
    clearInterval(durationTimer);
  }
});
</script>

<style scoped>
.capture-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 0;
  overflow: hidden;
}

/* ─── Npcap Notice ──────────────────────────────── */
.npcap-notice {
  display: flex;
  align-items: flex-start;
  gap: 20px;
  padding: 32px;
  margin: 40px auto;
  max-width: 520px;
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border-hover);
  border-radius: var(--lc-radius-lg);
}

.npcap-notice__icon {
  color: var(--lc-warning);
  flex-shrink: 0;
}

.npcap-notice__body h3 {
  margin: 0 0 8px;
  font-size: 16px;
  font-weight: 600;
  color: var(--lc-text);
}

.npcap-notice__body p {
  margin: 0 0 16px;
  font-size: 13px;
  color: var(--lc-text-secondary);
  line-height: 1.6;
}

.npcap-notice__body code {
  background: var(--lc-surface-3);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: var(--lc-font-mono);
  font-size: 12px;
}

.npcap-notice__actions {
  display: flex;
  gap: 8px;
}

/* ─── Toolbar ───────────────────────────────────── */
.capture-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  background: var(--lc-surface-1);
  border-bottom: 1px solid var(--lc-border);
  flex-shrink: 0;
}

.capture-toolbar__btns {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.iface-option {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.iface-option__name {
  font-size: 13px;
}

.iface-option__addrs {
  font-size: 11px;
  color: var(--lc-text-muted);
  font-family: var(--lc-font-mono);
}

/* ─── Packet Table (virtual scroll) ─────────────── */
.capture-table-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
}

.capture-table-header {
  display: flex;
  align-items: center;
  height: 30px;
  background: var(--lc-surface-2);
  border-bottom: 1px solid var(--lc-border);
  font-size: 11px;
  font-weight: 600;
  color: var(--lc-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  flex-shrink: 0;
  padding: 0 4px;
}

.capture-table-body {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  min-height: 0;
}

.ct-row {
  display: flex;
  align-items: center;
  height: 28px;
  font-size: 12px;
  font-family: var(--lc-font-mono);
  color: var(--lc-text);
  cursor: pointer;
  border-bottom: 1px solid var(--lc-border-subtle);
  padding: 0 4px;
  transition: background var(--lc-duration) var(--lc-ease);
}

.ct-row:hover {
  background: rgba(255, 255, 255, 0.03);
}

.ct-row--selected {
  background: var(--lc-accent-dim) !important;
  border-color: var(--lc-accent-dim);
}

/* Protocol row tinting */
.ct-row.proto-tcp { background: rgba(96, 165, 250, 0.04); }
.ct-row.proto-udp { background: rgba(52, 211, 153, 0.04); }
.ct-row.proto-icmp { background: rgba(248, 113, 113, 0.04); }
.ct-row.proto-arp { background: rgba(251, 191, 36, 0.04); }
.ct-row.proto-dns { background: rgba(167, 139, 250, 0.04); }

.ct-row.proto-tcp:hover { background: rgba(96, 165, 250, 0.08); }
.ct-row.proto-udp:hover { background: rgba(52, 211, 153, 0.08); }
.ct-row.proto-icmp:hover { background: rgba(248, 113, 113, 0.08); }
.ct-row.proto-arp:hover { background: rgba(251, 191, 36, 0.08); }
.ct-row.proto-dns:hover { background: rgba(167, 139, 250, 0.08); }

/* Column widths */
.ct-col { padding: 0 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ct-col--idx { width: 60px; flex-shrink: 0; color: var(--lc-text-muted); text-align: right; }
.ct-col--time { width: 110px; flex-shrink: 0; }
.ct-col--src { width: 160px; flex-shrink: 0; }
.ct-col--dst { width: 160px; flex-shrink: 0; }
.ct-col--proto { width: 70px; flex-shrink: 0; text-align: center; }
.ct-col--len { width: 60px; flex-shrink: 0; text-align: right; }
.ct-col--info { flex: 1; min-width: 0; }

/* Protocol badge */
.proto-badge {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.03em;
  line-height: 16px;
}

.proto-badge.proto-tcp { background: rgba(96, 165, 250, 0.15); color: #93bbfd; }
.proto-badge.proto-udp { background: rgba(52, 211, 153, 0.15); color: #6ee7b7; }
.proto-badge.proto-icmp { background: rgba(248, 113, 113, 0.15); color: #fca5a5; }
.proto-badge.proto-arp { background: rgba(251, 191, 36, 0.15); color: #fcd34d; }
.proto-badge.proto-dns { background: rgba(167, 139, 250, 0.15); color: #c4b5fd; }
.proto-badge.proto-other { background: rgba(255, 255, 255, 0.06); color: var(--lc-text-secondary); }

/* ─── Detail Panel ──────────────────────────────── */
.capture-detail {
  flex-shrink: 0;
  max-height: 240px;
  overflow-y: auto;
  border-top: 2px solid var(--lc-accent-dim);
  background: var(--lc-surface-1);
}

.capture-detail__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 600;
  color: var(--lc-accent);
  background: var(--lc-surface-2);
  border-bottom: 1px solid var(--lc-border);
  position: sticky;
  top: 0;
  z-index: 1;
}

.capture-detail__body {
  padding: 8px 12px;
}

.detail-section {
  margin-bottom: 12px;
}

.detail-section__title {
  font-size: 11px;
  font-weight: 700;
  color: var(--lc-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 6px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--lc-border);
}

.detail-row {
  display: flex;
  align-items: baseline;
  padding: 2px 0;
  font-size: 12px;
}

.detail-label {
  width: 80px;
  flex-shrink: 0;
  color: var(--lc-text-muted);
}

.detail-value {
  color: var(--lc-text);
}

.detail-value.mono,
.mono {
  font-family: var(--lc-font-mono);
}

.hex-dump {
  font-family: var(--lc-font-mono);
  font-size: 11px;
  line-height: 1.6;
  color: var(--lc-text-secondary);
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-sm);
  padding: 8px 12px;
  margin: 0;
  overflow-x: auto;
  white-space: pre;
  max-height: 120px;
}

/* ─── Status Bar ────────────────────────────────── */
.capture-statusbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 12px;
  background: var(--lc-surface-2);
  border-top: 1px solid var(--lc-border);
  font-size: 11px;
  font-family: var(--lc-font-mono);
  color: var(--lc-text-secondary);
  flex-shrink: 0;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--lc-text-muted);
  flex-shrink: 0;
}

.status-dot--active {
  background: var(--lc-success);
  animation: pulse-dot 1.5s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%, 100% { opacity: 1; box-shadow: 0 0 0 0 rgba(52, 211, 153, 0.4); }
  50% { opacity: 0.7; box-shadow: 0 0 0 4px rgba(52, 211, 153, 0); }
}

.status-sep {
  color: var(--lc-text-muted);
  opacity: 0.5;
}
</style>
