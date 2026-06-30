import type { ApiWorkbenchCollection, ApiWorkbenchFolder } from "../types/api-workbench";
import { getApiWorkbenchFolderAncestorIds } from "./apiWorkbenchTree";

function includesKeyword(value: string, keyword: string): boolean {
  return value.toLowerCase().includes(keyword);
}

function addFolderWithAncestors(
  folders: ApiWorkbenchFolder[],
  folderId: number | null,
  out: Set<number>,
) {
  for (const id of getApiWorkbenchFolderAncestorIds(folders, folderId)) out.add(id);
  if (folderId !== null) out.add(folderId);
}

export function filterApiWorkbenchCollection(
  collection: ApiWorkbenchCollection,
  query: string,
): ApiWorkbenchCollection {
  const keyword = query.trim().toLowerCase();
  if (!keyword) return collection;

  const folderIds = new Set<number>();
  const requestIds = new Set<number>();

  for (const request of collection.requests) {
    const matched =
      includesKeyword(request.name, keyword) ||
      includesKeyword(request.method, keyword) ||
      includesKeyword(request.url, keyword);
    if (!matched) continue;
    requestIds.add(request.id);
    addFolderWithAncestors(collection.folders, request.folderId, folderIds);
  }

  for (const folder of collection.folders) {
    if (!includesKeyword(folder.name, keyword)) continue;
    addFolderWithAncestors(collection.folders, folder.id, folderIds);
    for (const child of collection.folders.filter((item) => item.parentId === folder.id)) {
      folderIds.add(child.id);
    }
    for (const request of collection.requests.filter((item) => item.folderId === folder.id)) {
      requestIds.add(request.id);
    }
  }

  return {
    ...collection,
    folders: collection.folders.filter((folder) => folderIds.has(folder.id)),
    requests: collection.requests.filter((request) => requestIds.has(request.id)),
  };
}
