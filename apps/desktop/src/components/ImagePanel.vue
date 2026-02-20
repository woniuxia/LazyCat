<template>
  <div class="panel-grid">
    <!-- Input file -->
    <div class="panel-grid-full" style="display:flex;gap:8px;align-items:center;">
      <el-button @click="pickInputFile">选择图片</el-button>
      <el-input v-model="imageInputPath" placeholder="图片路径（支持 PNG/JPEG/WebP/AVIF/BMP/GIF/TIFF）" style="flex:1;" @change="onInputPathChange" />
    </div>

    <!-- Image preview + info -->
    <div v-if="previewSrc" class="panel-grid-full image-preview-row">
      <div class="image-preview-box">
        <img :src="previewSrc" alt="preview" class="image-preview-img" />
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

    <!-- Output format -->
    <div>
      <div class="field-label">输出格式</div>
      <el-select v-model="imageFormat" style="width:100%;" @change="updateOutputPath">
        <el-option label="PNG" value="png" />
        <el-option label="JPEG" value="jpeg" />
        <el-option label="WebP" value="webp" />
        <el-option label="AVIF" value="avif" />
      </el-select>
    </div>

    <!-- Quality -->
    <div>
      <div class="field-label">质量 (1-100)</div>
      <el-slider v-model="imageQuality" :min="1" :max="100" show-input :show-input-controls="false" input-size="small" />
    </div>

    <!-- Resize -->
    <div>
      <div class="field-label">宽度 (px)</div>
      <el-input-number v-model="imageWidth" :min="0" :max="10000" controls-position="right" placeholder="0 = 保持原始" style="width:100%;" />
    </div>
    <div>
      <div class="field-label">高度 (px)</div>
      <el-input-number v-model="imageHeight" :min="0" :max="10000" controls-position="right" placeholder="0 = 保持原始" style="width:100%;" />
    </div>

    <!-- Crop -->
    <div class="panel-grid-full">
      <el-collapse>
        <el-collapse-item title="裁剪设置（可选）" name="crop">
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;">
            <div>
              <div class="field-label">起始 X</div>
              <el-input-number v-model="cropX" :min="0" :max="10000" controls-position="right" style="width:100%;" />
            </div>
            <div>
              <div class="field-label">起始 Y</div>
              <el-input-number v-model="cropY" :min="0" :max="10000" controls-position="right" style="width:100%;" />
            </div>
            <div>
              <div class="field-label">裁剪宽度</div>
              <el-input-number v-model="cropWidth" :min="0" :max="10000" controls-position="right" style="width:100%;" />
            </div>
            <div>
              <div class="field-label">裁剪高度</div>
              <el-input-number v-model="cropHeight" :min="0" :max="10000" controls-position="right" style="width:100%;" />
            </div>
          </div>
        </el-collapse-item>
      </el-collapse>
    </div>

    <!-- Output path -->
    <div class="panel-grid-full" style="display:flex;gap:8px;align-items:center;">
      <el-input v-model="imageOutputPath" placeholder="输出路径（自动生成，可手动修改）" style="flex:1;" />
      <el-button @click="pickOutputDir">选择目录</el-button>
    </div>

    <!-- Actions -->
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="convertImage" :loading="converting">转换图片</el-button>
        <el-button @click="resetForm">重置</el-button>
      </el-space>
    </div>

    <!-- Result -->
    <div v-if="convertResult" class="panel-grid-full image-result-card">
      <div class="image-info-item">
        <span class="image-info-label">输出路径</span>
        <span class="image-info-value" style="word-break:break-all;">{{ convertResult.outputPath }}</span>
      </div>
      <div class="image-info-item">
        <span class="image-info-label">输出尺寸</span>
        <span class="image-info-value">{{ convertResult.width }} x {{ convertResult.height }}</span>
      </div>
      <div class="image-info-item">
        <span class="image-info-label">输出大小</span>
        <span class="image-info-value">{{ formatSize(convertResult.size) }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../bridge/tauri";

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
const converting = ref(false);
const previewSrc = ref("");
const imageInfo = ref<ImageInfo | null>(null);
const convertResult = ref<ConvertResult | null>(null);

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "avif", "bmp", "gif", "tiff", "tif"];

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function replaceExtension(path: string, newExt: string): string {
  const dot = path.lastIndexOf(".");
  const base = dot >= 0 ? path.slice(0, dot) : path;
  return `${base}_converted.${newExt}`;
}

function updateOutputPath() {
  if (!imageInputPath.value) return;
  imageOutputPath.value = replaceExtension(imageInputPath.value, imageFormat.value);
}

async function loadImageInfo(path: string) {
  try {
    const data = await invokeToolByChannel("tool:image:info", { inputPath: path }) as ImageInfo;
    imageInfo.value = data;
    // Set initial width/height from source to 0 (keep original)
    imageWidth.value = 0;
    imageHeight.value = 0;
  } catch {
    imageInfo.value = null;
  }
}

async function onInputPathChange() {
  const path = imageInputPath.value.trim();
  if (!path) {
    previewSrc.value = "";
    imageInfo.value = null;
    return;
  }
  previewSrc.value = convertFileSrc(path);
  updateOutputPath();
  await loadImageInfo(path);
}

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
    convertResult.value = null;
    updateOutputPath();
    await loadImageInfo(path);
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function pickOutputDir() {
  try {
    const selected = await open({
      directory: true,
    });
    if (!selected) return;
    const dir = typeof selected === "string" ? selected : selected.path;
    if (!dir) return;
    // Use input filename with new extension under selected dir
    const inputName = imageInputPath.value.split(/[\\/]/).pop() || "output";
    const baseName = inputName.replace(/\.[^.]+$/, "");
    imageOutputPath.value = `${dir}\\${baseName}_converted.${imageFormat.value}`;
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function convertImage() {
  if (!imageInputPath.value.trim()) {
    ElMessage.warning("请先选择图片");
    return;
  }
  if (!imageOutputPath.value.trim()) {
    ElMessage.warning("请指定输出路径");
    return;
  }
  converting.value = true;
  try {
    const payload: Record<string, unknown> = {
      inputPath: imageInputPath.value,
      outputPath: imageOutputPath.value,
      quality: imageQuality.value,
      format: imageFormat.value,
    };
    if (imageWidth.value > 0) payload.width = imageWidth.value;
    if (imageHeight.value > 0) payload.height = imageHeight.value;
    if (cropWidth.value > 0 && cropHeight.value > 0) {
      payload.cropX = cropX.value;
      payload.cropY = cropY.value;
      payload.cropWidth = cropWidth.value;
      payload.cropHeight = cropHeight.value;
    }
    const data = await invokeToolByChannel("tool:image:convert", payload) as ConvertResult;
    convertResult.value = data;
    ElMessage.success("转换完成");
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    converting.value = false;
  }
}

function resetForm() {
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
  previewSrc.value = "";
  imageInfo.value = null;
  convertResult.value = null;
}
</script>

<style scoped>
.field-label {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  margin-bottom: 6px;
}

.image-preview-row {
  display: flex;
  gap: 16px;
  align-items: flex-start;
}

.image-preview-box {
  border: 1px dashed var(--lc-border-hover);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-2);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 8px;
  min-width: 200px;
  max-width: 300px;
  min-height: 150px;
}

.image-preview-img {
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
  gap: 12px;
  align-items: baseline;
}

.image-info-label {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
  min-width: 60px;
}

.image-info-value {
  font-size: 13px;
  color: var(--lc-text);
}

.image-result-card {
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
