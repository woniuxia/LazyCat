import type { Ref } from 'vue';
import { invokeToolByChannel } from '../bridge/tauri';

/**
 * RichDescriptionEditor 的生命周期协同：
 * - afterSubmit(realId)：新建场景提交后，将 attachments 的 tmp-<uuid> 映射为真实 id
 * - onCancel()：新建场景取消时，清理 tmp 下残留的附件（DB 行 + 物理文件按引用计数）
 * - beforeCloseEdit()：编辑场景保存前，按当前 doc 的 attId 保留，清理用户删除的附件
 *
 * 使用方（PmItemDialog / PmProjectDialog / TodoDetailEdit）持有 editorRef 与 realId 的
 * getter，将 composable 返回的三个函数接入各自的提交/取消/关闭流程即可。
 */

export interface RichEditorExposed {
  getAttachmentIds: () => number[];
  getEffectiveOwnerId: () => string;
}

export type RichOwnerType = 'pm_project' | 'pm_item' | 'todo';

export interface UseRichDescriptionLifecycleOptions {
  ownerType: RichOwnerType;
  editorRef: Ref<RichEditorExposed | null>;
  getRealId: () => string | number | null | undefined;
}

export function useRichDescriptionLifecycle(opts: UseRichDescriptionLifecycleOptions) {
  async function afterSubmit(realId: string | number): Promise<void> {
    const editor = opts.editorRef.value;
    if (!editor) return;
    const tempId = editor.getEffectiveOwnerId();
    if (!tempId || !tempId.startsWith('tmp-')) return;
    await invokeToolByChannel('tool:attachments:rebind', {
      ownerType: opts.ownerType,
      fromOwnerId: tempId,
      toOwnerId: String(realId),
    });
  }

  async function onCancel(): Promise<void> {
    const editor = opts.editorRef.value;
    if (!editor) return;
    const ownerId = editor.getEffectiveOwnerId();
    if (!ownerId || !ownerId.startsWith('tmp-')) return;
    await invokeToolByChannel('tool:attachments:cleanup-orphans', {
      ownerType: opts.ownerType,
      ownerId,
      keepIds: [],
    });
  }

  async function beforeCloseEdit(): Promise<void> {
    const editor = opts.editorRef.value;
    if (!editor) return;
    const realId = opts.getRealId();
    if (realId == null || realId === '') return;
    await invokeToolByChannel('tool:attachments:cleanup-orphans', {
      ownerType: opts.ownerType,
      ownerId: String(realId),
      keepIds: editor.getAttachmentIds(),
    });
  }

  return { afterSubmit, onCancel, beforeCloseEdit };
}
