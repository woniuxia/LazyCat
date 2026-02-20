<template>
  <div class="panel-grid">
    <div class="panel-grid-full color-preview-row">
      <div class="color-preview" :style="{ background: previewColor }"></div>
      <div class="color-inputs">
        <el-input v-model="hexInput" placeholder="#RRGGBB">
          <template #prepend>HEX</template>
        </el-input>
        <div class="color-rgb-row">
          <el-input v-model="rInput" placeholder="R" type="number" :min="0" :max="255" />
          <el-input v-model="gInput" placeholder="G" type="number" :min="0" :max="255" />
          <el-input v-model="bInput" placeholder="B" type="number" :min="0" :max="255" />
        </div>
        <div class="color-hsl-row">
          <el-input v-model="hInput" placeholder="H" type="number" :min="0" :max="360" />
          <el-input v-model="sInput" placeholder="S%" type="number" :min="0" :max="100" />
          <el-input v-model="lInput" placeholder="L%" type="number" :min="0" :max="100" />
        </div>
      </div>
    </div>
    <div class="panel-grid-full">
      <el-space>
        <el-button @click="copyText(hexInput)">复制 HEX</el-button>
        <el-button @click="copyText(`rgb(${rInput}, ${gInput}, ${bInput})`)">复制 RGB</el-button>
        <el-button @click="copyText(`hsl(${hInput}, ${sInput}%, ${lInput}%)`)">复制 HSL</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";

const hexInput = ref("#1890FF");
const rInput = ref("24");
const gInput = ref("144");
const bInput = ref("255");
const hInput = ref("209");
const sInput = ref("100");
const lInput = ref("55");

let updating = false;

const previewColor = computed(() => hexInput.value || "#000000");

function hexToRgb(hex: string): [number, number, number] | null {
  const m = hex.replace("#", "").match(/^([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!m) return null;
  return [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)];
}

function rgbToHex(r: number, g: number, b: number): string {
  return "#" + [r, g, b].map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0")).join("").toUpperCase();
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, Math.round(l * 100)];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
  else if (max === g) h = ((b - r) / d + 2) / 6;
  else h = ((r - g) / d + 4) / 6;
  return [Math.round(h * 360), Math.round(s * 100), Math.round(l * 100)];
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  h /= 360; s /= 100; l /= 100;
  if (s === 0) { const v = Math.round(l * 255); return [v, v, v]; }
  const hue2rgb = (p: number, q: number, t: number) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  return [Math.round(hue2rgb(p, q, h + 1 / 3) * 255), Math.round(hue2rgb(p, q, h) * 255), Math.round(hue2rgb(p, q, h - 1 / 3) * 255)];
}

watch(hexInput, (val) => {
  if (updating) return;
  const rgb = hexToRgb(val);
  if (!rgb) return;
  updating = true;
  rInput.value = String(rgb[0]); gInput.value = String(rgb[1]); bInput.value = String(rgb[2]);
  const [h, s, l] = rgbToHsl(...rgb);
  hInput.value = String(h); sInput.value = String(s); lInput.value = String(l);
  updating = false;
});

watch([rInput, gInput, bInput], ([r, g, b]) => {
  if (updating) return;
  const rn = parseInt(r), gn = parseInt(g), bn = parseInt(b);
  if ([rn, gn, bn].some(isNaN)) return;
  updating = true;
  hexInput.value = rgbToHex(rn, gn, bn);
  const [h, s, l] = rgbToHsl(rn, gn, bn);
  hInput.value = String(h); sInput.value = String(s); lInput.value = String(l);
  updating = false;
});

watch([hInput, sInput, lInput], ([h, s, l]) => {
  if (updating) return;
  const hn = parseInt(h), sn = parseInt(s), ln = parseInt(l);
  if ([hn, sn, ln].some(isNaN)) return;
  updating = true;
  const [r, g, b] = hslToRgb(hn, sn, ln);
  rInput.value = String(r); gInput.value = String(g); bInput.value = String(b);
  hexInput.value = rgbToHex(r, g, b);
  updating = false;
});

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}
</script>

<style scoped>
.color-preview-row {
  display: flex;
  gap: 16px;
  align-items: flex-start;
}

.color-preview {
  width: 160px;
  height: 160px;
  border-radius: var(--lc-radius-md);
  border: 1px solid var(--lc-border);
  flex-shrink: 0;
}

.color-inputs {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.color-rgb-row,
.color-hsl-row {
  display: flex;
  gap: 8px;
}

.color-rgb-row > *,
.color-hsl-row > * {
  flex: 1;
}
</style>
