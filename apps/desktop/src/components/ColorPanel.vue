<template>
  <div class="color-panel">
    <!-- Section 1: Color Picker + Format Inputs -->
    <div class="color-top-section">
      <div class="color-picker-area">
        <el-color-picker
          v-model="pickerColor"
          color-format="hex"
          :predefine="quickPredefine"
          size="large"
          @change="onPickerConfirm"
          @active-change="onPickerDrag"
        />
        <div
          class="color-swatch"
          :style="{ background: previewColor }"
          @click="copyText(hexInput)"
          title="点击复制 HEX"
        >
          <span class="swatch-hex">{{ hexInput }}</span>
        </div>
      </div>
      <div class="color-inputs">
        <div class="color-input-row">
          <span class="color-label">HEX</span>
          <el-input v-model="hexInput" placeholder="#RRGGBB" />
        </div>
        <div class="color-input-row color-multi">
          <span class="color-label">RGB</span>
          <el-input v-model="rInput" placeholder="R" type="number" :min="0" :max="255" />
          <el-input v-model="gInput" placeholder="G" type="number" :min="0" :max="255" />
          <el-input v-model="bInput" placeholder="B" type="number" :min="0" :max="255" />
        </div>
        <div class="color-input-row color-multi">
          <span class="color-label">HSL</span>
          <el-input v-model="hInput" placeholder="H" type="number" :min="0" :max="360" />
          <el-input v-model="sInput" placeholder="S%" type="number" :min="0" :max="100" />
          <el-input v-model="lInput" placeholder="L%" type="number" :min="0" :max="100" />
        </div>
        <div class="color-input-row color-multi">
          <span class="color-label">HSV</span>
          <el-input v-model="hvInput" placeholder="H" type="number" :min="0" :max="360" />
          <el-input v-model="svInput" placeholder="S%" type="number" :min="0" :max="100" />
          <el-input v-model="vvInput" placeholder="V%" type="number" :min="0" :max="100" />
        </div>
        <div class="color-input-row color-multi">
          <span class="color-label">HWB</span>
          <el-input v-model="hwbHInput" placeholder="H" type="number" :min="0" :max="360" />
          <el-input v-model="hwbWInput" placeholder="W%" type="number" :min="0" :max="100" />
          <el-input v-model="hwbBInput" placeholder="B%" type="number" :min="0" :max="100" />
        </div>
        <div class="color-input-row color-multi cmyk-row">
          <span class="color-label">CMYK</span>
          <el-input v-model="cmykC" placeholder="C%" type="number" :min="0" :max="100" />
          <el-input v-model="cmykM" placeholder="M%" type="number" :min="0" :max="100" />
          <el-input v-model="cmykY" placeholder="Y%" type="number" :min="0" :max="100" />
          <el-input v-model="cmykK" placeholder="K%" type="number" :min="0" :max="100" />
        </div>
      </div>
    </div>

    <!-- Copy Buttons -->
    <div class="color-copy-row">
      <el-button size="small" @click="copyText(hexInput)">HEX</el-button>
      <el-button size="small" @click="copyText(`rgb(${rInput}, ${gInput}, ${bInput})`)">RGB</el-button>
      <el-button size="small" @click="copyText(`hsl(${hInput}, ${sInput}%, ${lInput}%)`)">HSL</el-button>
      <el-button size="small" @click="copyText(`hsv(${hvInput}, ${svInput}%, ${vvInput}%)`)">HSV</el-button>
      <el-button size="small" @click="copyText(`hwb(${hwbHInput} ${hwbWInput}% ${hwbBInput}%)`)">HWB</el-button>
      <el-button size="small" @click="copyText(`cmyk(${cmykC}%, ${cmykM}%, ${cmykY}%, ${cmykK}%)`)">CMYK</el-button>
    </div>

    <!-- Section 2: Color Harmony -->
    <div class="color-section">
      <div class="color-section-title">配色推荐</div>
      <div v-for="scheme in harmonySchemes" :key="scheme.name" class="harmony-row">
        <span class="harmony-label">{{ scheme.name }}</span>
        <div class="harmony-swatches">
          <el-tooltip
            v-for="(c, i) in scheme.colors"
            :key="i"
            :content="c"
            placement="top"
          >
            <div
              class="harmony-swatch"
              :style="{ background: c }"
              @click="applyColor(c)"
            ></div>
          </el-tooltip>
        </div>
        <el-button
          size="small"
          text
          @click="copyText(scheme.colors.join(', '))"
          title="复制全部"
        >复制</el-button>
      </div>
    </div>

    <!-- Section 3: Preset Palettes -->
    <div class="color-section">
      <div class="color-section-title">
        预设调色板
        <div class="palette-tabs">
          <span
            v-for="tab in paletteTabs"
            :key="tab.key"
            class="palette-tab"
            :class="{ active: activePalette === tab.key }"
            @click="activePalette = tab.key"
          >{{ tab.name }}</span>
        </div>
      </div>
      <div class="palette-grid">
        <el-tooltip
          v-for="(c, i) in currentPalette"
          :key="i"
          :content="c.name + ' ' + c.hex"
          placement="top"
        >
          <div
            class="palette-swatch"
            :style="{ background: c.hex }"
            @click="applyColor(c.hex)"
          ></div>
        </el-tooltip>
      </div>
    </div>

    <!-- Section 4: WCAG Contrast Check -->
    <div class="color-section">
      <div class="color-section-title">
        对比度检查 (WCAG)
        <el-button size="small" text @click="useCurrentAsContrast" title="使用当前颜色作为前景色">使用当前色</el-button>
      </div>
      <div class="contrast-row">
        <div class="contrast-input-group">
          <span class="contrast-label">前景</span>
          <el-color-picker
            v-model="contrastFg"
            color-format="hex"
            size="small"
            @active-change="onContrastFgDrag"
            @change="onContrastFgConfirm"
          />
          <el-input v-model="contrastFg" size="small" placeholder="#000000" />
        </div>
        <div class="contrast-input-group">
          <span class="contrast-label">背景</span>
          <el-color-picker
            v-model="contrastBg"
            color-format="hex"
            size="small"
            @active-change="onContrastBgDrag"
            @change="onContrastBgConfirm"
          />
          <el-input v-model="contrastBg" size="small" placeholder="#FFFFFF" />
        </div>
        <div class="contrast-result">
          <div class="contrast-ratio">{{ contrastRatio }}:1</div>
          <div class="contrast-badges">
            <span class="contrast-badge" :class="wcagAA ? 'pass' : 'fail'">AA {{ wcagAA ? '通过' : '未通过' }}</span>
            <span class="contrast-badge" :class="wcagAAA ? 'pass' : 'fail'">AAA {{ wcagAAA ? '通过' : '未通过' }}</span>
          </div>
        </div>
      </div>
      <div
        class="contrast-preview"
        :style="{ background: contrastBg, color: contrastFg }"
      >示例文字 The quick brown fox jumps over the lazy dog</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";

// ─── State ───────────────────────────────────────────────

let updating = false;

// HEX
const hexInput = ref("#1890FF");
// RGB
const rInput = ref("24");
const gInput = ref("144");
const bInput = ref("255");
// HSL
const hInput = ref("209");
const sInput = ref("100");
const lInput = ref("55");
// HSV
const hvInput = ref("209");
const svInput = ref("91");
const vvInput = ref("100");
// HWB
const hwbHInput = ref("209");
const hwbWInput = ref("9");
const hwbBInput = ref("0");
// CMYK
const cmykC = ref("91");
const cmykM = ref("44");
const cmykY = ref("0");
const cmykK = ref("0");

// Picker
const pickerColor = ref("#1890FF");

// Contrast
const contrastFg = ref("#000000");
const contrastBg = ref("#FFFFFF");

// Palettes
type PaletteKey = "web" | "chinese" | "material" | "flat" | "pastel" | "antd" | "arco" | "apple" | "ibm";
const activePalette = ref<PaletteKey>("web");

const previewColor = computed(() => hexInput.value || "#000000");

// Quick predefine colors for el-color-picker dropdown
const quickPredefine = [
  "#FF0000", "#FF7F00", "#FFFF00", "#00FF00", "#00FFFF",
  "#0000FF", "#8B00FF", "#FF1493", "#000000", "#FFFFFF",
];

// ─── Color Conversion Functions ──────────────────────────

function hexToRgb(hex: string): [number, number, number] | null {
  const clean = hex.replace("#", "");
  // support 3-char shorthand: #F0A -> #FF00AA
  if (/^[0-9a-f]{3}$/i.test(clean)) {
    return [
      parseInt(clean[0] + clean[0], 16),
      parseInt(clean[1] + clean[1], 16),
      parseInt(clean[2] + clean[2], 16),
    ];
  }
  const m = clean.match(/^([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!m) return null;
  return [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)];
}

function rgbToHex(r: number, g: number, b: number): string {
  return "#" + [r, g, b]
    .map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase();
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
  return [
    Math.round(hue2rgb(p, q, h + 1 / 3) * 255),
    Math.round(hue2rgb(p, q, h) * 255),
    Math.round(hue2rgb(p, q, h - 1 / 3) * 255),
  ];
}

function rgbToHsv(r: number, g: number, b: number): [number, number, number] {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const d = max - min;
  const v = max;
  const s = max === 0 ? 0 : d / max;
  let h = 0;
  if (max !== min) {
    if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
    else if (max === g) h = ((b - r) / d + 2) / 6;
    else h = ((r - g) / d + 4) / 6;
  }
  return [Math.round(h * 360), Math.round(s * 100), Math.round(v * 100)];
}

function hsvToRgb(h: number, s: number, v: number): [number, number, number] {
  h /= 360; s /= 100; v /= 100;
  const i = Math.floor(h * 6);
  const f = h * 6 - i;
  const p = v * (1 - s);
  const q = v * (1 - f * s);
  const t = v * (1 - (1 - f) * s);
  let r = 0, g = 0, b = 0;
  switch (i % 6) {
    case 0: r = v; g = t; b = p; break;
    case 1: r = q; g = v; b = p; break;
    case 2: r = p; g = v; b = t; break;
    case 3: r = p; g = q; b = v; break;
    case 4: r = t; g = p; b = v; break;
    case 5: r = v; g = p; b = q; break;
  }
  return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255)];
}

function rgbToHwb(r: number, g: number, b: number): [number, number, number] {
  const [h] = rgbToHsv(r, g, b);
  const w = Math.min(r, g, b) / 255;
  const bl = 1 - Math.max(r, g, b) / 255;
  return [h, Math.round(w * 100), Math.round(bl * 100)];
}

function hwbToRgb(h: number, w: number, b: number): [number, number, number] {
  w /= 100; b /= 100;
  if (w + b >= 1) {
    const gray = Math.round((w / (w + b)) * 255);
    return [gray, gray, gray];
  }
  const [r0, g0, b0] = hslToRgb(h, 100, 50);
  const f = (v: number) => Math.round((v / 255) * (1 - w - b) * 255 + w * 255);
  return [f(r0), f(g0), f(b0)];
}

function rgbToCmyk(r: number, g: number, b: number): [number, number, number, number] {
  const r1 = r / 255, g1 = g / 255, b1 = b / 255;
  const k = 1 - Math.max(r1, g1, b1);
  if (k === 1) return [0, 0, 0, 100];
  const c = (1 - r1 - k) / (1 - k);
  const m = (1 - g1 - k) / (1 - k);
  const y = (1 - b1 - k) / (1 - k);
  return [Math.round(c * 100), Math.round(m * 100), Math.round(y * 100), Math.round(k * 100)];
}

function cmykToRgb(c: number, m: number, y: number, k: number): [number, number, number] {
  c /= 100; m /= 100; y /= 100; k /= 100;
  return [
    Math.round(255 * (1 - c) * (1 - k)),
    Math.round(255 * (1 - m) * (1 - k)),
    Math.round(255 * (1 - y) * (1 - k)),
  ];
}

// ─── Sync: Update all formats from RGB ───────────────────

function syncFromRgb(r: number, g: number, b: number) {
  hexInput.value = rgbToHex(r, g, b);
  rInput.value = String(r);
  gInput.value = String(g);
  bInput.value = String(b);
  const [h, s, l] = rgbToHsl(r, g, b);
  hInput.value = String(h);
  sInput.value = String(s);
  lInput.value = String(l);
  const [hv, sv, vv] = rgbToHsv(r, g, b);
  hvInput.value = String(hv);
  svInput.value = String(sv);
  vvInput.value = String(vv);
  const [hh, hw, hb] = rgbToHwb(r, g, b);
  hwbHInput.value = String(hh);
  hwbWInput.value = String(hw);
  hwbBInput.value = String(hb);
  const [cc, cm, cy, ck] = rgbToCmyk(r, g, b);
  cmykC.value = String(cc);
  cmykM.value = String(cm);
  cmykY.value = String(cy);
  cmykK.value = String(ck);
  pickerColor.value = hexInput.value;
}

// ─── Watchers ────────────────────────────────────────────

watch(hexInput, (val) => {
  if (updating) return;
  const rgb = hexToRgb(val);
  if (!rgb) return;
  updating = true;
  syncFromRgb(...rgb);
  hexInput.value = val;
  updating = false;
});

watch([rInput, gInput, bInput], ([r, g, b]) => {
  if (updating) return;
  const rn = parseInt(r), gn = parseInt(g), bn = parseInt(b);
  if ([rn, gn, bn].some(isNaN)) return;
  updating = true;
  syncFromRgb(rn, gn, bn);
  rInput.value = r; gInput.value = g; bInput.value = b;
  updating = false;
});

watch([hInput, sInput, lInput], ([h, s, l]) => {
  if (updating) return;
  const hn = parseInt(h), sn = parseInt(s), ln = parseInt(l);
  if ([hn, sn, ln].some(isNaN)) return;
  updating = true;
  const [r, g, b] = hslToRgb(hn, sn, ln);
  syncFromRgb(r, g, b);
  hInput.value = h; sInput.value = s; lInput.value = l;
  updating = false;
});

watch([hvInput, svInput, vvInput], ([h, s, v]) => {
  if (updating) return;
  const hn = parseInt(h), sn = parseInt(s), vn = parseInt(v);
  if ([hn, sn, vn].some(isNaN)) return;
  updating = true;
  const [r, g, b] = hsvToRgb(hn, sn, vn);
  syncFromRgb(r, g, b);
  hvInput.value = h; svInput.value = s; vvInput.value = v;
  updating = false;
});

watch([hwbHInput, hwbWInput, hwbBInput], ([h, w, bv]) => {
  if (updating) return;
  const hn = parseInt(h), wn = parseInt(w), bn = parseInt(bv);
  if ([hn, wn, bn].some(isNaN)) return;
  updating = true;
  const [r, g, b] = hwbToRgb(hn, wn, bn);
  syncFromRgb(r, g, b);
  hwbHInput.value = h; hwbWInput.value = w; hwbBInput.value = bv;
  updating = false;
});

watch([cmykC, cmykM, cmykY, cmykK], ([c, m, y, k]) => {
  if (updating) return;
  const cn = parseInt(c), mn = parseInt(m), yn = parseInt(y), kn = parseInt(k);
  if ([cn, mn, yn, kn].some(isNaN)) return;
  updating = true;
  const [r, g, b] = cmykToRgb(cn, mn, yn, kn);
  syncFromRgb(r, g, b);
  cmykC.value = c; cmykM.value = m; cmykY.value = y; cmykK.value = k;
  updating = false;
});

// ─── Picker handlers ─────────────────────────────────────

function applyHex(hex: string) {
  // Normalize: strip alpha suffix if present (#RRGGBBAA -> #RRGGBB)
  if (hex.length === 9 && hex.startsWith("#")) hex = hex.slice(0, 7);
  const rgb = hexToRgb(hex);
  if (!rgb) return;
  updating = true;
  syncFromRgb(...rgb);
  updating = false;
}

// Fires in real-time while user drags in the picker panel
function onPickerDrag(val: string | null) {
  if (!val || updating) return;
  applyHex(val);
}

// Fires when user clicks "confirm" in the picker panel
function onPickerConfirm(val: string | null) {
  if (!val || updating) return;
  applyHex(val);
}

// Contrast picker handlers
function onContrastFgDrag(val: string | null) {
  if (val) contrastFg.value = val.length === 9 ? val.slice(0, 7) : val;
}
function onContrastFgConfirm(val: string | null) {
  if (val) contrastFg.value = val.length === 9 ? val.slice(0, 7) : val;
}
function onContrastBgDrag(val: string | null) {
  if (val) contrastBg.value = val.length === 9 ? val.slice(0, 7) : val;
}
function onContrastBgConfirm(val: string | null) {
  if (val) contrastBg.value = val.length === 9 ? val.slice(0, 7) : val;
}

function useCurrentAsContrast() {
  contrastFg.value = hexInput.value;
}

// ─── Apply a color from swatches ─────────────────────────

function applyColor(hex: string) {
  applyHex(hex);
}

// ─── Color Harmony ───────────────────────────────────────

function hslHex(h: number, s: number, l: number): string {
  h = ((h % 360) + 360) % 360;
  const [r, g, b] = hslToRgb(h, s, l);
  return rgbToHex(r, g, b);
}

const harmonySchemes = computed(() => {
  const h = parseInt(hInput.value) || 0;
  const s = parseInt(sInput.value) || 0;
  const l = parseInt(lInput.value) || 50;
  return [
    {
      name: "互补色",
      colors: [hslHex(h, s, l), hslHex(h + 180, s, l)],
    },
    {
      name: "类似色",
      colors: [hslHex(h - 30, s, l), hslHex(h, s, l), hslHex(h + 30, s, l)],
    },
    {
      name: "三角色",
      colors: [hslHex(h, s, l), hslHex(h + 120, s, l), hslHex(h + 240, s, l)],
    },
    {
      name: "分裂互补",
      colors: [hslHex(h, s, l), hslHex(h + 150, s, l), hslHex(h + 210, s, l)],
    },
    {
      name: "矩形配色",
      colors: [hslHex(h, s, l), hslHex(h + 60, s, l), hslHex(h + 180, s, l), hslHex(h + 240, s, l)],
    },
    {
      name: "正方配色",
      colors: [hslHex(h, s, l), hslHex(h + 90, s, l), hslHex(h + 180, s, l), hslHex(h + 270, s, l)],
    },
    {
      name: "单色系",
      colors: [
        hslHex(h, s, Math.max(l - 30, 5)),
        hslHex(h, s, Math.max(l - 15, 10)),
        hslHex(h, s, l),
        hslHex(h, s, Math.min(l + 15, 90)),
        hslHex(h, s, Math.min(l + 30, 95)),
      ],
    },
    {
      name: "暖色渐变",
      colors: [
        hslHex(h, s, l),
        hslHex(h + 15, Math.min(s + 10, 100), l),
        hslHex(h + 30, Math.min(s + 20, 100), l),
        hslHex(h + 45, Math.min(s + 10, 100), l),
        hslHex(h + 60, s, l),
      ],
    },
    {
      name: "饱和度梯度",
      colors: [
        hslHex(h, Math.max(s - 40, 0), l),
        hslHex(h, Math.max(s - 20, 0), l),
        hslHex(h, s, l),
        hslHex(h, Math.min(s + 10, 100), Math.max(l - 10, 10)),
        hslHex(h, Math.min(s + 20, 100), Math.max(l - 20, 10)),
      ],
    },
  ];
});

// ─── Preset Palettes ─────────────────────────────────────

const paletteTabs = [
  { key: "web" as const, name: "Tailwind" },
  { key: "antd" as const, name: "Ant Design" },
  { key: "arco" as const, name: "Arco" },
  { key: "material" as const, name: "Material" },
  { key: "apple" as const, name: "Apple" },
  { key: "ibm" as const, name: "IBM" },
  { key: "flat" as const, name: "Flat UI" },
  { key: "pastel" as const, name: "莫兰迪" },
  { key: "chinese" as const, name: "中国传统色" },
];

const webPalette = [
  { name: "Slate 50", hex: "#F8FAFC" }, { name: "Slate 500", hex: "#64748B" }, { name: "Slate 900", hex: "#0F172A" },
  { name: "Red 500", hex: "#EF4444" }, { name: "Orange 500", hex: "#F97316" }, { name: "Amber 500", hex: "#F59E0B" },
  { name: "Yellow 500", hex: "#EAB308" }, { name: "Lime 500", hex: "#84CC16" }, { name: "Green 500", hex: "#22C55E" },
  { name: "Emerald 500", hex: "#10B981" }, { name: "Teal 500", hex: "#14B8A6" }, { name: "Cyan 500", hex: "#06B6D4" },
  { name: "Sky 500", hex: "#0EA5E9" }, { name: "Blue 500", hex: "#3B82F6" }, { name: "Indigo 500", hex: "#6366F1" },
  { name: "Violet 500", hex: "#8B5CF6" }, { name: "Purple 500", hex: "#A855F7" }, { name: "Fuchsia 500", hex: "#D946EF" },
  { name: "Pink 500", hex: "#EC4899" }, { name: "Rose 500", hex: "#F43F5E" },
];

const antdPalette = [
  { name: "Red", hex: "#F5222D" }, { name: "Volcano", hex: "#FA541C" }, { name: "Orange", hex: "#FA8C16" },
  { name: "Gold", hex: "#FAAD14" }, { name: "Yellow", hex: "#FADB14" }, { name: "Lime", hex: "#A0D911" },
  { name: "Green", hex: "#52C41A" }, { name: "Cyan", hex: "#13C2C2" }, { name: "Blue", hex: "#1677FF" },
  { name: "Geekblue", hex: "#2F54EB" }, { name: "Purple", hex: "#722ED1" }, { name: "Magenta", hex: "#EB2F96" },
  { name: "Blue-1", hex: "#E6F4FF" }, { name: "Blue-3", hex: "#91CAFF" }, { name: "Blue-5", hex: "#4096FF" },
  { name: "Blue-7", hex: "#0958D9" }, { name: "Blue-9", hex: "#002C8C" },
  { name: "Grey-1", hex: "#FAFAFA" }, { name: "Grey-5", hex: "#D9D9D9" }, { name: "Grey-9", hex: "#434343" },
];

const arcoPalette = [
  { name: "Red", hex: "#F53F3F" }, { name: "OrangeRed", hex: "#F77234" }, { name: "Orange", hex: "#FF7D00" },
  { name: "Gold", hex: "#F7BA1E" }, { name: "Yellow", hex: "#FADC19" }, { name: "Lime", hex: "#9FDB1D" },
  { name: "Green", hex: "#00B42A" }, { name: "Cyan", hex: "#14C9C9" }, { name: "Blue", hex: "#3491FA" },
  { name: "ArcoBlue", hex: "#165DFF" }, { name: "Purple", hex: "#722ED1" }, { name: "PinkPurple", hex: "#D91AD9" },
  { name: "Magenta", hex: "#F5319D" }, { name: "Gray", hex: "#C9CDD4" },
  { name: "ArcoBlue-1", hex: "#E8F3FF" }, { name: "ArcoBlue-3", hex: "#94BFFF" },
  { name: "ArcoBlue-7", hex: "#0E42D2" }, { name: "ArcoBlue-9", hex: "#081C70" },
  { name: "Gray-3", hex: "#E5E6EB" }, { name: "Gray-8", hex: "#4E5969" },
];

const materialPalette = [
  { name: "Red", hex: "#F44336" }, { name: "Pink", hex: "#E91E63" }, { name: "Purple", hex: "#9C27B0" },
  { name: "Deep Purple", hex: "#673AB7" }, { name: "Indigo", hex: "#3F51B5" }, { name: "Blue", hex: "#2196F3" },
  { name: "Light Blue", hex: "#03A9F4" }, { name: "Cyan", hex: "#00BCD4" }, { name: "Teal", hex: "#009688" },
  { name: "Green", hex: "#4CAF50" }, { name: "Light Green", hex: "#8BC34A" }, { name: "Lime", hex: "#CDDC39" },
  { name: "Yellow", hex: "#FFEB3B" }, { name: "Amber", hex: "#FFC107" }, { name: "Orange", hex: "#FF9800" },
  { name: "Deep Orange", hex: "#FF5722" }, { name: "Brown", hex: "#795548" }, { name: "Grey", hex: "#9E9E9E" },
  { name: "Blue Grey", hex: "#607D8B" }, { name: "Black", hex: "#000000" },
];

const applePalette = [
  { name: "Blue", hex: "#007AFF" }, { name: "Green", hex: "#34C759" }, { name: "Indigo", hex: "#5856D6" },
  { name: "Orange", hex: "#FF9500" }, { name: "Pink", hex: "#FF2D55" }, { name: "Purple", hex: "#AF52DE" },
  { name: "Red", hex: "#FF3B30" }, { name: "Teal", hex: "#5AC8FA" }, { name: "Yellow", hex: "#FFCC00" },
  { name: "Mint", hex: "#00C7BE" }, { name: "Cyan", hex: "#32ADE6" }, { name: "Brown", hex: "#A2845E" },
  { name: "Gray", hex: "#8E8E93" }, { name: "Gray 2", hex: "#AEAEB2" }, { name: "Gray 3", hex: "#C7C7CC" },
  { name: "Gray 4", hex: "#D1D1D6" }, { name: "Gray 5", hex: "#E5E5EA" }, { name: "Gray 6", hex: "#F2F2F7" },
  { name: "Label", hex: "#3C3C43" }, { name: "Background", hex: "#F2F2F7" },
];

const ibmPalette = [
  { name: "Blue 60", hex: "#0F62FE" }, { name: "Blue 40", hex: "#78A9FF" }, { name: "Blue 80", hex: "#002D9C" },
  { name: "Cyan 50", hex: "#1192E8" }, { name: "Cyan 30", hex: "#82CFFF" }, { name: "Cyan 70", hex: "#003A6D" },
  { name: "Teal 50", hex: "#009D9A" }, { name: "Teal 70", hex: "#005D5D" },
  { name: "Green 50", hex: "#24A148" }, { name: "Green 70", hex: "#0E6027" },
  { name: "Purple 60", hex: "#8A3FFC" }, { name: "Purple 40", hex: "#BE95FF" },
  { name: "Magenta 50", hex: "#EE5396" }, { name: "Magenta 70", hex: "#9F1853" },
  { name: "Red 60", hex: "#DA1E28" }, { name: "Red 40", hex: "#FF8389" },
  { name: "Orange 40", hex: "#FF832B" }, { name: "Yellow 30", hex: "#F1C21B" },
  { name: "Gray 50", hex: "#8D8D8D" }, { name: "Gray 80", hex: "#393939" },
];

const flatPalette = [
  { name: "Turquoise", hex: "#1ABC9C" }, { name: "Emerald", hex: "#2ECC71" }, { name: "Peter River", hex: "#3498DB" },
  { name: "Amethyst", hex: "#9B59B6" }, { name: "Wet Asphalt", hex: "#34495E" }, { name: "Green Sea", hex: "#16A085" },
  { name: "Nephritis", hex: "#27AE60" }, { name: "Belize Hole", hex: "#2980B9" }, { name: "Wisteria", hex: "#8E44AD" },
  { name: "Midnight Blue", hex: "#2C3E50" }, { name: "Sun Flower", hex: "#F1C40F" }, { name: "Carrot", hex: "#E67E22" },
  { name: "Alizarin", hex: "#E74C3C" }, { name: "Clouds", hex: "#ECF0F1" }, { name: "Concrete", hex: "#95A5A6" },
  { name: "Orange", hex: "#F39C12" }, { name: "Pumpkin", hex: "#D35400" }, { name: "Pomegranate", hex: "#C0392B" },
  { name: "Silver", hex: "#BDC3C7" }, { name: "Asbestos", hex: "#7F8C8D" },
];

const pastelPalette = [
  { name: "暮云灰", hex: "#B5B5B5" }, { name: "雾蓝", hex: "#8EA2B0" }, { name: "灰豆绿", hex: "#A2B5A0" },
  { name: "藕粉", hex: "#D4A5A5" }, { name: "燕麦色", hex: "#C8B896" }, { name: "烟紫", hex: "#A89BB5" },
  { name: "青瓷", hex: "#9ABCB0" }, { name: "灰粉", hex: "#C9A9A6" }, { name: "雾绿", hex: "#A0B8A8" },
  { name: "银杏黄", hex: "#D4C48A" }, { name: "枯玫瑰", hex: "#BF8B8B" }, { name: "石灰蓝", hex: "#8CA0A8" },
  { name: "奶咖", hex: "#C4A882" }, { name: "灰丁香", hex: "#B0A0BA" }, { name: "莫兰迪绿", hex: "#8FAF8F" },
  { name: "沙色", hex: "#C6B598" }, { name: "雾灰", hex: "#A8A8A8" }, { name: "暮蓝", hex: "#7E95A5" },
  { name: "暖灰", hex: "#B8AFA0" }, { name: "柿红", hex: "#C08070" },
];

const chinesePalette = [
  { name: "靛青", hex: "#177CB0" }, { name: "朱砂", hex: "#FF461F" }, { name: "月白", hex: "#D6ECF0" },
  { name: "竹青", hex: "#789262" }, { name: "藕荷", hex: "#E4C6D0" }, { name: "鸦青", hex: "#424C50" },
  { name: "胭脂", hex: "#9D2933" }, { name: "黛", hex: "#4A4266" }, { name: "缃色", hex: "#F0C239" },
  { name: "秋香", hex: "#D9B611" }, { name: "松花", hex: "#BCE672" }, { name: "石青", hex: "#1685A9" },
  { name: "赤金", hex: "#F2BE45" }, { name: "雪白", hex: "#F0FCFF" }, { name: "玄色", hex: "#622A1D" },
  { name: "绛紫", hex: "#8C4356" }, { name: "青白", hex: "#C0EBD7" }, { name: "丁香", hex: "#CCA4E3" },
  { name: "琥珀", hex: "#CA6924" }, { name: "苍色", hex: "#75878A" },
];

const currentPalette = computed(() => {
  const map: Record<PaletteKey, typeof webPalette> = {
    web: webPalette,
    antd: antdPalette,
    arco: arcoPalette,
    material: materialPalette,
    apple: applePalette,
    ibm: ibmPalette,
    flat: flatPalette,
    pastel: pastelPalette,
    chinese: chinesePalette,
  };
  return map[activePalette.value];
});

// ─── WCAG Contrast ───────────────────────────────────────

function relativeLuminance(hex: string): number {
  const rgb = hexToRgb(hex);
  if (!rgb) return 0;
  const [r, g, b] = rgb.map((v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

const contrastRatio = computed(() => {
  const l1 = relativeLuminance(contrastFg.value);
  const l2 = relativeLuminance(contrastBg.value);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  const ratio = (lighter + 0.05) / (darker + 0.05);
  return Math.round(ratio * 100) / 100;
});

const wcagAA = computed(() => contrastRatio.value >= 4.5);
const wcagAAA = computed(() => contrastRatio.value >= 7);

// ─── Copy ────────────────────────────────────────────────

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
.color-panel {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

/* ─── Top Section: Picker + Inputs ─── */
.color-top-section {
  display: flex;
  gap: 20px;
  align-items: flex-start;
}

.color-picker-area {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.color-picker-area :deep(.el-color-picker__trigger) {
  width: 48px;
  height: 48px;
  border-radius: var(--lc-radius-sm);
  border-color: var(--lc-border);
}

.color-swatch {
  width: 140px;
  height: 100px;
  border-radius: var(--lc-radius-md);
  border: 1px solid var(--lc-border);
  transition: background var(--lc-duration) var(--lc-ease);
  cursor: pointer;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding-bottom: 6px;
  position: relative;
}

.color-swatch:hover {
  border-color: var(--lc-border-hover);
}

.swatch-hex {
  font-size: 11px;
  font-family: var(--lc-font-mono);
  font-weight: 600;
  color: #fff;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
  pointer-events: none;
}

.color-inputs {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}

.color-input-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.color-label {
  width: 42px;
  flex-shrink: 0;
  font-size: 12px;
  font-weight: 600;
  color: var(--lc-text-secondary);
  font-family: var(--lc-font-mono);
  text-align: right;
}

.color-multi .el-input {
  flex: 1;
  min-width: 0;
}

.cmyk-row .el-input {
  flex: 1;
  min-width: 0;
}

/* ─── Copy Buttons ─── */
.color-copy-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

/* ─── Section ─── */
.color-section {
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  padding: 16px;
}

.color-section-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--lc-text-secondary);
  margin-bottom: 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.palette-tabs {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.palette-tab {
  font-size: 12px;
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
  color: var(--lc-text-muted);
  transition: color var(--lc-duration) var(--lc-ease), background var(--lc-duration) var(--lc-ease);
  user-select: none;
}

.palette-tab:hover {
  color: var(--lc-text);
  background: rgba(255, 255, 255, 0.04);
}

.palette-tab.active {
  color: var(--lc-accent-light);
  background: var(--lc-accent-dim);
}

/* ─── Harmony ─── */
.harmony-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.harmony-row:last-child {
  margin-bottom: 0;
}

.harmony-label {
  width: 72px;
  flex-shrink: 0;
  font-size: 12px;
  color: var(--lc-text-secondary);
}

.harmony-swatches {
  display: flex;
  gap: 4px;
  flex: 1;
}

.harmony-swatch {
  height: 28px;
  flex: 1;
  border-radius: 4px;
  border: 1px solid var(--lc-border);
  cursor: pointer;
  transition: transform 0.15s ease, box-shadow 0.15s ease;
}

.harmony-swatch:hover {
  transform: scale(1.08);
  box-shadow: var(--lc-shadow-md);
}

/* ─── Palette ─── */
.palette-grid {
  display: grid;
  grid-template-columns: repeat(10, 1fr);
  gap: 6px;
}

.palette-swatch {
  aspect-ratio: 1;
  border-radius: 4px;
  border: 1px solid var(--lc-border);
  cursor: pointer;
  transition: transform 0.15s ease, box-shadow 0.15s ease;
}

.palette-swatch:hover {
  transform: scale(1.15);
  box-shadow: var(--lc-shadow-md);
  z-index: 1;
}

/* ─── Contrast ─── */
.contrast-row {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}

.contrast-input-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

.contrast-input-group .el-input {
  width: 100px;
}

.contrast-input-group :deep(.el-color-picker__trigger) {
  width: 28px;
  height: 28px;
  border-radius: 4px;
}

.contrast-label {
  font-size: 12px;
  color: var(--lc-text-secondary);
  flex-shrink: 0;
}

.contrast-result {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-left: auto;
}

.contrast-ratio {
  font-size: 20px;
  font-weight: 700;
  font-family: var(--lc-font-mono);
  color: var(--lc-text);
}

.contrast-badges {
  display: flex;
  gap: 6px;
}

.contrast-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 4px;
}

.contrast-badge.pass {
  background: rgba(52, 211, 153, 0.15);
  color: var(--lc-success);
}

.contrast-badge.fail {
  background: rgba(248, 113, 113, 0.15);
  color: var(--lc-danger);
}

.contrast-preview {
  margin-top: 10px;
  padding: 12px 16px;
  border-radius: var(--lc-radius-sm);
  border: 1px solid var(--lc-border);
  font-size: 14px;
  line-height: 1.6;
}
</style>
