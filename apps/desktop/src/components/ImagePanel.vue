<template>
  <div class="panel-grid">
    <div class="panel-grid-full image-mode-row">
      <div class="field-label image-mode-label">处理模式</div>
      <el-radio-group v-model="mode" size="small" aria-label="处理模式">
        <el-radio-button value="convert">转换</el-radio-button>
        <el-radio-button value="compress">压缩</el-radio-button>
      </el-radio-group>
    </div>

    <div class="panel-grid-full image-mode-hint">
      {{
        mode === "convert" ? "可修改格式、尺寸和裁剪区域" : "保持源格式和像素尺寸，只调整编码参数"
      }}
    </div>

    <div class="panel-grid-full image-input-row">
      <el-button @click="pickInputFile">选择图片</el-button>
      <el-input
        v-model="imageInputPath"
        placeholder="图片路径（支持 PNG/JPEG/WebP/AVIF/BMP/GIF/TIFF）"
        @change="onInputPathChange"
      />
    </div>

    <div v-if="previewSrc" class="panel-grid-full image-preview-row">
      <div class="image-preview-box">
        <img :src="previewSrc" alt="源图片预览" class="image-preview-img" />
      </div>
      <div v-if="imageInfo" class="image-info-box">
        <div class="image-info-item">
          <span class="image-info-label">尺寸</span>
          <span class="image-info-value">{{ imageInfo.width }} x {{ imageInfo.height }}</span>
        </div>
        <div class="image-info-item">
          <span class="image-info-label">文件大小</span>
          <span class="image-info-value">{{ formatSize(imageInfo.size) }}</span>
        </div>
        <div class="image-info-item">
          <span class="image-info-label">格式</span>
          <span class="image-info-value">{{ imageInfo.format }}</span>
        </div>
      </div>
    </div>

    <template v-if="mode === 'convert'">
      <div>
        <div class="field-label">输出格式</div>
        <el-select v-model="imageFormat" style="width: 100%" @change="updateOutputPath">
          <el-option label="PNG" value="png" />
          <el-option label="JPEG" value="jpeg" />
          <el-option label="WebP" value="webp" />
          <el-option label="AVIF" value="avif" />
        </el-select>
      </div>
    </template>
    <div v-else>
      <div class="field-label">压缩格式</div>
      <div class="image-fixed-format">{{ compressionFormatLabel }}</div>
    </div>

    <div v-if="activeEncoderKind === 'quality'">
      <div class="field-label">质量 (1-100)</div>
      <el-slider
        v-model="imageQuality"
        :min="1"
        :max="100"
        show-input
        :show-input-controls="false"
        input-size="small"
      />
    </div>
    <div v-else-if="activeEncoderKind === 'png'">
      <div class="field-label">无损压缩级别 (1-9)</div>
      <el-slider
        v-model="imageCompressionLevel"
        :min="1"
        :max="9"
        show-input
        :show-input-controls="false"
        input-size="small"
      />
    </div>
    <div v-else class="format-hint">
      {{ formatQualityHint }}
    </div>

    <template v-if="mode === 'convert'">
      <div>
        <div class="field-label-row">
          <div class="field-label">宽度 (px)</div>
          <el-checkbox v-model="keepAspectRatio">锁定宽高比</el-checkbox>
        </div>
        <el-input-number
          :model-value="imageWidth"
          :min="0"
          :max="10000"
          controls-position="right"
          placeholder="0 = 保持原始"
          style="width: 100%"
          @update:model-value="updateImageWidth"
        />
      </div>
      <div>
        <div class="field-label">高度 (px)</div>
        <el-input-number
          :model-value="imageHeight"
          :min="0"
          :max="10000"
          controls-position="right"
          placeholder="0 = 保持原始"
          style="width: 100%"
          @update:model-value="updateImageHeight"
        />
        <div v-if="outputSizeHint" class="field-help">预计输出：{{ outputSizeHint }}</div>
      </div>

      <div class="panel-grid-full">
        <el-collapse>
          <el-collapse-item title="裁剪设置（可选）" name="crop">
            <div class="crop-grid">
              <div>
                <div class="field-label">起始 X</div>
                <el-input-number
                  v-model="cropX"
                  :min="0"
                  :max="10000"
                  controls-position="right"
                  style="width: 100%"
                />
              </div>
              <div>
                <div class="field-label">起始 Y</div>
                <el-input-number
                  v-model="cropY"
                  :min="0"
                  :max="10000"
                  controls-position="right"
                  style="width: 100%"
                />
              </div>
              <div>
                <div class="field-label">裁剪宽度</div>
                <el-input-number
                  v-model="cropWidth"
                  :min="0"
                  :max="10000"
                  controls-position="right"
                  style="width: 100%"
                />
              </div>
              <div>
                <div class="field-label">裁剪高度</div>
                <el-input-number
                  v-model="cropHeight"
                  :min="0"
                  :max="10000"
                  controls-position="right"
                  style="width: 100%"
                />
              </div>
            </div>
            <div v-if="cropError" class="field-error" role="alert">{{ cropError }}</div>
            <div v-else-if="cropEnabled" class="field-help">
              裁剪区域：{{ cropWidth }} x {{ cropHeight }} px
            </div>
          </el-collapse-item>
        </el-collapse>
      </div>
    </template>

    <div class="panel-grid-full image-output-row">
      <el-input v-model="imageOutputPath" placeholder="输出路径（自动生成，可手动修改）" />
      <el-button @click="pickOutputDir">选择目录</el-button>
    </div>

    <div v-if="compressValidationError" class="panel-grid-full field-error" role="alert">
      {{ compressValidationError }}
    </div>

    <div class="panel-grid-full image-action-row">
      <el-space>
        <el-button
          type="primary"
          :loading="processing"
          :disabled="mode === 'compress' && !!compressValidationError"
          @click="processImage"
        >
          {{ mode === "compress" ? "压缩图片" : "转换图片" }}
        </el-button>
        <el-button @click="resetForm">重置</el-button>
      </el-space>
    </div>

    <div v-if="imageResult" class="panel-grid-full image-result-card">
      <div v-if="resultPreviewSrc" class="image-result-preview-box">
        <img :src="resultPreviewSrc" alt="输出图片预览" class="image-result-preview-img" />
      </div>
      <div class="image-info-item">
        <span class="image-info-label">输出路径</span>
        <span class="image-info-value image-path-value">{{ imageResult.outputPath }}</span>
      </div>
      <div class="image-info-item">
        <span class="image-info-label">输出尺寸</span>
        <span class="image-info-value">{{ imageResult.width }} x {{ imageResult.height }}</span>
      </div>
      <template v-if="mode === 'compress' && compressResult">
        <div class="image-info-item">
          <span class="image-info-label">原图大小</span>
          <span class="image-info-value">{{ formatSize(compressResult.inputSize) }}</span>
        </div>
        <div class="image-info-item">
          <span class="image-info-label">输出大小</span>
          <span class="image-info-value">{{ formatSize(compressResult.size) }}</span>
        </div>
        <div class="image-info-item">
          <span class="image-info-label">体积变化</span>
          <span class="image-info-value">
            {{ compressResult.savedBytes >= 0 ? "节省 " : "增加 "
            }}{{ formatSize(Math.abs(compressResult.savedBytes)) }}
          </span>
        </div>
        <div class="image-info-item">
          <span class="image-info-label">压缩后占比</span>
          <span class="image-info-value">{{ compressResult.compressionRatio.toFixed(1) }}%</span>
        </div>
        <div v-if="compressResult.savedBytes <= 0" class="image-result-warning" role="status">
          体积未减少，输出文件已生成，请按实际效果决定是否使用。
        </div>
      </template>
      <div v-else class="image-info-item">
        <span class="image-info-label">输出大小</span>
        <span class="image-info-value">{{ formatSize(imageResult.size) }}</span>
      </div>
      <div class="image-result-actions">
        <el-button size="small" @click="revealOutput(imageResult.outputPath)">
          在文件夹中显示
        </el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../bridge/tauri";

type ImageMode = "convert" | "compress";

interface ImageInfo {
  width: number;
  height: number;
  size: number;
  format: string;
}

interface ConvertResult {
  outputPath: string;
  width: number;
  height: number;
  size: number;
}

interface CompressResult extends ConvertResult {
  inputSize: number;
  savedBytes: number;
  compressionRatio: number;
}

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "avif", "bmp", "gif", "tiff", "tif"];
const COMPRESSIBLE_FORMATS = ["png", "jpeg", "webp", "avif"];
const MAX_COMPRESS_INPUT_BYTES = 100 * 1024 * 1024;
const MAX_COMPRESS_PIXELS = 50_000_000;

const mode = ref<ImageMode>("convert");
const imageInputPath = ref("");
const imageOutputPath = ref("");
const imageFormat = ref("png");
const imageWidth = ref(0);
const imageHeight = ref(0);
const cropX = ref(0);
const cropY = ref(0);
const cropWidth = ref(0);
const cropHeight = ref(0);
const imageQuality = ref(80);
const imageCompressionLevel = ref(6);
const keepAspectRatio = ref(true);
const processing = ref(false);
const previewSrc = ref("");
const resultPreviewSrc = ref("");
const imageInfo = ref<ImageInfo | null>(null);
const convertResult = ref<ConvertResult | null>(null);
const compressResult = ref<CompressResult | null>(null);

const cropEnabled = computed(
  () => cropX.value > 0 || cropY.value > 0 || cropWidth.value > 0 || cropHeight.value > 0,
);
const cropError = computed(() => {
  if (!cropEnabled.value) return "";
  if (!imageInfo.value) return "请先选择有效图片，再设置裁剪区域";
  if (cropWidth.value <= 0 || cropHeight.value <= 0) return "裁剪宽度和高度必须同时大于 0";
  if (
    cropX.value + cropWidth.value > imageInfo.value.width ||
    cropY.value + cropHeight.value > imageInfo.value.height
  ) {
    return `裁剪区域超出原图边界（${imageInfo.value.width} x ${imageInfo.value.height}）`;
  }
  return "";
});
const resizeBase = computed(() => {
  if (cropEnabled.value && !cropError.value)
    return { width: cropWidth.value, height: cropHeight.value };
  return imageInfo.value ? { width: imageInfo.value.width, height: imageInfo.value.height } : null;
});
const outputSizeHint = computed(() => {
  if (!resizeBase.value) return "";
  if (imageWidth.value <= 0 && imageHeight.value <= 0) {
    return `${resizeBase.value.width} x ${resizeBase.value.height} px（保持原始）`;
  }
  if (imageWidth.value > 0 && imageHeight.value > 0)
    return `${imageWidth.value} x ${imageHeight.value} px`;
  if (imageWidth.value > 0) {
    return `${imageWidth.value} x ${scaleDimension(imageWidth.value, resizeBase.value.width, resizeBase.value.height)} px`;
  }
  return `${scaleDimension(imageHeight.value, resizeBase.value.height, resizeBase.value.width)} x ${imageHeight.value} px`;
});

const compressionFormat = computed(() => normalizeFormat(imageInfo.value?.format ?? ""));
const compressionFormatLabel = computed(() => {
  const labels: Record<string, string> = {
    png: "PNG",
    jpeg: "JPEG",
    webp: "WebP",
    avif: "AVIF",
  };
  return labels[compressionFormat.value] ?? imageInfo.value?.format ?? "未选择图片";
});
const activeFormat = computed(() =>
  mode.value === "compress" ? compressionFormat.value : imageFormat.value,
);
const activeEncoderKind = computed(() => {
  if (activeFormat.value === "jpeg" || activeFormat.value === "avif") return "quality";
  if (activeFormat.value === "png") return "png";
  return "none";
});
const formatQualityHint = computed(() => {
  if (activeFormat.value === "webp") return "WebP 当前仅支持无损编码。";
  if (mode.value === "compress" && !COMPRESSIBLE_FORMATS.includes(activeFormat.value)) {
    return "压缩仅支持 PNG、JPEG、WebP、AVIF。";
  }
  return "当前格式不提供额外编码参数。";
});
const compressValidationError = computed(() => {
  if (mode.value !== "compress" || !imageInfo.value) return "";
  if (!COMPRESSIBLE_FORMATS.includes(compressionFormat.value)) {
    return "压缩仅支持 PNG、JPEG、WebP、AVIF，BMP/GIF/TIFF 不支持。";
  }
  if (imageInfo.value.size > MAX_COMPRESS_INPUT_BYTES) return "压缩输入图片不能超过 100 MB。";
  if (imageInfo.value.width * imageInfo.value.height > MAX_COMPRESS_PIXELS) {
    return "压缩输入图片的像素数不能超过 50 MP。";
  }
  return "";
});
const imageResult = computed<ConvertResult | CompressResult | null>(() =>
  mode.value === "compress" ? compressResult.value : convertResult.value,
);

function normalizeFormat(format: string): string {
  const value = format.toLowerCase();
  if (value === "jpg" || value === "jpeg") return "jpeg";
  return value;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function formatExtension(format: string): string {
  return format === "jpeg" ? "jpg" : format;
}

function inputExtension(): string {
  const path = imageInputPath.value.trim();
  const separator = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  const dot = path.lastIndexOf(".");
  if (dot <= separator || dot < 0) return "png";
  return formatExtension(normalizeFormat(path.slice(dot + 1)));
}

function outputExtension(): string {
  if (mode.value === "compress") {
    return formatExtension(compressionFormat.value || inputExtension());
  }
  return formatExtension(imageFormat.value);
}

function outputSuffix(): string {
  return mode.value === "compress" ? "compressed" : "converted";
}

function replaceExtension(path: string): string {
  const separator = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  const dot = path.lastIndexOf(".");
  const base = dot > separator ? path.slice(0, dot) : path;
  return `${base}_${outputSuffix()}.${outputExtension()}`;
}

function updateOutputPath() {
  if (!imageInputPath.value) return;
  imageOutputPath.value = replaceExtension(imageInputPath.value);
}

async function loadImageInfo(path: string) {
  try {
    const data = (await invokeToolByChannel("tool:image:info", { inputPath: path })) as ImageInfo;
    imageInfo.value = data;
    imageWidth.value = 0;
    imageHeight.value = 0;
    updateOutputPath();
  } catch (error) {
    imageInfo.value = null;
    previewSrc.value = "";
    throw error;
  }
}

function clearResults() {
  convertResult.value = null;
  compressResult.value = null;
  resultPreviewSrc.value = "";
}

async function onInputPathChange() {
  const path = imageInputPath.value.trim();
  clearResults();
  if (!path) {
    previewSrc.value = "";
    imageInfo.value = null;
    imageOutputPath.value = "";
    return;
  }
  previewSrc.value = convertFileSrc(path);
  updateOutputPath();
  try {
    await loadImageInfo(path);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function scaleDimension(value: number, sourceDimension: number, targetDimension: number): number {
  if (value <= 0 || sourceDimension <= 0) return 0;
  return Math.max(1, Math.round((value * targetDimension) / sourceDimension));
}

function updateImageWidth(value: number | undefined) {
  imageWidth.value = value ?? 0;
  if (!keepAspectRatio.value || !resizeBase.value) return;
  imageHeight.value =
    imageWidth.value > 0
      ? scaleDimension(imageWidth.value, resizeBase.value.width, resizeBase.value.height)
      : 0;
}

function updateImageHeight(value: number | undefined) {
  imageHeight.value = value ?? 0;
  if (!keepAspectRatio.value || !resizeBase.value) return;
  imageWidth.value =
    imageHeight.value > 0
      ? scaleDimension(imageHeight.value, resizeBase.value.height, resizeBase.value.width)
      : 0;
}

watch([cropWidth, cropHeight], () => {
  if (!keepAspectRatio.value || !resizeBase.value || imageWidth.value <= 0) return;
  imageHeight.value = scaleDimension(
    imageWidth.value,
    resizeBase.value.width,
    resizeBase.value.height,
  );
});

watch(mode, () => {
  clearResults();
  updateOutputPath();
});

async function pickInputFile() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: IMAGE_EXTENSIONS }],
    });
    if (!selected) return;
    const path = typeof selected === "string" ? selected : selected.path;
    if (!path) return;
    imageInputPath.value = path;
    previewSrc.value = convertFileSrc(path);
    clearResults();
    updateOutputPath();
    await loadImageInfo(path);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function pickOutputDir() {
  try {
    const selected = await open({ directory: true });
    if (!selected) return;
    const dir = typeof selected === "string" ? selected : selected.path;
    if (!dir) return;
    const inputName = imageInputPath.value.split(/[\\/]/).pop() || "output";
    const baseName = inputName.replace(/\.[^.]+$/, "");
    imageOutputPath.value = `${dir}\\${baseName}_${outputSuffix()}.${outputExtension()}`;
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function buildConvertPayload(overwrite: boolean): Record<string, unknown> {
  const payload: Record<string, unknown> = {
    inputPath: imageInputPath.value.trim(),
    outputPath: imageOutputPath.value.trim(),
    format: imageFormat.value,
    overwrite,
  };
  if (imageFormat.value === "jpeg" || imageFormat.value === "avif") {
    payload.quality = imageQuality.value;
  }
  if (imageFormat.value === "png") payload.compressionLevel = imageCompressionLevel.value;
  if (imageWidth.value > 0) payload.width = imageWidth.value;
  if (imageHeight.value > 0) payload.height = imageHeight.value;
  if (cropEnabled.value) {
    payload.cropX = cropX.value;
    payload.cropY = cropY.value;
    payload.cropWidth = cropWidth.value;
    payload.cropHeight = cropHeight.value;
  }
  return payload;
}

function buildCompressPayload(overwrite: boolean): Record<string, unknown> {
  const payload: Record<string, unknown> = {
    inputPath: imageInputPath.value.trim(),
    outputPath: imageOutputPath.value.trim(),
    overwrite,
  };
  if (compressionFormat.value === "jpeg" || compressionFormat.value === "avif") {
    payload.quality = imageQuality.value;
  }
  if (compressionFormat.value === "png") payload.compressionLevel = imageCompressionLevel.value;
  return payload;
}

function setResultPreview(path: string) {
  resultPreviewSrc.value = `${convertFileSrc(path)}?v=${Date.now()}`;
}

async function runConversion(overwrite: boolean) {
  const data = (await invokeToolByChannel(
    "tool:image:convert",
    buildConvertPayload(overwrite),
  )) as ConvertResult;
  convertResult.value = data;
  setResultPreview(data.outputPath);
  ElMessage.success("转换完成");
}

async function runCompression(overwrite: boolean) {
  const data = (await invokeToolByChannel(
    "tool:image:compress",
    buildCompressPayload(overwrite),
  )) as CompressResult;
  compressResult.value = data;
  setResultPreview(data.outputPath);
  ElMessage.success("压缩完成");
}

async function runImageAction(overwrite: boolean) {
  if (mode.value === "compress") {
    await runCompression(overwrite);
  } else {
    await runConversion(overwrite);
  }
}

async function processImage() {
  if (!imageInputPath.value.trim()) {
    ElMessage.warning("请先选择图片");
    return;
  }
  if (!imageOutputPath.value.trim()) {
    ElMessage.warning("请指定输出路径");
    return;
  }
  if (mode.value === "compress" && compressValidationError.value) {
    ElMessage.warning(compressValidationError.value);
    return;
  }
  if (mode.value === "convert" && cropError.value) {
    ElMessage.warning(cropError.value);
    return;
  }

  processing.value = true;
  clearResults();
  try {
    await runImageAction(false);
  } catch (error) {
    const message = (error as Error).message;
    if (!message.includes("输出文件已存在")) {
      ElMessage.error(message);
      return;
    }
    try {
      await ElMessageBox.confirm(
        `输出文件已存在，是否覆盖？\n${imageOutputPath.value}`,
        "确认覆盖",
        { type: "warning", confirmButtonText: "覆盖", cancelButtonText: "取消" },
      );
      await runImageAction(true);
    } catch (confirmError) {
      if (confirmError !== "cancel" && confirmError !== "close") {
        ElMessage.error((confirmError as Error).message);
      }
    }
  } finally {
    processing.value = false;
  }
}

async function revealOutput(path: string) {
  try {
    await invokeToolByChannel("tool:system:reveal-in-folder", { path });
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function resetForm() {
  mode.value = "convert";
  imageInputPath.value = "";
  imageOutputPath.value = "";
  imageFormat.value = "png";
  imageWidth.value = 0;
  imageHeight.value = 0;
  cropX.value = 0;
  cropY.value = 0;
  cropWidth.value = 0;
  cropHeight.value = 0;
  imageQuality.value = 80;
  imageCompressionLevel.value = 6;
  keepAspectRatio.value = true;
  previewSrc.value = "";
  imageInfo.value = null;
  clearResults();
}
</script>

<style scoped>
.field-label {
  margin-bottom: 6px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

.field-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.field-help,
.format-hint,
.image-mode-hint {
  margin-top: 6px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--el-text-color-secondary);
}

.image-mode-row,
.image-input-row,
.image-output-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.image-mode-label {
  margin: 0;
  white-space: nowrap;
}

.image-mode-hint {
  margin-top: -4px;
}

.image-input-row :deep(.el-input),
.image-output-row :deep(.el-input) {
  flex: 1;
  min-width: 0;
}

.format-hint {
  display: flex;
  align-items: center;
  min-height: 32px;
  margin-top: 22px;
  padding: 0 10px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-1);
}

.image-fixed-format {
  display: flex;
  align-items: center;
  min-height: 32px;
  padding: 0 10px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  color: var(--lc-text);
  background: var(--lc-surface-1);
}

.crop-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.field-error {
  margin-top: 10px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--el-color-danger);
}

.image-action-row {
  margin-top: 2px;
}

.image-preview-row {
  display: flex;
  align-items: flex-start;
  gap: 16px;
}

.image-preview-box,
.image-result-preview-box {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 200px;
  min-height: 150px;
  max-width: 300px;
  padding: 8px;
  border: 1px dashed var(--lc-border-hover);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-2);
}

.image-preview-img,
.image-result-preview-img {
  max-width: 100%;
  max-height: 250px;
  object-fit: contain;
  border-radius: 4px;
}

.image-info-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 4px;
}

.image-info-item {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.image-info-label {
  min-width: 60px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
}

.image-info-value {
  min-width: 0;
  font-size: 13px;
  color: var(--lc-text);
}

.image-path-value {
  overflow-wrap: anywhere;
}

.image-result-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px 16px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-1);
}

.image-result-preview-box {
  width: 100%;
  max-width: none;
  min-height: 120px;
}

.image-result-warning {
  padding: 8px 10px;
  border-left: 3px solid var(--el-color-warning);
  color: var(--el-color-warning-dark-2);
  background: var(--el-color-warning-light-9);
  font-size: 12px;
  line-height: 1.5;
}

.image-result-actions {
  margin-top: 4px;
}

@media (max-width: 720px) {
  .image-mode-row,
  .image-input-row,
  .image-output-row {
    align-items: stretch;
    flex-direction: column;
  }

  .image-mode-row {
    align-items: flex-start;
  }

  .image-preview-row {
    flex-direction: column;
  }

  .image-preview-box,
  .image-result-preview-box {
    width: 100%;
    min-width: 0;
    max-width: none;
  }

  .format-hint {
    margin-top: 0;
  }

  .crop-grid {
    grid-template-columns: 1fr;
  }
}
</style>
