import { Node } from '@tiptap/vue-3';
import StarterKit from '@tiptap/starter-kit';
import Image from '@tiptap/extension-image';
import Link from '@tiptap/extension-link';
import { Placeholder } from '@tiptap/extensions';

import { ensureDataDir, getSyncDataDir, resolveAttachmentPath } from './data-dir';

/**
 * 封装 Tauri asset 协议转换。
 * TS 5.9 无法正确解析 @tauri-apps/api/core 的 convertFileSrc 命名导出（.d.ts 确实有导出），
 * 通过 __TAURI_INTERNALS__ 间接调用规避类型检查问题。
 */
function toAssetUrl(absPath: string): string {
  return (window as unknown as { __TAURI_INTERNALS__: { convertFileSrc: (p: string) => string } })
    .__TAURI_INTERNALS__.convertFileSrc(absPath);
}

/**
 * 共享 schema 模块：Editor 与 Viewer 必须使用完全一致的扩展集，
 * 否则 Editor 写入的 JSON 在 Viewer 侧无法识别节点/mark 类型。
 *
 * 注意事项：
 * - Link 只放行 http/https/mailto 协议，openOnClick 关闭（由 Viewer 层的
 *   点击监听走 tool:system:open_external）
 * - Image 扩展自定义 attId / uploadingId 两个 attr，分别承载附件 DB id
 *   与前端粘贴期间的占位标识
 * - FileRef 为 inline atom Node，承载非图片文件的"内嵌链接"节点；
 *   kind='attachment' 表示文件已复制到附件目录（src 为相对路径），
 *   kind='path' 表示仅保存了原始绝对路径（src 为本地绝对路径）
 */

export const LINK_PROTOCOLS = ['http', 'https', 'mailto'] as const;

const CustomImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      attId: {
        default: null,
        parseHTML: (el: HTMLElement) => {
          const v = el.getAttribute('data-att-id');
          if (v == null || v === '') return null;
          const n = Number(v);
          return Number.isFinite(n) ? n : null;
        },
        renderHTML: (attrs: Record<string, unknown>) => {
          const v = attrs.attId;
          return v == null ? {} : { 'data-att-id': String(v) };
        },
      },
      uploadingId: {
        default: null,
        parseHTML: (el: HTMLElement) => el.getAttribute('data-uploading-id'),
        renderHTML: (attrs: Record<string, unknown>) => {
          const v = attrs.uploadingId;
          return v == null || v === '' ? {} : { 'data-uploading-id': String(v) };
        },
      },
    };
  },

  /**
   * NodeView：Editor 里 image 节点的 src 可能是：
   * - `blob:...`（上传中占位）
   * - `attachments/<hash>.<ext>`（已落盘附件的相对路径）
   * - 带 scheme 的外链（远程图）
   *
   * ProseMirror 默认的 toDOM 会把 attrs.src 原样塞进 <img src>，相对路径在
   * WebView 里会相对于当前页面 URL 解析，导致图片无法加载。这里用 NodeView
   * 在 DOM 渲染层做一次 convertFileSrc 转换,JSON 保存的 attrs.src 保持相对路径
   * 不变,避免污染持久化数据。
   *
   * dataDir 未就绪时会异步兜底一次,回调里比对 currentSrc 防止旧值覆盖新值。
   */
  addNodeView() {
    return ({ node }) => {
      const dom = document.createElement('img');
      let currentSrc = '';

      const applySrc = (src: string) => {
        if (!src) {
          dom.removeAttribute('src');
          return;
        }
        if (src.startsWith('blob:') || /^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(src)) {
          dom.src = src;
          return;
        }
        const dir = getSyncDataDir();
        if (dir) {
          dom.src = toAssetUrl(resolveAttachmentPath(src, dir));
          return;
        }
        void ensureDataDir().then((d) => {
          if (!d || currentSrc !== src) return;
          dom.src = toAssetUrl(resolveAttachmentPath(src, d));
        });
      };

      const applyAttrs = (n: typeof node) => {
        const attrs = n.attrs as Record<string, unknown>;
        const alt = attrs.alt;
        const title = attrs.title;
        const attId = attrs.attId;
        const uploadingId = attrs.uploadingId;
        if (typeof alt === 'string' && alt) dom.setAttribute('alt', alt);
        else dom.removeAttribute('alt');
        if (typeof title === 'string' && title) dom.setAttribute('title', title);
        else dom.removeAttribute('title');
        if (attId != null) dom.setAttribute('data-att-id', String(attId));
        else dom.removeAttribute('data-att-id');
        if (uploadingId) dom.setAttribute('data-uploading-id', String(uploadingId));
        else dom.removeAttribute('data-uploading-id');
        const src = String(attrs.src ?? '');
        currentSrc = src;
        applySrc(src);
      };

      applyAttrs(node);

      return {
        dom,
        update: (updatedNode) => {
          if (updatedNode.type.name !== 'image') return false;
          applyAttrs(updatedNode);
          return true;
        },
      };
    };
  },
});

/**
 * FileRef：非图片文件的内嵌引用节点。
 *
 * - inline + atom：作为一个整体字符参与排版，Backspace 整块删
 * - selectable：允许用户点击选中，配合右键菜单做"删除节点"
 * - draggable: false：避免与 ProseMirror 默认拖拽行为叠加导致光标定位异常
 *
 * 属性：
 * - attId：仅 kind='attachment' 场景有值，与 attachments 表行绑定
 * - src：'attachments/<hash>.<ext>'（attachment）或绝对路径（path）
 * - name：显示用文件名
 * - size：字节数，nullable
 * - mime：原始 mime，可空
 * - kind：'attachment' | 'path'
 * - uploadingId：上传中占位标识，回填后置空
 */
export const FileRef = Node.create({
  name: 'fileRef',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      attId: {
        default: null,
        parseHTML: (el: HTMLElement) => {
          const v = el.getAttribute('data-att-id');
          if (v == null || v === '') return null;
          const n = Number(v);
          return Number.isFinite(n) ? n : null;
        },
        renderHTML: (attrs: Record<string, unknown>) => {
          const v = attrs.attId;
          return v == null ? {} : { 'data-att-id': String(v) };
        },
      },
      src: {
        default: '',
        parseHTML: (el: HTMLElement) => el.getAttribute('data-src') ?? '',
        renderHTML: (attrs: Record<string, unknown>) => ({
          'data-src': String(attrs.src ?? ''),
        }),
      },
      name: {
        default: '',
        parseHTML: (el: HTMLElement) => el.getAttribute('data-name') ?? '',
        renderHTML: (attrs: Record<string, unknown>) => ({
          'data-name': String(attrs.name ?? ''),
        }),
      },
      size: {
        default: null,
        parseHTML: (el: HTMLElement) => {
          const v = el.getAttribute('data-size');
          if (v == null || v === '') return null;
          const n = Number(v);
          return Number.isFinite(n) ? n : null;
        },
        renderHTML: (attrs: Record<string, unknown>) => {
          const v = attrs.size;
          return v == null ? {} : { 'data-size': String(v) };
        },
      },
      mime: {
        default: '',
        parseHTML: (el: HTMLElement) => el.getAttribute('data-mime') ?? '',
        renderHTML: (attrs: Record<string, unknown>) => {
          const v = attrs.mime;
          return v == null || v === '' ? {} : { 'data-mime': String(v) };
        },
      },
      kind: {
        default: 'attachment',
        parseHTML: (el: HTMLElement) => {
          const v = el.getAttribute('data-kind') ?? 'attachment';
          return v === 'path' ? 'path' : 'attachment';
        },
        renderHTML: (attrs: Record<string, unknown>) => ({
          'data-kind': attrs.kind === 'path' ? 'path' : 'attachment',
        }),
      },
      uploadingId: {
        default: null,
        parseHTML: (el: HTMLElement) => el.getAttribute('data-uploading-id'),
        renderHTML: (attrs: Record<string, unknown>) => {
          const v = attrs.uploadingId;
          return v == null || v === '' ? {} : { 'data-uploading-id': String(v) };
        },
      },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-file-ref]' }];
  },

  renderHTML({ node, HTMLAttributes }) {
    const label = String(node.attrs.name ?? '').trim() || '未命名文件';
    return [
      'span',
      {
        ...HTMLAttributes,
        'data-file-ref': '',
        class: 'rte-file-ref',
      },
      `\uD83D\uDCCE ${label}`,
    ];
  },
});

export function buildExtensions(opts: { placeholder?: string } = {}) {
  return [
    StarterKit.configure({
      // StarterKit 已内置 Paragraph/Heading/Bold/Italic/Strike/Code/BulletList/
      // OrderedList/Blockquote/CodeBlock/HardBreak/HorizontalRule/Dropcursor/
      // Gapcursor/History；不在这里重复扩展。
      link: false,
    }),
    CustomImage,
    FileRef,
    Link.configure({
      openOnClick: false,
      autolink: true,
      defaultProtocol: 'https',
      protocols: [...LINK_PROTOCOLS] as string[],
      HTMLAttributes: { rel: 'noopener noreferrer' },
    }),
    Placeholder.configure({ placeholder: opts.placeholder ?? '' }),
  ];
}
