<template>
  <div class="chmod-panel">
    <el-card shadow="never" class="section-card">
      <template #header><span>权限输入</span></template>
      <div class="input-area">
        <div class="input-row">
          <span class="field-label">数字模式</span>
          <el-input
            v-model="numericValue"
            style="width: 100px"
            maxlength="3"
            @change="onNumericChange"
          />
          <span class="field-label" style="margin-left: 16px">符号模式</span>
          <el-input :model-value="symbolicValue" style="width: 140px" readonly />
        </div>
        <div class="presets">
          <span class="field-label">常用预设</span>
          <div
            v-for="p in presetList"
            :key="p.value"
            class="preset-chip"
            :class="{ active: numericValue === p.value }"
            @click="applyPreset(p.value)"
          >
            <span class="preset-value">{{ p.value }}</span>
            <span class="preset-hint">{{ p.hint }}</span>
          </div>
        </div>
      </div>
    </el-card>

    <el-card shadow="never" class="section-card">
      <template #header><span>权限矩阵</span></template>
      <div class="chmod-matrix">
        <div class="matrix-cell matrix-corner"></div>
        <div class="matrix-cell matrix-header">读取 (r)</div>
        <div class="matrix-cell matrix-header">写入 (w)</div>
        <div class="matrix-cell matrix-header">执行 (x)</div>
        <div class="matrix-cell matrix-header">八进制</div>
        <template v-for="role in roles" :key="role.key">
          <div class="matrix-cell role-label">{{ role.label }}</div>
          <div class="matrix-cell check-cell">
            <el-checkbox v-model="perms[role.key].read" @change="onCheckboxChange" />
          </div>
          <div class="matrix-cell check-cell">
            <el-checkbox v-model="perms[role.key].write" @change="onCheckboxChange" />
          </div>
          <div class="matrix-cell check-cell">
            <el-checkbox v-model="perms[role.key].execute" @change="onCheckboxChange" />
          </div>
          <div class="matrix-cell octal-cell">{{ roleOctal(role.key) }}</div>
        </template>
      </div>
    </el-card>

    <el-card shadow="never" class="section-card">
      <template #header><span>应用命令</span></template>
      <div class="cmd-list">
        <div v-for="cmd in commands" :key="cmd.label" class="cmd-item">
          <span class="cmd-label">{{ cmd.label }}</span>
          <code class="cmd-code">{{ cmd.text }}</code>
          <el-button size="small" text type="primary" @click="copy(cmd.text)">复制</el-button>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

function copy(text: string) {
  navigator.clipboard.writeText(text).then(() => ElMessage.success("已复制"));
}

interface PermSet {
  read: boolean;
  write: boolean;
  execute: boolean;
}

const roles = [
  { key: "owner" as const, label: "所有者 (Owner)" },
  { key: "group" as const, label: "用户组 (Group)" },
  { key: "other" as const, label: "其他 (Other)" },
];

const presetList = [
  { value: "644", hint: "文件默认" },
  { value: "755", hint: "目录/脚本" },
  { value: "777", hint: "完全开放" },
  { value: "600", hint: "仅所有者" },
  { value: "400", hint: "只读" },
];

const perms = reactive<Record<string, PermSet>>({
  owner: { read: true, write: true, execute: false },
  group: { read: true, write: false, execute: false },
  other: { read: true, write: false, execute: false },
});

const numericValue = ref("644");
const symbolicValue = ref("rw-r--r--");

function roleOctal(key: string) {
  const p = perms[key];
  return (p.read ? 4 : 0) + (p.write ? 2 : 0) + (p.execute ? 1 : 0);
}

const commands = computed(() => [
  { label: "文件", text: `chmod ${numericValue.value} file.txt` },
  { label: "目录", text: `chmod ${numericValue.value} /path/to/dir` },
  { label: "递归", text: `chmod -R ${numericValue.value} /path/to/dir` },
]);

function computeFromCheckbox() {
  const o = roleOctal("owner");
  const g = roleOctal("group");
  const t = roleOctal("other");
  numericValue.value = `${o}${g}${t}`;
  const sym = [perms.owner, perms.group, perms.other]
    .map((p) => `${p.read ? "r" : "-"}${p.write ? "w" : "-"}${p.execute ? "x" : "-"}`)
    .join("");
  symbolicValue.value = sym;
}

function onCheckboxChange() {
  computeFromCheckbox();
}

async function onNumericChange() {
  try {
    const data = (await invokeToolByChannel("tool:network:chmod-calc", {
      mode: "numeric",
      value: numericValue.value,
    })) as {
      numeric: string;
      symbolic: string;
      owner: PermSet;
      group: PermSet;
      other: PermSet;
    };
    Object.assign(perms.owner, data.owner);
    Object.assign(perms.group, data.group);
    Object.assign(perms.other, data.other);
    symbolicValue.value = data.symbolic;
    numericValue.value = data.numeric;
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function applyPreset(val: string) {
  numericValue.value = val;
  onNumericChange();
}
</script>

<style scoped>
.chmod-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-width: 560px;
}
.section-card :deep(.el-card__header) {
  padding: 10px 16px;
  font-weight: 600;
  font-size: 13px;
}
.section-card :deep(.el-card__body) {
  padding: 14px 16px;
}
.input-area {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.input-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.field-label {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
}
.presets {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.preset-chip {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 6px 14px;
  border-radius: 6px;
  border: 1px solid var(--el-border-color);
  background: var(--el-bg-color);
  cursor: pointer;
  transition: all 0.15s;
  user-select: none;
}
.preset-chip:hover {
  border-color: var(--el-color-primary-light-3);
  background: var(--el-color-primary-light-9);
}
.preset-chip.active {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}
.preset-value {
  font-weight: 700;
  font-size: 14px;
  font-family: monospace;
  line-height: 1.2;
}
.preset-chip.active .preset-value {
  color: var(--el-color-primary);
}
.preset-hint {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  line-height: 1.2;
}
.chmod-matrix {
  display: grid;
  grid-template-columns: 120px repeat(3, 70px) 60px;
  gap: 0;
}
.matrix-cell {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 8px 4px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.matrix-corner {
  border-bottom: 2px solid var(--el-border-color);
}
.matrix-header {
  font-weight: 600;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  border-bottom: 2px solid var(--el-border-color);
}
.role-label {
  justify-content: flex-start;
  font-weight: 500;
  font-size: 13px;
}
.check-cell .el-checkbox {
  margin-right: 0;
}
.octal-cell {
  font-family: monospace;
  font-weight: 700;
  font-size: 14px;
  color: var(--el-color-primary);
}
.cmd-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.cmd-item {
  display: flex;
  align-items: center;
  gap: 8px;
}
.cmd-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  min-width: 32px;
}
.cmd-code {
  flex: 1;
  font-family: monospace;
  font-size: 13px;
  background: var(--el-fill-color-light);
  padding: 6px 12px;
  border-radius: 4px;
  border: 1px solid var(--el-border-color-lighter);
}
</style>
