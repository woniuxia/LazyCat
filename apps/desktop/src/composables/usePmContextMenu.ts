import { ref } from "vue";
import type { CtxMenuAction, PmItem, PmItemStatus, PmSiyuanPageRef } from "../types/pm";
import { PM_STATUS_COLUMNS } from "../types/pm";

export interface PmContextMenuDeps {
  editItem: (item: PmItem) => void;
  openItemLink: (url: string | null | undefined) => Promise<void>;
  openSiyuanPage: (page: PmSiyuanPageRef | null | undefined) => Promise<void>;
  toggleItemPinFor: (item: PmItem) => Promise<void>;
  findNextStatus: (item: PmItem) => PmItemStatus | null;
  advanceItemStatusFor: (item: PmItem) => Promise<void>;
  deleteItemRecord: (item: PmItem) => Promise<void>;
}

export function usePmContextMenu(deps: PmContextMenuDeps) {
  const ctxMenuVisible = ref(false);
  const ctxMenuX = ref(0);
  const ctxMenuY = ref(0);
  const ctxMenuActions = ref<CtxMenuAction[]>([]);

  function buildItemContextActions(item: PmItem): CtxMenuAction[] {
    const actions: CtxMenuAction[] = [{ label: "编辑", action: () => deps.editItem(item) }];

    if (item.linkUrl) {
      actions.push({
        label: "打开链接",
        action: () => void deps.openItemLink(item.linkUrl),
      });
    }

    if (item.siyuanPrimaryPage) {
      actions.push({
        label: "打开思源主页面",
        action: () => void deps.openSiyuanPage(item.siyuanPrimaryPage),
      });
    }

    actions.push({
      label: item.pinned ? "取消置顶" : "置顶",
      action: () => void deps.toggleItemPinFor(item),
    });

    const nextStatus = deps.findNextStatus(item);
    if (nextStatus) {
      const nextLabel = PM_STATUS_COLUMNS.find((entry) => entry.key === nextStatus)?.label ?? nextStatus;
      actions.push({
        label: `推进到「${nextLabel}」`,
        action: () => void deps.advanceItemStatusFor(item),
      });
    }

    actions.push(
      { divider: true, label: "", action: () => {} },
      {
        label: "删除",
        danger: true,
        action: () => void deps.deleteItemRecord(item),
      },
    );

    return actions;
  }

  function openItemContextMenu(event: MouseEvent, item: PmItem) {
    openItemContextMenuAt(item, event.clientX, event.clientY);
  }

  function openItemContextMenuAt(item: PmItem, anchorX: number, anchorY: number) {
    ctxMenuActions.value = buildItemContextActions(item);
    ctxMenuX.value = anchorX;
    ctxMenuY.value = anchorY;
    ctxMenuVisible.value = true;
  }

  function openCtxMenu(event: MouseEvent, actions: CtxMenuAction[]) {
    ctxMenuActions.value = actions;
    ctxMenuX.value = event.clientX;
    ctxMenuY.value = event.clientY;
    ctxMenuVisible.value = true;
  }

  function closeCtxMenu() {
    ctxMenuVisible.value = false;
  }

  return {
    ctxMenuVisible,
    ctxMenuX,
    ctxMenuY,
    ctxMenuActions,
    buildItemContextActions,
    openItemContextMenu,
    openItemContextMenuAt,
    openCtxMenu,
    closeCtxMenu,
  };
}
