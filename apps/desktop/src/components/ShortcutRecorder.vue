<template>
  <div class="shortcut-recorder-row">
    <div class="shortcut-recorder-wrap" :style="{ width }">
      <input
        v-if="!manualMode"
        ref="inputRef"
        class="shortcut-recorder"
        :class="{ focused, conflict: !!conflictHint }"
        readonly
        :value="modelValue"
        :placeholder="focused ? '请按下快捷键组合...' : (placeholder || '点击此处录入快捷键')"
        @focus="onFocus"
        @blur="onBlur"
        @keydown="onKeydown"
      />
      <input
        v-else
        ref="manualInputRef"
        class="shortcut-recorder"
        :class="{ conflict: !!conflictHint }"
        :value="modelValue"
        placeholder="例如：Alt+Space"
        @input="onManualInput"
        @keydown.enter="manualInputRef?.blur()"
        @blur="onManualBlur"
      />
      <button
        v-if="modelValue"
        class="shortcut-recorder-clear"
        tabindex="-1"
        @mousedown.prevent="clearValue"
      >
        &times;
      </button>
    </div>
    <button
      class="shortcut-recorder-toggle"
      :title="manualMode ? '切换到按键录入' : '切换到手动输入'"
      @mousedown.prevent="toggleManualMode"
    >
      <svg v-if="manualMode" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
        <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
      </svg>
      <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
        <path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a.996.996 0 0 0 0-1.41l-2.34-2.34a.996.996 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/>
      </svg>
    </button>
    <span v-if="conflictHint" class="shortcut-recorder-hint">{{ conflictHint }}</span>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { pauseAllShortcuts, resumeAllShortcuts } from "../bridge/tauri";

const props = defineProps<{
  modelValue: string;
  placeholder?: string;
  width?: string;
  /** Return conflict label (e.g. "显示/隐藏") if shortcut is taken, or empty/undefined if ok */
  checkConflict?: (shortcut: string) => string | undefined;
}>();

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const manualInputRef = ref<HTMLInputElement | null>(null);
const focused = ref(false);
const manualMode = ref(false);
const conflictHint = ref("");
const isToggling = ref(false);

const MODIFIER_KEYS = new Set([
  "Control", "Alt", "Shift", "Meta",
  "ControlLeft", "ControlRight",
  "AltLeft", "AltRight",
  "ShiftLeft", "ShiftRight",
  "MetaLeft", "MetaRight",
]);

function mapKeyName(e: KeyboardEvent): string | null {
  const { key, code } = e;

  if (/^F(\d+)$/.test(key)) return key;
  if (/^Key([A-Z])$/.test(code)) return code.slice(3);
  if (/^Digit(\d)$/.test(code)) return code.slice(5);
  if (/^Numpad(\d)$/.test(code)) return `Numpad${code.slice(6)}`;

  const specialMap: Record<string, string> = {
    Space: "Space", Enter: "Enter", Tab: "Tab", Escape: "Esc",
    Backspace: "Backspace", Delete: "Delete", Insert: "Insert",
    Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown",
    ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
    PrintScreen: "PrintScreen", ScrollLock: "ScrollLock", Pause: "Pause",
    NumLock: "NumLock", CapsLock: "CapsLock",
  };
  if (specialMap[key]) return specialMap[key];

  const punctMap: Record<string, string> = {
    Semicolon: ";", Equal: "=", Comma: ",", Minus: "-", Period: ".",
    Slash: "/", Backquote: "`", BracketLeft: "[", Backslash: "\\",
    BracketRight: "]", Quote: "'",
  };
  if (punctMap[code]) return punctMap[code];

  if (key.length === 1) return key.toUpperCase();
  return null;
}

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

function onKeydown(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  const sc = keyEventToShortcut(e);
  if (!sc) return;

  conflictHint.value = "";
  if (props.checkConflict) {
    const label = props.checkConflict(sc);
    if (label) {
      conflictHint.value = `该快捷键已被「${label}」使用`;
      return;
    }
  }
  emit("update:modelValue", sc);
}

async function onFocus() {
  focused.value = true;
  conflictHint.value = "";
  try { await pauseAllShortcuts(); } catch { /* ignore */ }
}

async function onBlur() {
  if (isToggling.value) return; // Skip if we're in the middle of mode toggle
  focused.value = false;
  // Only resume shortcuts if we're in key recording mode
  if (!manualMode.value) {
    try { await resumeAllShortcuts(); } catch { /* ignore */ }
  }
}

function clearValue() {
  conflictHint.value = "";
  emit("update:modelValue", "");
}

function toggleManualMode() {
  isToggling.value = true;
  const newMode = !manualMode.value;
  conflictHint.value = "";

  if (newMode) {
    // Switching to manual mode
    manualMode.value = true;
    focused.value = false;
    resumeAllShortcuts().catch(() => {});
    setTimeout(() => {
      manualInputRef.value?.focus();
      isToggling.value = false;
    }, 0);
  } else {
    // Switching to key mode
    manualMode.value = false;
    setTimeout(() => {
      inputRef.value?.focus();
      isToggling.value = false;
    }, 0);
  }
}

function onManualInput(e: Event) {
  const value = (e.target as HTMLInputElement).value.trim();
  conflictHint.value = "";
  if (value && props.checkConflict) {
    const label = props.checkConflict(value);
    if (label) {
      conflictHint.value = `该快捷键已被「${label}」使用`;
      return;
    }
  }
  emit("update:modelValue", value);
}

function onManualBlur() {
  // Trim and validate on blur
  const value = props.modelValue.trim();
  if (value !== props.modelValue) {
    emit("update:modelValue", value);
  }
}

function focus() {
  inputRef.value?.focus();
}

defineExpose({ focus });
</script>

<style scoped>
.shortcut-recorder-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.shortcut-recorder-wrap {
  position: relative;
  display: inline-block;
  width: 260px;
  flex-shrink: 0;
}

.shortcut-recorder {
  width: 100%;
  height: 32px;
  padding: 0 28px 0 12px;
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
  box-sizing: border-box;
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

.shortcut-recorder.conflict {
  border-color: var(--el-color-danger);
  box-shadow: 0 0 0 1px var(--el-color-danger-light-5);
}

.shortcut-recorder-hint {
  font-size: 12px;
  color: var(--el-color-danger);
  white-space: nowrap;
}

.shortcut-recorder-toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--el-border-color);
  border-radius: 4px;
  background: var(--el-bg-color);
  color: var(--el-text-color-secondary);
  cursor: pointer;
  transition: all 0.2s;
  flex-shrink: 0;
}

.shortcut-recorder-toggle:hover {
  color: var(--el-color-primary);
  border-color: var(--el-color-primary-light-5);
}

.shortcut-recorder-clear {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  width: 18px;
  height: 18px;
  border: none;
  border-radius: 50%;
  background: var(--el-text-color-placeholder);
  color: var(--el-bg-color);
  font-size: 13px;
  line-height: 18px;
  text-align: center;
  cursor: pointer;
  opacity: 0.6;
  transition: opacity 0.2s;
  padding: 0;
}

.shortcut-recorder-clear:hover {
  opacity: 1;
}
</style>
