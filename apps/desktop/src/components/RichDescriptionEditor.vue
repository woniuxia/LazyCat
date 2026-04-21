<template>
  <div class="rte" :class="{ 'is-disabled': disabled }">
    <div class="rte-toolbar" v-if="!disabled">
      <el-button-group size="small">
        <el-tooltip content="加粗 Ctrl+B" placement="top">
          <el-button
            :class="{ 'is-active': isActive('bold') }"
            @click="run((c) => c.toggleBold())"
          >B</el-button>
        </el-tooltip>
        <el-tooltip content="斜体 Ctrl+I" placement="top">
          <el-button
            :class="{ 'is-active': isActive('italic') }"
            @click="run((c) => c.toggleItalic())"
          ><i>I</i></el-button>
        </el-tooltip>
        <el-tooltip content="删除线" placement="top">
          <el-button
            :class="{ 'is-active': isActive('strike') }"
            @click="run((c) => c.toggleStrike())"
          ><s>S</s></el-button>
        </el-tooltip>
        <el-tooltip content="行内代码" placement="top">
          <el-button
            :class="{ 'is-active': isActive('code') }"
            @click="run((c) => c.toggleCode())"
          ><code>&lt;/&gt;</code></el-button>
        </el-tooltip>
      </el-button-group>
      <el-button-group size="small">
        <el-tooltip content="一级标题" placement="top">
          <el-button
            :class="{ 'is-active': isActive('heading', { level: 1 }) }"
            @click="run((c) => c.toggleHeading({ level: 1 }))"
          >H1</el-button>
        </el-tooltip>
        <el-tooltip content="二级标题" placement="top">
          <el-button
            :class="{ 'is-active': isActive('heading', { level: 2 }) }"
            @click="run((c) => c.toggleHeading({ level: 2 }))"
          >H2</el-button>
        </el-tooltip>
        <el-tooltip content="三级标题" placement="top">
          <el-button
            :class="{ 'is-active': isActive('heading', { level: 3 }) }"
            @click="run((c) => c.toggleHeading({ level: 3 }))"
          >H3</el-button>
        </el-tooltip>
      </el-button-group>
      <el-button-group size="small">
        <el-tooltip content="无序列表" placement="top">
          <el-button
            :class="{ 'is-active': isActive('bulletList') }"
            @click="run((c) => c.toggleBulletList())"
          >•&nbsp;List</el-button>
        </el-tooltip>
        <el-tooltip content="有序列表" placement="top">
          <el-button
            :class="{ 'is-active': isActive('orderedList') }"
            @click="run((c) => c.toggleOrderedList())"
          >1.&nbsp;List</el-button>
        </el-tooltip>
        <el-tooltip content="引用" placement="top">
          <el-button
            :class="{ 'is-active': isActive('blockquote') }"
            @click="run((c) => c.toggleBlockquote())"
          >&ldquo;&nbsp;&rdquo;</el-button>
        </el-tooltip>
        <el-tooltip content="代码块" placement="top">
          <el-button
            :class="{ 'is-active': isActive('codeBlock') }"
            @click="run((c) => c.toggleCodeBlock())"
          >Code</el-button>
        </el-tooltip>
      </el-button-group>
      <el-button-group size="small">
        <el-tooltip content="分割线" placement="top">
          <el-button @click="run((c) => c.setHorizontalRule())">&mdash;</el-button>
        </el-tooltip>
        <el-tooltip content="链接" placement="top">
          <el-button
            :class="{ 'is-active': isActive('link') }"
            @click="promptLink"
          >Link</el-button>
        </el-tooltip>
      </el-button-group>
      <el-button-group size="small">
        <el-tooltip content="撤销 Ctrl+Z" placement="top">
          <el-button
            :disabled="!editor?.can().undo()"
            @click="run((c) => c.undo())"
          >↶</el-button>
        </el-tooltip>
        <el-tooltip content="重做 Ctrl+Y" placement="top">
          <el-button
            :disabled="!editor?.can().redo()"
            @click="run((c) => c.redo())"
          >↷</el-button>
        </el-tooltip>
      </el-button-group>
      <span class="rte-hint" v-if="uploadingCount > 0">
        {{ uploadingCount }} 张图片上传中…
      </span>
    </div>
    <EditorContent
      class="rte-prose rte-editable"
      :editor="editor"
    />
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { EditorContent, useEditor } from '@tiptap/vue-3';
import type { Editor, ChainedCommands } from '@tiptap/vue-3';
import { ElMessage, ElMessageBox } from 'element-plus';

import { buildExtensions } from '../rich/extensions';
import { normalizeLegacy } from '../rich/legacy';
import { invokeToolByChannel } from '../bridge/tauri';

type OwnerType = 'pm_project' | 'pm_item' | 'todo';

const props = withDefaults(
  defineProps<{
    modelValue: string;
    ownerType: OwnerType;
    ownerId?: string | number | null;
    placeholder?: string;
    maxImageMb?: number;
    disabled?: boolean;
  }>(),
  {
    placeholder: '输入描述，支持粘贴图片与基础排版',
    maxImageMb: 5,
    disabled: false,
    ownerId: null,
  }
);

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void;
  (e: 'attachment-added', attId: number): void;
  (e: 'oversize', mb: number): void;
}>();

const internalTempId = ref<string | null>(null);
function ensureTempId(): string {
  if (!internalTempId.value) internalTempId.value = `tmp-${crypto.randomUUID()}`;
  return internalTempId.value;
}

function getEffectiveOwnerId(): string {
  // 仅 null / undefined 生成 tempId；显式传 0 / '' 视为合法 id
  if (props.ownerId == null) return ensureTempId();
  return String(props.ownerId);
}

const activeBlobs = new Map<string, string>();
const uploadingCount = ref(0);

const editor = useEditor({
  content: normalizeLegacy(props.modelValue),
  extensions: buildExtensions({ placeholder: props.placeholder }),
  editable: !props.disabled,
  editorProps: {
    handlePaste: (_view, event) => {
      const files = Array.from(event.clipboardData?.files ?? []);
      const images = files.filter((f) => f.type.startsWith('image/'));
      if (images.length === 0) return false;
      event.preventDefault();
      images.forEach((f) => uploadPastedImage(f));
      return true;
    },
    handleDrop: (_view, event, _slice, moved) => {
      if (moved) return false;
      const dt = event.dataTransfer;
      if (!dt) return false;
      const files = Array.from(dt.files ?? []).filter((f) => f.type.startsWith('image/'));
      if (files.length === 0) return false;
      event.preventDefault();
      files.forEach((f) => uploadPastedImage(f));
      return true;
    },
  },
  onUpdate: ({ editor }) => {
    emit('update:modelValue', JSON.stringify(editor.getJSON()));
  },
});

watch(
  () => props.disabled,
  (d) => editor.value?.setEditable(!d)
);

// 外部 modelValue 变化时，仅当与当前 editor 不同步（例如父组件切换编辑对象）才覆盖
watch(
  () => props.modelValue,
  (v) => {
    const e = editor.value;
    if (!e) return;
    const current = JSON.stringify(e.getJSON());
    if (current === v) return;
    // 也兼容外部传入的 legacy 纯文本
    e.commands.setContent(normalizeLegacy(v), { emitUpdate: false });
  }
);

function isActive(name: string, attrs?: Record<string, unknown>): boolean {
  if (!editor.value) return false;
  return attrs ? editor.value.isActive(name, attrs) : editor.value.isActive(name);
}

function run(fn: (chain: ChainedCommands) => ChainedCommands): void {
  const e = editor.value;
  if (!e) return;
  fn(e.chain().focus()).run();
}

async function promptLink(): Promise<void> {
  const e = editor.value;
  if (!e) return;
  const existing = e.getAttributes('link')?.href ?? '';
  let input = '';
  try {
    const res = await ElMessageBox.prompt('输入链接地址（仅支持 http/https/mailto）', '插入链接', {
      inputValue: String(existing ?? ''),
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      inputPlaceholder: 'https://example.com',
    });
    input = String(res.value ?? '').trim();
  } catch {
    return;
  }
  if (!input) {
    e.chain().focus().extendMarkRange('link').unsetLink().run();
    return;
  }
  if (!/^(https?:|mailto:)/i.test(input)) {
    ElMessage.warning('仅支持 http/https/mailto 协议');
    return;
  }
  e.chain().focus().extendMarkRange('link').setLink({ href: input }).run();
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const s = String(reader.result ?? '');
      const idx = s.indexOf(',');
      resolve(idx >= 0 ? s.slice(idx + 1) : s);
    };
    reader.onerror = () => reject(reader.error ?? new Error('read file failed'));
    reader.readAsDataURL(file);
  });
}

async function uploadPastedImage(file: File): Promise<void> {
  const maxBytes = (props.maxImageMb ?? 5) * 1024 * 1024;
  if (file.size > maxBytes) {
    const mb = file.size / 1024 / 1024;
    ElMessage.warning(`单张图片不能超过 ${props.maxImageMb ?? 5} MB`);
    emit('oversize', mb);
    return;
  }

  const uploadingId = crypto.randomUUID();
  const blobUrl = URL.createObjectURL(file);
  activeBlobs.set(uploadingId, blobUrl);
  uploadingCount.value += 1;

  // 插入占位 image 节点
  const e = editor.value;
  if (e) {
    e.chain()
      .focus()
      .insertContent({
        type: 'image',
        attrs: { src: blobUrl, alt: file.name, uploadingId, attId: null },
      })
      .run();
  }

  try {
    const bytesBase64 = await fileToBase64(file);
    const res = (await invokeToolByChannel('tool:attachments:save', {
      ownerType: props.ownerType,
      ownerId: getEffectiveOwnerId(),
      fileName: file.name,
      mime: file.type || 'application/octet-stream',
      kind: 'image',
      bytesBase64,
    })) as { id: number; relPath: string; hash: string; size: number };

    if (editor.value) {
      replaceUploadingImage(uploadingId, res.relPath, res.id);
    }
    emit('attachment-added', res.id);
  } catch (err) {
    const msg = (err as Error)?.message ?? '上传图片失败';
    ElMessage.error(msg);
    if (editor.value) removeUploadingImage(uploadingId);
  } finally {
    URL.revokeObjectURL(blobUrl);
    activeBlobs.delete(uploadingId);
    uploadingCount.value = Math.max(0, uploadingCount.value - 1);
  }
}

function replaceUploadingImage(uploadingId: string, src: string, attId: number): void {
  const view = editor.value?.view;
  if (!view) return;
  const tr = view.state.tr;
  let changed = false;
  view.state.doc.descendants((node, pos) => {
    if (node.type.name === 'image' && node.attrs?.uploadingId === uploadingId) {
      tr.setNodeMarkup(pos, undefined, {
        ...node.attrs,
        src,
        attId,
        uploadingId: null,
      });
      changed = true;
    }
  });
  if (changed) {
    view.dispatch(tr);
    // 同步 modelValue（tr.setNodeMarkup 不触发 onUpdate 时）
    const e = editor.value;
    if (e) emit('update:modelValue', JSON.stringify(e.getJSON()));
  }
}

function removeUploadingImage(uploadingId: string): void {
  const view = editor.value?.view;
  if (!view) return;
  let tr = view.state.tr;
  let changed = false;
  // 倒序删除，避免 pos 偏移
  const positions: Array<{ pos: number; size: number }> = [];
  view.state.doc.descendants((node, pos) => {
    if (node.type.name === 'image' && node.attrs?.uploadingId === uploadingId) {
      positions.push({ pos, size: node.nodeSize });
    }
  });
  positions
    .sort((a, b) => b.pos - a.pos)
    .forEach(({ pos, size }) => {
      tr = tr.delete(pos, pos + size);
      changed = true;
    });
  if (changed) {
    view.dispatch(tr);
    const e = editor.value;
    if (e) emit('update:modelValue', JSON.stringify(e.getJSON()));
  }
}

function collectAttachmentIds(): number[] {
  const e = editor.value;
  if (!e) return [];
  const ids = new Set<number>();
  e.state.doc.descendants((node) => {
    if (node.type.name === 'image') {
      const v = node.attrs?.attId;
      if (typeof v === 'number' && Number.isFinite(v)) ids.add(v);
    }
  });
  return [...ids];
}

onBeforeUnmount(() => {
  activeBlobs.forEach((url) => URL.revokeObjectURL(url));
  activeBlobs.clear();
});

defineExpose({
  focus: () => editor.value?.commands.focus(),
  blur: () => editor.value?.commands.blur(),
  getAttachmentIds: collectAttachmentIds,
  getEffectiveOwnerId,
  getEditor: () => editor.value,
  /**
   * 重置内部状态：清空 tempId + 重新 setContent。
   * 用于对话框复用同一 Editor 实例时，进入下一轮编辑前调用。
   */
  reset(nextValue: string) {
    internalTempId.value = null;
    activeBlobs.forEach((url) => URL.revokeObjectURL(url));
    activeBlobs.clear();
    uploadingCount.value = 0;
    const e = editor.value;
    if (!e) return;
    e.commands.setContent(normalizeLegacy(nextValue), { emitUpdate: false });
  },
});
</script>

<style scoped>
.rte {
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  background: var(--el-bg-color);
  display: flex;
  flex-direction: column;
  min-height: 180px;
}
.rte.is-disabled {
  opacity: 0.7;
}
.rte-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  background: var(--el-fill-color-light);
  align-items: center;
}
.rte-toolbar :deep(.el-button) {
  padding: 4px 10px;
  min-width: 32px;
  font-size: 12px;
  line-height: 1;
}
.rte-toolbar :deep(.el-button.is-active) {
  color: var(--el-color-primary);
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}
.rte-hint {
  margin-left: auto;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.rte-editable {
  flex: 1;
  padding: 10px 12px;
  font-size: 14px;
  line-height: 1.6;
  color: var(--el-text-color-primary);
  overflow: auto;
}
.rte-editable :deep(.ProseMirror) {
  outline: none;
  min-height: 120px;
}
.rte-editable :deep(.ProseMirror p.is-editor-empty:first-child::before) {
  content: attr(data-placeholder);
  float: left;
  color: var(--el-text-color-placeholder);
  pointer-events: none;
  height: 0;
}
.rte-editable :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: 4px;
}
.rte-editable :deep(img[data-uploading-id]) {
  opacity: 0.55;
  outline: 2px dashed var(--el-color-primary-light-5);
  outline-offset: 2px;
}
.rte-editable :deep(blockquote) {
  border-left: 3px solid var(--el-border-color);
  color: var(--el-text-color-regular);
  padding: 2px 10px;
  margin: 8px 0;
}
.rte-editable :deep(pre) {
  background: var(--el-fill-color);
  padding: 8px 10px;
  border-radius: 4px;
  overflow-x: auto;
  font-family: var(--el-font-family-monospace, Menlo, Consolas, monospace);
  font-size: 13px;
}
.rte-editable :deep(code) {
  background: var(--el-fill-color);
  padding: 0 4px;
  border-radius: 3px;
  font-size: 13px;
}
.rte-editable :deep(a) {
  color: var(--el-color-primary);
  text-decoration: underline;
}
.rte-editable :deep(hr) {
  border: none;
  border-top: 1px solid var(--el-border-color);
  margin: 12px 0;
}
.rte-editable :deep(ul),
.rte-editable :deep(ol) {
  padding-left: 22px;
}
</style>
