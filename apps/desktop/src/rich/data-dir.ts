import { invokeToolByChannel } from '../bridge/tauri';

/**
 * 描述富文本共用的 dataDir 缓存：
 * - 首次访问发起一次 `tool:system:get-paths`，随后走内存缓存
 * - Editor 的 NodeView 与 Viewer 的 rewriteLocalSrc 都依赖此值来拼接附件绝对路径
 *
 * 失败（例如后端未就绪）会清掉 pending，允许下一次调用重试；cache 保持空串。
 */

let cache = '';
let pending: Promise<string> | null = null;

export function getSyncDataDir(): string {
  return cache;
}

export function ensureDataDir(): Promise<string> {
  if (cache) return Promise.resolve(cache);
  if (!pending) {
    pending = invokeToolByChannel('tool:system:get-paths', {})
      .then((res) => {
        const v = (res as { dataDir?: string })?.dataDir ?? '';
        cache = v;
        return v;
      })
      .catch(() => {
        pending = null;
        return '';
      }) as Promise<string>;
  }
  return pending;
}

/** 拼接附件相对路径为绝对路径（保留原生分隔符，供后端 IPC 使用） */
export function joinAttachmentPath(dir: string, rel: string): string {
  const d = dir.replace(/[\/\\]+$/, '');
  const s = rel.replace(/^[\/\\]+/, '');
  return `${d}/${s}`;
}

/**
 * 把 FileRef / Image 的 `src`（可能是相对路径、已带 scheme 的 URL、blob:）
 * 转为绝对路径。
 * - blob: 或已带 scheme 的直接原样返回（调用方无需再转 convertFileSrc）
 * - 相对路径：拼接 dataDir 得到绝对路径（统一正斜杠，供 convertFileSrc 使用）
 * - dataDir 未就绪时返回原值（交由调用方异步重试）
 */
export function resolveAttachmentPath(src: string, dataDir: string): string {
  if (!src) return '';
  if (src.startsWith('blob:') || /^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(src)) {
    return src;
  }
  if (!dataDir) return src;
  return joinAttachmentPath(dataDir, src).replace(/\\/g, '/');
}
