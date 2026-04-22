<template>
  <div class="rte" :class="{ 'is-disabled': disabled }" @contextmenu="onContextMenu">
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
        {{ uploadingCount }} 个附件上传中…
      </span>
    </div>
    <EditorContent
      class="rte-prose rte-editable"
      :editor="editor"
    />
    <RteFileRefMenu
      :visible="menu.visible"
      :x="menu.x"
      :y="menu.y"
      :kind="menu.kind"
      :can-delete="!disabled"
      @close="menu.visible = false"
      @action="onMenuAction"
    />
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { EditorContent, useEditor } from '@tiptap/vue-3';
import type { ChainedCommands } from '@tiptap/vue-3';
import { ElMessage, ElMessageBox } from 'element-plus';

import { buildExtensions } from '../rich/extensions';
import { normalizeLegacy } from '../rich/legacy';
import { ensureDataDir, joinAttachmentPath } from '../rich/data-dir';
import { invokeToolByChannel } from '../bridge/tauri';
import RteFileRefMenu from './RteFileRefMenu.vue';

type OwnerType = 'pm_project' | 'pm_item' | 'todo';
type FileRefKind = 'attachment' | 'path';

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
    placeholder: '输入描述，支持粘贴图片/文件与基础排版',
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

// 右键菜单状态。target 信息从 DOM 反解：
// - src：附件相对路径 or 本地绝对路径
// - kind：决定展示与点击行为
// - attId：仅 attachment 类型有值，用于"删除节点"时可选地回调 cleanup
// - domEl：用于在选择"删除"时通过 view.posAtDOM 定位节点
interface MenuTarget {
  src: string;
  name: string;
  kind: FileRefKind;
  attId: number | null;
  domEl: HTMLElement;
}
const menu = reactive({
  visible: false,
  x: 0,
  y: 0,
  kind: 'attachment' as FileRefKind,
  target: null as MenuTarget | null,
});

// dataDir 走共享缓存：Editor/Viewer/NodeView 共用一份，避免重复 IPC。
// onMounted 里预热一次，让 NodeView 首次渲染就能同步拿到值。

const editor = useEditor({
  content: normalizeLegacy(props.modelValue),
  extensions: buildExtensions({ placeholder: props.placeholder }),
  editable: !props.disabled,
  editorProps: {
    handlePaste: (_view, event) => {
      const files = Array.from(event.clipboardData?.files ?? []);
      if (files.length === 0) return false;
      const images = files.filter((f) => f.type.startsWith('image/'));
      const nonImages = files.filter((f) => !f.type.startsWith('image/'));
      if (nonImages.length === 0 && images.length > 0) {
        event.preventDefault();
        images.forEach((f) => uploadPastedImage(f));
        return true;
      }
      if (nonImages.length > 0) {
        event.preventDefault();
        void handlePastedFiles(nonImages, /* fromPaste */ true);
        // 混合粘贴：图片也并发处理，避免截图+文件场景图片丢失
        images.forEach((f) => uploadPastedImage(f));
        return true;
      }
      return false;
    },
    handleDrop: (_view, event, _slice, moved) => {
      if (moved) return false;
      const dt = event.dataTransfer;
      if (!dt) return false;
      const files = Array.from(dt.files ?? []);
      if (files.length === 0) return false;
      const images = files.filter((f) => f.type.startsWith('image/'));
      const nonImages = files.filter((f) => !f.type.startsWith('image/'));
      if (nonImages.length === 0 && images.length > 0) {
        event.preventDefault();
        images.forEach((f) => uploadPastedImage(f));
        return true;
      }
      if (nonImages.length > 0) {
        event.preventDefault();
        void handlePastedFiles(nonImages, /* fromPaste */ false);
        images.forEach((f) => uploadPastedImage(f));
        return true;
      }
      return false;
    },
    handleClickOn: (_view, _pos, node, _nodePos, event, _direct) => {
      if (node.type.name !== 'fileRef') return false;
      // 左键点击：用系统默认程序打开；右键不在此分支处理
      if ((event as MouseEvent).button !== 0) return false;
      const kind = (node.attrs.kind === 'path' ? 'path' : 'attachment') as FileRefKind;
      const src = String(node.attrs.src ?? '');
      const uploadingId = node.attrs.uploadingId;
      if (uploadingId) {
        ElMessage.info('文件上传中，请稍候');
        return true;
      }
      if (!src) {
        ElMessage.warning('附件路径为空');
        return true;
      }
      void openFileRef(kind, src);
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

// ── 非图片文件处理 ────────────────────────────────────
//
// 粘贴场景：调 system:read_clipboard_files 拿真实路径（仅 Windows 有效）；
// 拖拽场景：无法拿真实路径，files[i].name 仅是 basename。
// 弹窗询问："复制到附件" / "仅保存路径"，无路径时只允许前者。

async function handlePastedFiles(files: File[], fromPaste: boolean): Promise<void> {
  let paths: string[] = [];
  if (fromPaste) {
    try {
      const res = (await invokeToolByChannel('tool:system:read-clipboard-files', {})) as {
        paths?: string[];
      };
      paths = Array.isArray(res?.paths) ? res.paths : [];
    } catch {
      paths = [];
    }
  }
  // 对齐到 files 数量，保证索引 mapping 稳定；不够的位置置空
  if (paths.length !== files.length) {
    const hasAny = paths.length > 0;
    const aligned = new Array<string>(files.length).fill('');
    if (hasAny) {
      // 按 basename 匹配；匹配不到的保持空串
      const used = new Set<number>();
      files.forEach((file, idx) => {
        const base = file.name;
        const hit = paths.findIndex((p, i) => {
          if (used.has(i)) return false;
          const bn = p.split(/[\\/]/).pop() ?? '';
          return bn === base;
        });
        if (hit >= 0) {
          aligned[idx] = paths[hit];
          used.add(hit);
        }
      });
    }
    paths = aligned;
  }

  const hasPath = paths.some((p) => p && p.length > 0);
  let mode: 'copy' | 'path' = 'copy';
  try {
    await ElMessageBox.confirm(
      hasPath
        ? `共 ${files.length} 个文件。选择"复制到附件"将文件拷贝到本工具的附件目录；选择"仅保存路径"只记录原始路径（原文件被移动或删除后将失效）。`
        : `共 ${files.length} 个文件，当前只能复制到附件目录（未能获取原始路径）。`,
      '粘贴文件',
      {
        confirmButtonText: '复制到附件',
        cancelButtonText: hasPath ? '仅保存路径' : '取消',
        distinguishCancelAndClose: true,
        type: 'info',
      }
    );
    mode = 'copy';
  } catch (action) {
    if (action === 'cancel' && hasPath) {
      mode = 'path';
    } else {
      return; // close 或 无路径时取消
    }
  }

  if (mode === 'copy') {
    for (let i = 0; i < files.length; i += 1) {
      const f = files[i];
      const srcPath = paths[i] || undefined;
      void uploadPastedFile(f, srcPath);
    }
  } else {
    for (let i = 0; i < files.length; i += 1) {
      const f = files[i];
      const p = paths[i];
      if (!p) continue;
      insertPathRef(p, f);
    }
  }
}

async function uploadPastedFile(file: File, srcPath?: string): Promise<void> {
  const uploadingId = crypto.randomUUID();
  uploadingCount.value += 1;
  const e = editor.value;
  if (e) {
    e.chain()
      .focus()
      .insertContent({
        type: 'fileRef',
        attrs: {
          src: '',
          name: file.name,
          size: file.size,
          mime: file.type || '',
          kind: 'attachment',
          uploadingId,
          attId: null,
        },
      })
      .run();
  }

  try {
    let res: { id: number; relPath: string; hash: string; size: number };
    if (srcPath) {
      res = (await invokeToolByChannel('tool:attachments:save-from-path', {
        ownerType: props.ownerType,
        ownerId: getEffectiveOwnerId(),
        srcPath,
        fileName: file.name,
        mime: file.type || '',
        kind: 'file',
      })) as { id: number; relPath: string; hash: string; size: number };
    } else {
      // 拖拽场景无原始路径：回退 base64（大文件会慢，已在弹窗提示）
      const bytesBase64 = await fileToBase64(file);
      res = (await invokeToolByChannel('tool:attachments:save', {
        ownerType: props.ownerType,
        ownerId: getEffectiveOwnerId(),
        fileName: file.name,
        mime: file.type || 'application/octet-stream',
        kind: 'file',
        bytesBase64,
      })) as { id: number; relPath: string; hash: string; size: number };
    }
    if (editor.value) {
      replaceUploadingFileRef(uploadingId, {
        src: res.relPath,
        attId: res.id,
        size: res.size,
        name: file.name,
        mime: file.type || '',
      });
    }
    emit('attachment-added', res.id);
  } catch (err) {
    const msg = (err as Error)?.message ?? '上传文件失败';
    ElMessage.error(msg);
    if (editor.value) removeUploadingFileRef(uploadingId);
  } finally {
    uploadingCount.value = Math.max(0, uploadingCount.value - 1);
  }
}

function insertPathRef(srcPath: string, file: File): void {
  const e = editor.value;
  if (!e) return;
  e.chain()
    .focus()
    .insertContent({
      type: 'fileRef',
      attrs: {
        src: srcPath,
        name: file.name,
        size: file.size,
        mime: file.type || '',
        kind: 'path',
        attId: null,
        uploadingId: null,
      },
    })
    .run();
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

function replaceUploadingFileRef(
  uploadingId: string,
  next: { src: string; attId: number; size: number; name: string; mime: string }
): void {
  const view = editor.value?.view;
  if (!view) return;
  const tr = view.state.tr;
  let changed = false;
  view.state.doc.descendants((node, pos) => {
    if (node.type.name === 'fileRef' && node.attrs?.uploadingId === uploadingId) {
      tr.setNodeMarkup(pos, undefined, {
        ...node.attrs,
        src: next.src,
        attId: next.attId,
        size: next.size,
        name: next.name,
        mime: next.mime,
        kind: 'attachment',
        uploadingId: null,
      });
      changed = true;
    }
  });
  if (changed) {
    view.dispatch(tr);
    const e = editor.value;
    if (e) emit('update:modelValue', JSON.stringify(e.getJSON()));
  }
}

function removeUploadingFileRef(uploadingId: string): void {
  const view = editor.value?.view;
  if (!view) return;
  let tr = view.state.tr;
  let changed = false;
  const positions: Array<{ pos: number; size: number }> = [];
  view.state.doc.descendants((node, pos) => {
    if (node.type.name === 'fileRef' && node.attrs?.uploadingId === uploadingId) {
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
    } else if (node.type.name === 'fileRef' && node.attrs?.kind !== 'path') {
      const v = node.attrs?.attId;
      if (typeof v === 'number' && Number.isFinite(v)) ids.add(v);
    }
  });
  return [...ids];
}

// ── 右键菜单 & 左键打开 ─────────────────────────────

function onContextMenu(event: MouseEvent): void {
  const target = event.target as Element | null;
  const el = target?.closest?.('.rte-file-ref') as HTMLElement | null;
  if (!el) return; // 非 FileRef 区域：走浏览器默认右键
  event.preventDefault();
  const kind = (el.getAttribute('data-kind') === 'path' ? 'path' : 'attachment') as FileRefKind;
  const src = el.getAttribute('data-src') ?? '';
  const name = el.getAttribute('data-name') ?? '';
  const attIdRaw = el.getAttribute('data-att-id');
  const attId = attIdRaw ? Number(attIdRaw) : null;
  menu.target = {
    src,
    name,
    kind,
    attId: Number.isFinite(attId as number) ? (attId as number) : null,
    domEl: el,
  };
  menu.kind = kind;
  menu.x = event.clientX;
  menu.y = event.clientY;
  menu.visible = true;
}

async function onMenuAction(action: 'open' | 'reveal' | 'copy-path' | 'delete'): Promise<void> {
  const target = menu.target;
  if (!target) return;
  const absPath = await resolveAbsPath(target.kind, target.src);
  if (action === 'open') {
    if (!absPath) return;
    void openAbsPath(absPath);
  } else if (action === 'reveal') {
    if (!absPath) return;
    try {
      await invokeToolByChannel('tool:system:reveal-in-folder', { path: absPath });
    } catch (err) {
      ElMessage.error((err as Error).message || '无法定位文件');
    }
  } else if (action === 'copy-path') {
    if (!absPath) return;
    try {
      await navigator.clipboard.writeText(absPath);
      ElMessage.success('路径已复制');
    } catch {
      ElMessage.error('复制失败');
    }
  } else if (action === 'delete') {
    deleteFileRefByDom(target.domEl);
  }
}

function deleteFileRefByDom(el: HTMLElement): void {
  const view = editor.value?.view;
  if (!view) return;
  let pos: number | null = null;
  try {
    pos = view.posAtDOM(el, 0);
  } catch {
    pos = null;
  }
  if (pos == null) return;
  const $pos = view.state.doc.resolve(pos);
  // posAtDOM 返回节点内部的位置；向父层找到 fileRef 节点
  for (let depth = $pos.depth; depth >= 0; depth -= 1) {
    const node = $pos.node(depth);
    if (node.type.name === 'fileRef') {
      const start = $pos.before(depth);
      const tr = view.state.tr.delete(start, start + node.nodeSize);
      view.dispatch(tr);
      const e = editor.value;
      if (e) emit('update:modelValue', JSON.stringify(e.getJSON()));
      return;
    }
  }
  // posAtDOM 返回的是紧邻节点外侧位置时，直接按 nodeSize 删除
  const node = view.state.doc.nodeAt(pos);
  if (node && node.type.name === 'fileRef') {
    const tr = view.state.tr.delete(pos, pos + node.nodeSize);
    view.dispatch(tr);
    const e = editor.value;
    if (e) emit('update:modelValue', JSON.stringify(e.getJSON()));
  }
}

async function resolveAbsPath(kind: FileRefKind, src: string): Promise<string> {
  if (!src) return '';
  if (kind === 'path') return src;
  const dir = await ensureDataDir();
  if (!dir) return '';
  return joinAttachmentPath(dir, src);
}

async function openFileRef(kind: FileRefKind, src: string): Promise<void> {
  const abs = await resolveAbsPath(kind, src);
  if (!abs) {
    ElMessage.error('附件路径解析失败');
    return;
  }
  await openAbsPath(abs);
}

async function openAbsPath(absPath: string): Promise<void> {
  try {
    await invokeToolByChannel('tool:system:open-local-path', { path: absPath });
  } catch (err) {
    const msg = (err as Error).message || '';
    if (msg.includes('file not found')) {
      ElMessage.error('文件不存在');
    } else {
      ElMessage.error(msg || '无法打开文件');
    }
  }
}

onBeforeUnmount(() => {
  activeBlobs.forEach((url) => URL.revokeObjectURL(url));
  activeBlobs.clear();
});

// 挂载后预热 dataDir：NodeView 首次创建时若拿不到同步值，会走异步兜底；
// 这里直接发起一次请求确保缓存尽快填充。
onMounted(() => {
  void ensureDataDir();
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
    menu.visible = false;
    menu.target = null;
    const e = editor.value;
    if (!e) return;
    e.commands.setContent(normalizeLegacy(nextValue), { emitUpdate: false });
  },
});
</script>

<style scoped>
.rte {
  width: 100%;
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
.rte-editable :deep(.rte-file-ref) {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 1px 6px;
  margin: 0 2px;
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
  border-radius: 4px;
  font-size: 13px;
  line-height: 1.4;
  cursor: pointer;
  border: 1px solid var(--el-color-primary-light-7);
  user-select: none;
  white-space: nowrap;
  vertical-align: baseline;
}
.rte-editable :deep(.rte-file-ref:hover) {
  background: var(--el-color-primary-light-8);
}
.rte-editable :deep(.rte-file-ref.ProseMirror-selectednode) {
  outline: 2px solid var(--el-color-primary);
  outline-offset: 1px;
}
.rte-editable :deep(.rte-file-ref[data-uploading-id]) {
  opacity: 0.6;
  cursor: progress;
}
</style>
