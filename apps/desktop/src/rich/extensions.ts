import StarterKit from '@tiptap/starter-kit';
import Image from '@tiptap/extension-image';
import Link from '@tiptap/extension-link';
import { Placeholder } from '@tiptap/extensions';

/**
 * 共享 schema 模块：Editor 与 Viewer 必须使用完全一致的扩展集，
 * 否则 Editor 写入的 JSON 在 Viewer 侧无法识别节点/mark 类型。
 *
 * 注意事项：
 * - Link 只放行 http/https/mailto 协议，openOnClick 关闭（由 Viewer 层的
 *   点击监听走 tool:system:open_external）
 * - Image 扩展自定义 attId / uploadingId 两个 attr，分别承载附件 DB id
 *   与前端粘贴期间的占位标识
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
