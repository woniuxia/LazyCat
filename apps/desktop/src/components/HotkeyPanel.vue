<template>
  <div class="panel-grid">
    <!-- 单个快捷键检测 -->
    <el-divider class="panel-grid-full" content-position="left">单个快捷键检测</el-divider>
    <div class="panel-grid-full input-row">
      <input
        ref="recorderRef"
        class="shortcut-recorder"
        :class="{ focused: recorderFocused }"
        readonly
        :value="shortcutInput"
        :placeholder="recorderFocused ? '请按下快捷键组合...' : '点击此处录入快捷键'"
        @focus="recorderFocused = true"
        @blur="recorderFocused = false"
        @keydown="onRecorderKeydown"
      />
      <el-button type="primary" :disabled="!shortcutInput" :loading="checkLoading" @click="checkSingle">检测</el-button>
      <el-button @click="clearSingle">清除</el-button>
    </div>
    <div v-if="singleResult !== null" class="panel-grid-full">
      <el-alert
        :title="singleResult.available ? '可用 - 该快捷键当前未被占用' : '已占用 - 该快捷键已被其他程序注册'"
        :type="singleResult.available ? 'success' : 'error'"
        show-icon
        :closable="false"
      >
        <template #default>
          <div>{{ singleResult.shortcut }}</div>
          <div v-if="!singleResult.available && singleSuspects.length > 0" class="suspects-detail">
            <div class="suspects-detail-title">疑似占用应用 (数据库推测):</div>
            <div v-for="s in singleSuspects" :key="s.appId" class="suspect-item">
              <el-tag
                :type="s.confidence === 'high' ? 'danger' : 'warning'"
                size="small"
              >
                {{ s.displayName }}
                <template v-if="s.confidence === 'high' && s.matchedHotkeys.length > 0">
                  - {{ s.matchedHotkeys.map(h => h.action).join(', ') }}
                </template>
              </el-tag>
              <span class="confidence-label">{{ s.confidence === 'high' ? '高置信' : '低置信' }}</span>
            </div>
          </div>
          <div v-if="!singleResult.available" class="detect-owner-section">
            <el-button
              size="small"
              type="warning"
              :loading="singleDetectLoading"
              @click="confirmDetectOwner(singleResult.shortcut)"
            >
              检测占用应用
            </el-button>
            <div v-if="singleDetectResult" class="detect-result">
              <template v-if="singleDetectResult.detected && singleDetectResult.owner">
                <el-tag type="danger" size="small">
                  {{ singleDetectResult.owner.processName }}
                </el-tag>
                <span class="detect-info">
                  PID: {{ singleDetectResult.owner.pid }}
                  <template v-if="singleDetectResult.owner.windowTitle">
                    | {{ singleDetectResult.owner.windowTitle }}
                  </template>
                </span>
                <el-tag
                  :type="confidenceTagType(singleDetectResult.confidence)"
                  size="small"
                  class="confidence-tag"
                >
                  {{ confidenceLabel(singleDetectResult.confidence) }}
                </el-tag>
              </template>
              <template v-else-if="singleDetectResult.detected && singleDetectResult.signals.clipboardChanged">
                <span class="detect-info">检测到剪贴板变化，可能是截图工具</span>
                <el-tag type="info" size="small" class="confidence-tag">低置信</el-tag>
              </template>
              <template v-else>
                <span class="detect-info">无法检测到响应进程（可能是后台类热键）</span>
              </template>
            </div>
          </div>
        </template>
      </el-alert>
    </div>

    <!-- 批量扫描 -->
    <el-divider class="panel-grid-full" content-position="left">批量扫描</el-divider>
    <div class="panel-grid-full input-row">
      <el-button type="primary" :loading="scanLoading" @click="scanDefaults">扫描常见快捷键</el-button>
      <el-button :loading="scanLoading" @click="showCustom = !showCustom">
        {{ showCustom ? '收起自定义' : '自定义扫描' }}
      </el-button>
    </div>
    <div v-if="showCustom" class="panel-grid-full">
      <el-input
        v-model="customShortcuts"
        type="textarea"
        :rows="4"
        placeholder="每行输入一个快捷键组合，例如：&#10;Ctrl+Shift+A&#10;Alt+F1&#10;Win+Shift+S"
      />
      <el-button style="margin-top: 8px" type="primary" :loading="scanLoading" :disabled="!customShortcuts.trim()" @click="scanCustom">
        扫描自定义列表
      </el-button>
    </div>

    <!-- 扫描进度 -->
    <div v-if="scanLoading" class="panel-grid-full">
      <el-progress :percentage="100" :indeterminate="true" :stroke-width="6" status="warning" />
      <div class="progress-hint">正在扫描快捷键占用情况...</div>
    </div>

    <!-- 扫描结果 -->
    <template v-if="scanResults.length > 0">
      <!-- 疑似占用应用 -->
      <div v-if="scanSuspects.length > 0" class="panel-grid-full">
        <el-alert type="warning" show-icon :closable="false">
          <template #title>
            <span>检测到以下运行中的应用可能注册了全局快捷键</span>
          </template>
          <template #default>
            <div class="suspects-list">
              <el-tag v-for="s in scanSuspects" :key="s.processName" size="small" type="warning">
                {{ s.displayName }}
              </el-tag>
            </div>
          </template>
        </el-alert>
      </div>

      <div class="panel-grid-full scan-summary">
        <el-tag>总计: {{ scanSummary.total }}</el-tag>
        <el-tag type="danger">已占用: {{ scanSummary.occupied }}</el-tag>
        <el-tag type="success">可用: {{ scanSummary.available }}</el-tag>
        <div class="scan-controls">
          <el-radio-group v-model="filterMode" size="small">
            <el-radio-button value="all">全部</el-radio-button>
            <el-radio-button value="occupied">已占用</el-radio-button>
            <el-radio-button value="available">可用</el-radio-button>
          </el-radio-group>
          <el-radio-group v-model="displayMode" size="small" style="margin-left: 8px">
            <el-radio-button value="list">列表</el-radio-button>
            <el-radio-button value="group">分组</el-radio-button>
          </el-radio-group>
          <el-dropdown trigger="click" style="margin-left: 8px" @command="handleExport">
            <el-button size="small">
              导出 <el-icon style="margin-left: 4px"><ArrowDown /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="clipboard">复制到剪贴板</el-dropdown-item>
                <el-dropdown-item command="file">保存为文本文件</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </div>

      <!-- 列表模式 -->
      <el-table v-if="displayMode === 'list'" class="panel-grid-full" :data="filteredScanResults" border max-height="420" size="small">
        <el-table-column prop="shortcut" label="快捷键" min-width="160" />
        <el-table-column label="状态" width="100" align="center">
          <template #default="{ row }">
            <el-tag :type="row.available ? 'success' : 'danger'" size="small">
              {{ row.available ? '可用' : '已占用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="疑似占用" min-width="240">
          <template #default="{ row }">
            <template v-if="!row.available">
              <!-- 实际检测结果 -->
              <template v-if="scanDetectResults[row.shortcut]">
                <template v-if="scanDetectResults[row.shortcut].detected && scanDetectResults[row.shortcut].owner">
                  <el-tag type="danger" size="small">{{ scanDetectResults[row.shortcut].owner!.processName }}</el-tag>
                  <el-tag :type="confidenceTagType(scanDetectResults[row.shortcut].confidence)" size="small" class="confidence-tag">
                    {{ confidenceLabel(scanDetectResults[row.shortcut].confidence) }}
                  </el-tag>
                </template>
                <template v-else-if="scanDetectResults[row.shortcut].signals.clipboardChanged">
                  <span class="detect-info-inline">剪贴板变化(截图工具?)</span>
                </template>
                <template v-else>
                  <span class="no-suspect">无法检测</span>
                </template>
              </template>
              <!-- 数据库推测 -->
              <template v-else-if="row.suspects && row.suspects.length > 0">
                <span v-for="(s, idx) in row.suspects" :key="s.appId">
                  <el-tag
                    :type="s.confidence === 'high' ? 'danger' : 'warning'"
                    size="small"
                    class="suspect-tag"
                  >
                    {{ s.displayName }}
                    <template v-if="s.confidence === 'high' && s.matchedHotkeys && s.matchedHotkeys.length > 0">
                      ({{ s.matchedHotkeys.map((h: { action: string }) => h.action).join(', ') }})
                    </template>
                  </el-tag>
                  <span v-if="idx < row.suspects.length - 1" class="suspect-sep" />
                </span>
              </template>
              <span v-else class="no-suspect">--</span>
            </template>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="90" align="center">
          <template #default="{ row }">
            <el-button
              v-if="!row.available"
              size="small"
              type="warning"
              link
              :loading="scanDetectLoadingKeys[row.shortcut]"
              @click="confirmDetectOwner(row.shortcut)"
            >
              检测
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <!-- 分组模式 -->
      <div v-else class="panel-grid-full">
        <el-collapse v-model="expandedGroups">
          <el-collapse-item v-for="group in modifierGroups" :key="group.key" :name="group.key">
            <template #title>
              <span class="group-title">{{ group.label }}</span>
              <el-tag size="small" type="info" class="group-count">{{ group.items.length }}</el-tag>
              <el-tag
                v-if="group.items.filter(i => !i.available).length > 0"
                size="small"
                type="danger"
                class="group-count"
              >
                {{ group.items.filter(i => !i.available).length }} 已占用
              </el-tag>
            </template>
            <el-table :data="group.items" border size="small">
              <el-table-column prop="shortcut" label="快捷键" min-width="160" />
              <el-table-column label="状态" width="100" align="center">
                <template #default="{ row }">
                  <el-tag :type="row.available ? 'success' : 'danger'" size="small">
                    {{ row.available ? '可用' : '已占用' }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="疑似占用" min-width="240">
                <template #default="{ row }">
                  <template v-if="!row.available">
                    <template v-if="scanDetectResults[row.shortcut]">
                      <template v-if="scanDetectResults[row.shortcut].detected && scanDetectResults[row.shortcut].owner">
                        <el-tag type="danger" size="small">{{ scanDetectResults[row.shortcut].owner!.processName }}</el-tag>
                        <el-tag :type="confidenceTagType(scanDetectResults[row.shortcut].confidence)" size="small" class="confidence-tag">
                          {{ confidenceLabel(scanDetectResults[row.shortcut].confidence) }}
                        </el-tag>
                      </template>
                      <template v-else-if="scanDetectResults[row.shortcut].signals.clipboardChanged">
                        <span class="detect-info-inline">剪贴板变化(截图工具?)</span>
                      </template>
                      <template v-else>
                        <span class="no-suspect">无法检测</span>
                      </template>
                    </template>
                    <template v-else-if="row.suspects && row.suspects.length > 0">
                      <span v-for="(s, idx) in row.suspects" :key="s.appId">
                        <el-tag
                          :type="s.confidence === 'high' ? 'danger' : 'warning'"
                          size="small"
                          class="suspect-tag"
                        >
                          {{ s.displayName }}
                          <template v-if="s.confidence === 'high' && s.matchedHotkeys && s.matchedHotkeys.length > 0">
                            ({{ s.matchedHotkeys.map((h: { action: string }) => h.action).join(', ') }})
                          </template>
                        </el-tag>
                        <span v-if="idx < row.suspects.length - 1" class="suspect-sep" />
                      </span>
                    </template>
                    <span v-else class="no-suspect">--</span>
                  </template>
                </template>
              </el-table-column>
              <el-table-column label="操作" width="90" align="center">
                <template #default="{ row }">
                  <el-button
                    v-if="!row.available"
                    size="small"
                    type="warning"
                    link
                    :loading="scanDetectLoadingKeys[row.shortcut]"
                    @click="confirmDetectOwner(row.shortcut)"
                  >
                    检测
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-collapse-item>
        </el-collapse>
      </div>
    </template>
    <div v-else-if="!scanLoading" class="panel-grid-full empty-hint">
      点击"扫描常见快捷键"检测 Ctrl+Shift、Ctrl+Alt、Alt+数字、Ctrl+F 等常见组合的占用情况
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { ArrowDown } from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ShortcutSuspect,
  SuspectApp,
  HotkeyResult,
  CheckResponse,
  ScanResponse,
  ModifierGroup,
  DetectOwnerResponse,
} from "../types";

const recorderRef = ref<HTMLInputElement | null>(null);
const recorderFocused = ref(false);
const shortcutInput = ref("");
const checkLoading = ref(false);
const singleResult = ref<CheckResponse | null>(null);
const singleSuspects = ref<ShortcutSuspect[]>([]);

const scanLoading = ref(false);
const scanResults = ref<HotkeyResult[]>([]);
const scanSuspects = ref<SuspectApp[]>([]);
const showCustom = ref(false);
const customShortcuts = ref("");
const filterMode = ref<"all" | "occupied" | "available">("all");
const displayMode = ref<"list" | "group">("list");
const expandedGroups = ref<string[]>([]);

// Detect owner state
const singleDetectLoading = ref(false);
const singleDetectResult = ref<DetectOwnerResponse | null>(null);
const scanDetectResults = reactive<Record<string, DetectOwnerResponse>>({});
const scanDetectLoadingKeys = reactive<Record<string, boolean>>({});

const MODIFIER_KEYS = new Set([
  "Control", "Alt", "Shift", "Meta",
  "ControlLeft", "ControlRight",
  "AltLeft", "AltRight",
  "ShiftLeft", "ShiftRight",
  "MetaLeft", "MetaRight",
]);

function keyEventToShortcut(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Win");

  if (parts.length === 0) return null;

  const keyName = mapKeyName(e);
  if (!keyName) return null;

  parts.push(keyName);
  return parts.join("+");
}

function mapKeyName(e: KeyboardEvent): string | null {
  const { key, code } = e;

  // Function keys
  if (/^F(\d+)$/.test(key)) return key;

  // Letters (use code to get consistent uppercase)
  if (/^Key([A-Z])$/.test(code)) return code.slice(3);

  // Digits (top row)
  if (/^Digit(\d)$/.test(code)) return code.slice(5);

  // Numpad digits
  if (/^Numpad(\d)$/.test(code)) return `Numpad${code.slice(6)}`;

  // Special keys
  const specialMap: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Escape: "Esc",
    Backspace: "Backspace",
    Delete: "Delete",
    Insert: "Insert",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    PrintScreen: "PrintScreen",
    ScrollLock: "ScrollLock",
    Pause: "Pause",
    NumLock: "NumLock",
    CapsLock: "CapsLock",
  };

  if (specialMap[key]) return specialMap[key];

  // Punctuation via code
  const punctMap: Record<string, string> = {
    Semicolon: ";",
    Equal: "=",
    Comma: ",",
    Minus: "-",
    Period: ".",
    Slash: "/",
    Backquote: "`",
    BracketLeft: "[",
    Backslash: "\\",
    BracketRight: "]",
    Quote: "'",
  };

  if (punctMap[code]) return punctMap[code];

  // Fallback: use key if single char
  if (key.length === 1) return key.toUpperCase();

  return null;
}

function onRecorderKeydown(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  const sc = keyEventToShortcut(e);
  if (sc) {
    shortcutInput.value = sc;
    singleResult.value = null;
    singleSuspects.value = [];
  }
}

function clearSingle() {
  shortcutInput.value = "";
  singleResult.value = null;
  singleSuspects.value = [];
  singleDetectResult.value = null;
  recorderRef.value?.focus();
}

function confidenceTagType(c: string): "" | "success" | "warning" | "info" | "danger" {
  switch (c) {
    case "high": return "danger";
    case "medium": return "warning";
    case "low": return "info";
    default: return "info";
  }
}

function confidenceLabel(c: string): string {
  switch (c) {
    case "high": return "高置信";
    case "medium": return "中置信";
    case "low": return "低置信";
    default: return "未检测到";
  }
}

async function confirmDetectOwner(shortcut: string) {
  try {
    await ElMessageBox.confirm(
      `检测将模拟按下 [${shortcut}]，可能触发该快捷键的原有功能（如截图、打开应用等）。是否继续？`,
      "检测占用应用",
      { confirmButtonText: "继续检测", cancelButtonText: "取消", type: "warning" }
    );
    await doDetectOwner(shortcut);
  } catch {
    // user cancelled
  }
}

async function doDetectOwner(shortcut: string) {
  // Determine if this is a single-check or batch-scan detect
  const isSingle = singleResult.value?.shortcut === shortcut;
  if (isSingle) {
    singleDetectLoading.value = true;
  } else {
    scanDetectLoadingKeys[shortcut] = true;
  }
  try {
    const data = await invokeToolByChannel("tool:hotkey:detect-owner", { shortcut });
    const resp = data as DetectOwnerResponse;
    if (isSingle) {
      singleDetectResult.value = resp;
    } else {
      scanDetectResults[shortcut] = resp;
    }
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    if (isSingle) {
      singleDetectLoading.value = false;
    } else {
      scanDetectLoadingKeys[shortcut] = false;
    }
  }
}

async function checkSingle() {
  if (!shortcutInput.value) return;
  checkLoading.value = true;
  try {
    const data = await invokeToolByChannel("tool:hotkey:check", {
      shortcut: shortcutInput.value,
    });
    const resp = data as CheckResponse;
    singleResult.value = resp;
    singleSuspects.value = resp.suspects ?? [];
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    checkLoading.value = false;
  }
}

async function scanDefaults() {
  scanLoading.value = true;
  scanResults.value = [];
  scanSuspects.value = [];
  filterMode.value = "all";
  try {
    const data = await invokeToolByChannel("tool:hotkey:scan", {});
    const payload = data as ScanResponse;
    scanSuspects.value = payload.suspectedOwners ?? [];
    // Sort: occupied first
    scanResults.value = payload.results.sort((a, b) => {
      if (a.available === b.available) return a.shortcut.localeCompare(b.shortcut);
      return a.available ? 1 : -1;
    });
    // Auto-expand groups with occupied keys
    expandedGroups.value = modifierGroups.value
      .filter(g => g.items.some(i => !i.available))
      .map(g => g.key);
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    scanLoading.value = false;
  }
}

async function scanCustom() {
  const lines = customShortcuts.value
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  if (lines.length === 0) return;

  scanLoading.value = true;
  scanResults.value = [];
  scanSuspects.value = [];
  filterMode.value = "all";
  try {
    const data = await invokeToolByChannel("tool:hotkey:scan", {
      shortcuts: lines,
    });
    const payload = data as ScanResponse;
    scanSuspects.value = payload.suspectedOwners ?? [];
    scanResults.value = payload.results.sort((a, b) => {
      if (a.available === b.available) return a.shortcut.localeCompare(b.shortcut);
      return a.available ? 1 : -1;
    });
    expandedGroups.value = modifierGroups.value
      .filter(g => g.items.some(i => !i.available))
      .map(g => g.key);
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    scanLoading.value = false;
  }
}

const scanSummary = computed(() => {
  const total = scanResults.value.length;
  const occupied = scanResults.value.filter((r) => !r.available).length;
  return { total, occupied, available: total - occupied };
});

const filteredScanResults = computed(() => {
  if (filterMode.value === "occupied") return scanResults.value.filter((r) => !r.available);
  if (filterMode.value === "available") return scanResults.value.filter((r) => r.available);
  return scanResults.value;
});

/** Extract modifier prefix from a shortcut string (everything except the last key) */
function getModifierPrefix(shortcut: string): string {
  const parts = shortcut.split("+");
  return parts.slice(0, -1).join("+");
}

const modifierGroups = computed<ModifierGroup[]>(() => {
  const filtered = filteredScanResults.value;
  const groupMap = new Map<string, HotkeyResult[]>();

  for (const item of filtered) {
    const prefix = getModifierPrefix(item.shortcut);
    if (!groupMap.has(prefix)) {
      groupMap.set(prefix, []);
    }
    groupMap.get(prefix)!.push(item);
  }

  return Array.from(groupMap.entries()).map(([key, items]) => ({
    key,
    label: key,
    items: items.sort((a, b) => a.shortcut.localeCompare(b.shortcut)),
  }));
});

/** Format scan results as plain text for export */
function formatExportText(): string {
  const lines: string[] = [];
  lines.push(`快捷键扫描结果 (${new Date().toLocaleString()})`);
  lines.push(`总计: ${scanSummary.value.total} | 已占用: ${scanSummary.value.occupied} | 可用: ${scanSummary.value.available}`);
  lines.push("");

  if (scanSuspects.value.length > 0) {
    lines.push("运行中的相关应用:");
    for (const s of scanSuspects.value) {
      lines.push(`  - ${s.displayName}`);
    }
    lines.push("");
  }

  lines.push("已占用的快捷键:");
  const occupied = scanResults.value.filter(r => !r.available);
  if (occupied.length === 0) {
    lines.push("  (无)");
  } else {
    for (const r of occupied) {
      let line = `  ${r.shortcut}`;
      if (r.suspects && r.suspects.length > 0) {
        const suspectStrs = r.suspects.map(s => {
          if (s.confidence === "high" && s.matchedHotkeys && s.matchedHotkeys.length > 0) {
            return `${s.displayName} (${s.matchedHotkeys.map(h => h.action).join(", ")})`;
          }
          return `${s.displayName} (?)`;
        });
        line += ` <- ${suspectStrs.join(", ")}`;
      }
      lines.push(line);
    }
  }

  return lines.join("\n");
}

async function handleExport(command: string) {
  if (scanResults.value.length === 0) {
    ElMessage.warning("没有可导出的扫描结果");
    return;
  }

  const text = formatExportText();

  if (command === "clipboard") {
    try {
      await navigator.clipboard.writeText(text);
      ElMessage.success("已复制到剪贴板");
    } catch {
      ElMessage.error("复制失败");
    }
  } else if (command === "file") {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const filePath = await save({
        defaultPath: `hotkey-scan-${Date.now()}.txt`,
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
      if (filePath) {
        await invokeToolByChannel("tool:file:write-text", { path: filePath, content: text });
        ElMessage.success("已保存");
      }
    } catch {
      // Fallback: copy to clipboard
      try {
        await navigator.clipboard.writeText(text);
        ElMessage.success("对话框不可用，已复制到剪贴板");
      } catch {
        ElMessage.error("导出失败");
      }
    }
  }
}
</script>

<style scoped>
.input-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.shortcut-recorder {
  flex: 1;
  min-width: 240px;
  height: 32px;
  padding: 0 12px;
  border: 1px solid var(--el-border-color);
  border-radius: 4px;
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--el-text-color-primary);
  background: var(--el-bg-color);
  outline: none;
  cursor: pointer;
  transition: border-color 0.2s;
  font-family: inherit;
}

.shortcut-recorder::placeholder {
  font-weight: 400;
  letter-spacing: 0;
  color: var(--el-text-color-placeholder);
}

.shortcut-recorder.focused {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 1px var(--el-color-primary-light-5);
}

.suspects-detail {
  margin-top: 8px;
}

.suspects-detail-title {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-bottom: 4px;
}

.suspect-item {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
}

.confidence-label {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
}

.suspects-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}

.scan-summary {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.scan-controls {
  display: flex;
  align-items: center;
  margin-left: auto;
  flex-wrap: wrap;
  gap: 4px;
}

.suspect-tag {
  margin-right: 4px;
}

.suspect-sep {
  display: inline;
}

.no-suspect {
  color: var(--el-text-color-placeholder);
  font-size: 12px;
}

.group-title {
  font-weight: 600;
  margin-right: 8px;
}

.group-count {
  margin-right: 6px;
}

.progress-hint {
  text-align: center;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
}

.empty-hint {
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  text-align: center;
  padding: 32px 0;
}

.detect-owner-section {
  margin-top: 10px;
  padding-top: 8px;
  border-top: 1px dashed var(--el-border-color-lighter);
}

.detect-result {
  margin-top: 8px;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.detect-info {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.detect-info-inline {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.confidence-tag {
  margin-left: 4px;
}
</style>
