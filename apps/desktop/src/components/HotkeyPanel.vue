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
          <div v-if="!singleResult.available && singleSuspects.length > 0" class="suspects-inline">
            疑似占用: {{ singleSuspects.map(s => s.displayName).join(', ') }}
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
            <div class="suspects-note">Windows 无法精确查询某个快捷键由哪个进程注册，以上为基于运行进程的推测</div>
          </template>
        </el-alert>
      </div>

      <div class="panel-grid-full scan-summary">
        <el-tag>总计: {{ scanSummary.total }}</el-tag>
        <el-tag type="danger">已占用: {{ scanSummary.occupied }}</el-tag>
        <el-tag type="success">可用: {{ scanSummary.available }}</el-tag>
        <el-radio-group v-model="filterMode" size="small" style="margin-left: auto">
          <el-radio-button value="all">全部</el-radio-button>
          <el-radio-button value="occupied">已占用</el-radio-button>
          <el-radio-button value="available">可用</el-radio-button>
        </el-radio-group>
      </div>
      <el-table class="panel-grid-full" :data="filteredScanResults" border max-height="420" size="small">
        <el-table-column prop="shortcut" label="快捷键" min-width="200" />
        <el-table-column label="状态" width="120" align="center">
          <template #default="{ row }">
            <el-tag :type="row.available ? 'success' : 'danger'" size="small">
              {{ row.available ? '可用' : '已占用' }}
            </el-tag>
          </template>
        </el-table-column>
      </el-table>
    </template>
    <div v-else-if="!scanLoading" class="panel-grid-full empty-hint">
      点击"扫描常见快捷键"检测 Ctrl+Shift、Ctrl+Alt、Alt+数字、Ctrl+F 等常见组合的占用情况
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

interface SuspectApp {
  processName: string;
  displayName: string;
}

interface HotkeyResult {
  shortcut: string;
  available: boolean;
}

interface CheckResponse {
  shortcut: string;
  available: boolean;
  suspectedOwners: SuspectApp[];
}

interface ScanResponse {
  results: HotkeyResult[];
  scannedCount: number;
  occupiedCount: number;
  suspectedOwners: SuspectApp[];
}

const recorderRef = ref<HTMLInputElement | null>(null);
const recorderFocused = ref(false);
const shortcutInput = ref("");
const checkLoading = ref(false);
const singleResult = ref<CheckResponse | null>(null);
const singleSuspects = ref<SuspectApp[]>([]);

const scanLoading = ref(false);
const scanResults = ref<HotkeyResult[]>([]);
const scanSuspects = ref<SuspectApp[]>([]);
const showCustom = ref(false);
const customShortcuts = ref("");
const filterMode = ref<"all" | "occupied" | "available">("all");

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
  recorderRef.value?.focus();
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
    singleSuspects.value = resp.suspectedOwners ?? [];
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

.suspects-inline {
  margin-top: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.suspects-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}

.suspects-note {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.scan-summary {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.empty-hint {
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  text-align: center;
  padding: 32px 0;
}
</style>
