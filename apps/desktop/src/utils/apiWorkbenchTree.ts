import type {
  ApiWorkbenchCollection,
  ApiWorkbenchFolder,
  ApiWorkbenchMenuItem,
  ApiWorkbenchMoveTarget,
  ApiWorkbenchNavTarget,
  ApiWorkbenchOrderDirection,
  ApiWorkbenchTree,
  ApiWorkbenchTreeFolderNode,
  ApiWorkbenchTreeRequestNode,
} from "../types/api-workbench";

function bySortThenId<T extends { sortOrder: number; id: number }>(a: T, b: T): number {
  return a.sortOrder - b.sortOrder || a.id - b.id;
}

function indentLabel(name: string, depth: number): string {
  return `${"  ".repeat(depth)}${name}`;
}

export function buildApiWorkbenchTree(collection: ApiWorkbenchCollection): ApiWorkbenchTree {
  const foldersById = new Map<number, ApiWorkbenchTreeFolderNode>();
  const requestsById = new Map<number, ApiWorkbenchTreeRequestNode>();
  const roots: ApiWorkbenchTreeFolderNode[] = [];

  for (const folder of [...collection.folders].sort(bySortThenId)) {
    foldersById.set(folder.id, { ...folder, children: [], requests: [] });
  }

  for (const folder of [...foldersById.values()].sort(bySortThenId)) {
    if (folder.parentId === null) {
      roots.push(folder);
      continue;
    }

    const parent = foldersById.get(folder.parentId);
    if (parent) parent.children.push(folder);
    else roots.push(folder);
  }

  const unassigned: ApiWorkbenchTree["unassigned"] = {
    folderId: null,
    name: "未分组",
    requests: [] as ApiWorkbenchTreeRequestNode[],
  };
  for (const request of [...collection.requests].sort(bySortThenId)) {
    requestsById.set(request.id, request);
    if (request.folderId === null) {
      unassigned.requests.push(request);
      continue;
    }

    const folder = foldersById.get(request.folderId);
    if (folder) folder.requests.push(request);
    else unassigned.requests.push(request);
  }

  return { collectionId: collection.id, roots, unassigned, foldersById, requestsById };
}

export function getApiWorkbenchFolderAncestorIds(
  folders: ApiWorkbenchFolder[],
  folderId: number | null,
): number[] {
  if (folderId === null) return [];
  const byId = new Map(folders.map((folder) => [folder.id, folder]));
  const out: number[] = [];
  let current = byId.get(folderId)?.parentId ?? null;
  const seen = new Set<number>();
  while (current !== null && !seen.has(current)) {
    seen.add(current);
    out.unshift(current);
    current = byId.get(current)?.parentId ?? null;
  }
  return out;
}

function collectFolderTargets(
  node: ApiWorkbenchTreeFolderNode,
  depth: number,
  out: ApiWorkbenchMoveTarget[],
  blocked: Set<number>,
) {
  if (!blocked.has(node.id)) {
    out.push({ folderId: node.id, label: indentLabel(node.name, depth), depth });
  }
  for (const child of node.children) collectFolderTargets(child, depth + 1, out, blocked);
}

export function buildApiWorkbenchFolderMoveTargets(
  collection: ApiWorkbenchCollection,
  movingFolderId: number,
): ApiWorkbenchMoveTarget[] {
  const tree = buildApiWorkbenchTree(collection);
  const blocked = new Set<number>([movingFolderId]);
  const moving = tree.foldersById.get(movingFolderId);
  const markDescendants = (node: ApiWorkbenchTreeFolderNode) => {
    for (const child of node.children) {
      blocked.add(child.id);
      markDescendants(child);
    }
  };
  if (moving) markDescendants(moving);

  const out: ApiWorkbenchMoveTarget[] = [{ folderId: null, label: "根级", depth: 0 }];
  for (const root of tree.roots) collectFolderTargets(root, 0, out, blocked);
  return out;
}

export function buildApiWorkbenchRequestMoveTargets(
  collection: ApiWorkbenchCollection,
): ApiWorkbenchMoveTarget[] {
  const tree = buildApiWorkbenchTree(collection);
  const out: ApiWorkbenchMoveTarget[] = [{ folderId: null, label: "未分组", depth: 0 }];
  for (const root of tree.roots) collectFolderTargets(root, 0, out, new Set());
  return out;
}

export function moveApiWorkbenchOrderedId(
  ids: number[],
  id: number,
  direction: ApiWorkbenchOrderDirection,
): number[] {
  const index = ids.indexOf(id);
  if (index < 0) return ids;
  const target = direction === "up" ? index - 1 : index + 1;
  if (target < 0 || target >= ids.length) return ids;
  const next = [...ids];
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

export function buildApiWorkbenchNavMenuItems(
  target: ApiWorkbenchNavTarget,
  options: { hasSelectedCollection: boolean },
): ApiWorkbenchMenuItem[] {
  if (target.type === "blank") {
    return [
      { key: "collection:create", label: "新建集合" },
      {
        key: "request:create",
        label: "新建接口",
        disabled: !options.hasSelectedCollection,
      },
      {
        key: "folder:create-root",
        label: "新建根文件夹",
        disabled: !options.hasSelectedCollection,
      },
      { divider: true, key: "blank:tools", label: "" },
      { key: "request:import-curl", label: "导入 cURL" },
      {
        key: "collection:export",
        label: "导出 Markdown",
        disabled: !options.hasSelectedCollection,
      },
    ];
  }
  if (target.type === "collection") {
    return [
      { key: "collection:select", label: "选择集合" },
      { key: "request:create", label: "新建接口" },
      { key: "folder:create-root", label: "新建文件夹" },
      { divider: true, key: "collection:tools", label: "" },
      { key: "request:import-curl", label: "导入 cURL" },
      { key: "collection:export", label: "导出 Markdown" },
      { divider: true, key: "collection:manage", label: "" },
      { key: "collection:rename", label: "重命名" },
      { key: "collection:delete", label: "删除", danger: true },
    ];
  }
  if (target.type === "folder") {
    return [
      { key: "request:create", label: "新建接口" },
      { key: "folder:create-child", label: "新建子文件夹" },
      { divider: true, key: "folder:tools", label: "" },
      { key: "request:import-curl", label: "导入 cURL" },
      { key: "collection:export", label: "导出 Markdown" },
      { divider: true, key: "folder:manage", label: "" },
      { key: "folder:rename", label: "重命名" },
      { key: "folder:move", label: "移动到" },
      { key: "folder:up", label: "上移" },
      { key: "folder:down", label: "下移" },
      { key: "folder:delete", label: "删除", danger: true },
    ];
  }
  return [
    { key: "request:open", label: "打开" },
    { key: "request:import-curl", label: "导入 cURL" },
    { key: "collection:export", label: "导出 Markdown" },
    { divider: true, key: "request:manage", label: "" },
    { key: "request:rename", label: "重命名" },
    { key: "request:move", label: "移动到" },
    { key: "request:up", label: "上移" },
    { key: "request:down", label: "下移" },
    { key: "request:delete", label: "删除", danger: true },
  ];
}
