<template>
  <div class="calc-draft">
    <div class="calc-draft-list">
      <el-empty v-if="!calcHistory.length" description="暂无计算记录，输入公式后按回车" />
      <div
        v-for="item in calcHistory"
        :key="item.id"
        class="calc-row calc-row-history"
        tabindex="0"
        @click="onHistoryClick(item)"
        @keyup.enter="onHistoryClick(item)"
      >
        <div class="calc-expression">{{ item.expression }}</div>
        <div class="calc-result-wrap">
          <div class="calc-result">= {{ item.resultDisplay }}</div>
          <div class="calc-time">{{ formatCalcHistoryTime(item.createdAt) }}</div>
        </div>
      </div>
    </div>
    <div class="calc-row calc-row-active">
      <el-input
        class="calc-input"
        :model-value="calcCurrentInput"
        type="textarea"
        :rows="3"
        placeholder="输入计算公式，例如 23.7%*5789+4587，按回车计算并复制结果"
        @update:model-value="onInput"
        @keydown.enter.exact.prevent="onCalcEnter"
      />
      <div class="calc-result calc-result-pending">= {{ calcCurrentPreview || "" }}</div>
    </div>
    <div class="calc-draft-actions">
      <el-space>
        <el-button type="primary" @click="onCalcEnter">计算并复制</el-button>
        <el-button @click="clearCalcHistory">清空历史</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import type { CalcDraftEntry } from "../types";
import { loadJson, saveJson } from "../composables/useLocalStorage";

const CALC_DRAFT_HISTORY_STORAGE_KEY = "lazycat:calc-draft-history:v1";
const MAX_CALC_HISTORY = 200;

const calcCurrentInput = ref("");
const calcHistory = ref<CalcDraftEntry[]>([]);

const calcCurrentPreview = computed(() => getCalcPreview(calcCurrentInput.value));

// Load history from localStorage on init
calcHistory.value = loadCalcHistoryFromStorage();

function loadCalcHistoryFromStorage(): CalcDraftEntry[] {
  const parsed = loadJson<unknown[]>(CALC_DRAFT_HISTORY_STORAGE_KEY, []);
  if (!Array.isArray(parsed)) return [];
  return parsed
    .filter((item): item is CalcDraftEntry => {
      const e = item as Record<string, unknown>;
      return (
        typeof e?.id === "number" &&
        typeof e?.expression === "string" &&
        typeof e?.resultRaw === "string" &&
        typeof e?.resultDisplay === "string" &&
        typeof e?.createdAt === "number"
      );
    })
    .slice(-MAX_CALC_HISTORY);
}

function normalizeExpression(input: string) {
  return input
    .replace(/[，,]/g, "")
    .replace(/、/g, "/")
    .replace(/[×xX]/g, "*")
    .replace(/÷/g, "/")
    .replace(/（/g, "(")
    .replace(/）/g, ")")
    .replace(/\s+/g, "")
    .replace(/(\d+(?:\.\d+)?)%/g, "($1/100)");
}

function formatCalcResult(value: number) {
  return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 15 }).format(value);
}

function calculateExpression(input: string): { rawValue: string; displayValue: string } {
  const normalized = normalizeExpression(input);
  if (!normalized) throw new Error("请输入计算公式");
  if (!/^[0-9+\-*/().]+$/.test(normalized)) throw new Error("仅支持数字和 + - * / ( ) 运算符");
  let result: unknown;
  try {
    result = Function(`"use strict"; return (${normalized});`)();
  } catch {
    throw new Error("公式格式不正确");
  }
  if (typeof result !== "number" || !Number.isFinite(result)) throw new Error("计算结果无效");
  return { rawValue: result.toString(), displayValue: formatCalcResult(result) };
}

function getCalcPreview(input: string) {
  const source = input.trim();
  if (!source) return "";
  try {
    return calculateExpression(source).displayValue;
  } catch { /* incomplete expression */ }
  const fallbackSource = source.replace(/[+\-*/xX×÷、(]+$/, "").trim();
  if (!fallbackSource) return "";
  try {
    return calculateExpression(fallbackSource).displayValue;
  } catch {
    return "";
  }
}

async function copyTextToClipboard(value: string) {
  try {
    await navigator.clipboard.writeText(value);
    return true;
  } catch {
    return false;
  }
}

async function onCalcEnter() {
  try {
    const expression = calcCurrentInput.value.trim();
    if (!expression) return;
    const result = calculateExpression(expression);
    const now = Date.now();
    const entry: CalcDraftEntry = {
      id: now,
      expression,
      resultRaw: result.rawValue,
      resultDisplay: result.displayValue,
      createdAt: now,
    };
    calcHistory.value = [...calcHistory.value, entry].slice(-MAX_CALC_HISTORY);
    const copied = await copyTextToClipboard(result.rawValue);
    calcCurrentInput.value = result.rawValue;
    ElMessage.success(
      copied
        ? `结果 ${result.displayValue} 已复制到剪贴板`
        : `结果 ${result.displayValue}（剪贴板写入失败）`,
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function clearCalcHistory() {
  calcHistory.value = [];
  ElMessage.success("计算历史已清空");
}

function onInput(value: string | number) {
  calcCurrentInput.value = String(value ?? "").replaceAll("、", "/");
}

async function onHistoryClick(item: CalcDraftEntry) {
  const copied = await copyTextToClipboard(item.resultRaw);
  ElMessage.success(copied ? `已复制结果: ${item.resultRaw}` : `复制失败，结果为: ${item.resultRaw}`);
}

function formatCalcHistoryTime(timestamp: number) {
  const date = new Date(timestamp);
  const yyyy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const dd = String(date.getDate()).padStart(2, "0");
  const hh = String(date.getHours()).padStart(2, "0");
  const mi = String(date.getMinutes()).padStart(2, "0");
  const ss = String(date.getSeconds()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}:${ss}`;
}

// Persist history
watch(calcHistory, () => saveJson(CALC_DRAFT_HISTORY_STORAGE_KEY, calcHistory.value), { deep: true });
</script>
