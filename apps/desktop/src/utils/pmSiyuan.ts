import type {
  PmSiyuanLocation,
  PmSiyuanNotebookDirectory,
  PmSiyuanPageRef,
  PmSiyuanTreeNode,
} from "../types/pm";

type PmSiyuanDirectoryNode = PmSiyuanNotebookDirectory | PmSiyuanTreeNode;

export type PmSiyuanLocationPagesResult =
  | { state: "ready"; pages: PmSiyuanPageRef[] }
  | { state: "empty"; pages: [] }
  | { state: "invalid-location"; pages: [] };

export function resolvePmSiyuanEffectiveLocation(
  projectLocation: PmSiyuanLocation | null | undefined,
  globalLocation: PmSiyuanLocation | null | undefined,
): PmSiyuanLocation | null {
  return projectLocation ?? globalLocation ?? null;
}

export function formatPmSiyuanLocationLabel(location: PmSiyuanLocation | null | undefined): string {
  if (!location) {
    return "未设置";
  }
  if (location.parentHpath) {
    return `${location.notebookName} · ${location.parentHpath}`;
  }
  return `${location.notebookName} · 笔记本根目录`;
}

export function formatPmSiyuanLocationTargetLabel(
  location: PmSiyuanLocation | null | undefined,
): string {
  if (!location) {
    return "未选择位置";
  }
  return location.parentDocTitle || "笔记本根目录";
}

export function formatPmSiyuanLocationPathLabel(
  location: PmSiyuanLocation | null | undefined,
): string {
  if (!location) {
    return "-";
  }
  return location.parentHpath || "/";
}

export function isPmSiyuanNotebookDirectory(
  node: PmSiyuanDirectoryNode,
): node is PmSiyuanNotebookDirectory {
  return !("hpath" in node);
}

function normalizePmSiyuanKeyword(keyword: string): string {
  return keyword.trim().toLowerCase();
}

function matchesPmSiyuanDirectoryNode(node: PmSiyuanDirectoryNode, keyword: string): boolean {
  const fields = isPmSiyuanNotebookDirectory(node)
    ? [node.name]
    : [node.name, node.hpath, node.path ?? ""];
  return fields.some((field) => field.toLowerCase().includes(keyword));
}

function filterPmSiyuanTreeNode(node: PmSiyuanTreeNode, keyword: string): PmSiyuanTreeNode | null {
  const children = node.children
    .map((child) => filterPmSiyuanTreeNode(child, keyword))
    .filter((child): child is PmSiyuanTreeNode => Boolean(child));
  const matched = matchesPmSiyuanDirectoryNode(node, keyword);
  if (!matched && children.length === 0) {
    return null;
  }
  return {
    ...node,
    children,
  };
}

export function filterPmSiyuanDirectory(
  notebooks: PmSiyuanNotebookDirectory[],
  keyword: string,
): PmSiyuanNotebookDirectory[] {
  const normalizedKeyword = normalizePmSiyuanKeyword(keyword);
  if (!normalizedKeyword) {
    return notebooks;
  }

  return notebooks
    .map((notebook) => {
      const children = notebook.children
        .map((child) => filterPmSiyuanTreeNode(child, normalizedKeyword))
        .filter((child): child is PmSiyuanTreeNode => Boolean(child));
      const matched = matchesPmSiyuanDirectoryNode(notebook, normalizedKeyword);
      if (!matched && children.length === 0) {
        return null;
      }
      return {
        ...notebook,
        children,
      };
    })
    .filter((notebook): notebook is PmSiyuanNotebookDirectory => Boolean(notebook));
}

export function collectPmSiyuanExpandedKeys(notebooks: PmSiyuanNotebookDirectory[]): string[] {
  const keys = new Set<string>();

  function walk(nodes: PmSiyuanTreeNode[]) {
    for (const node of nodes) {
      if (node.children.length > 0) {
        keys.add(node.id);
        walk(node.children);
      }
    }
  }

  for (const notebook of notebooks) {
    if (notebook.children.length > 0) {
      keys.add(notebook.id);
      walk(notebook.children);
    }
  }

  return [...keys];
}

function createPmSiyuanPageRef(
  notebook: PmSiyuanNotebookDirectory,
  node: PmSiyuanTreeNode,
): PmSiyuanPageRef {
  return {
    docId: node.id,
    docTitle: node.name,
    docHpath: node.hpath,
    docPath: node.path,
    notebookId: notebook.id,
    notebookName: notebook.name,
  };
}

function appendPmSiyuanPageRef(
  result: PmSiyuanPageRef[],
  seen: Set<string>,
  notebook: PmSiyuanNotebookDirectory,
  node: PmSiyuanTreeNode,
) {
  if (!node.id || seen.has(node.id)) {
    return;
  }
  seen.add(node.id);
  result.push(createPmSiyuanPageRef(notebook, node));
}

function flattenPmSiyuanTreeNodes(
  result: PmSiyuanPageRef[],
  seen: Set<string>,
  notebook: PmSiyuanNotebookDirectory,
  nodes: PmSiyuanTreeNode[],
) {
  for (const node of nodes) {
    appendPmSiyuanPageRef(result, seen, notebook, node);
    if (node.children.length > 0) {
      flattenPmSiyuanTreeNodes(result, seen, notebook, node.children);
    }
  }
}

function collectPmSiyuanPagesFromNode(
  notebook: PmSiyuanNotebookDirectory,
  target: PmSiyuanTreeNode,
): PmSiyuanPageRef[] {
  const result: PmSiyuanPageRef[] = [];
  const seen = new Set<string>();
  appendPmSiyuanPageRef(result, seen, notebook, target);
  if (target.children.length > 0) {
    flattenPmSiyuanTreeNodes(result, seen, notebook, target.children);
  }
  return result;
}

function collectPmSiyuanPagesFromNotebook(notebook: PmSiyuanNotebookDirectory): PmSiyuanPageRef[] {
  const result: PmSiyuanPageRef[] = [];
  flattenPmSiyuanTreeNodes(result, new Set<string>(), notebook, notebook.children);
  return result;
}

function findPmSiyuanTreeNodeById(
  nodes: PmSiyuanTreeNode[],
  targetId: string,
): PmSiyuanTreeNode | null {
  for (const node of nodes) {
    if (node.id === targetId) {
      return node;
    }
    const child = findPmSiyuanTreeNodeById(node.children, targetId);
    if (child) {
      return child;
    }
  }
  return null;
}

export function collectPmSiyuanPagesForLocation(
  notebooks: PmSiyuanNotebookDirectory[],
  location: PmSiyuanLocation | null | undefined,
): PmSiyuanLocationPagesResult {
  if (!location) {
    return { state: "invalid-location", pages: [] };
  }

  const notebook = notebooks.find((entry) => entry.id === location.notebookId);
  if (!notebook || notebook.closed) {
    return { state: "invalid-location", pages: [] };
  }

  const pages = location.parentDocId
    ? (() => {
        const target = findPmSiyuanTreeNodeById(notebook.children, location.parentDocId);
        if (!target) {
          return null;
        }
        return collectPmSiyuanPagesFromNode(notebook, target);
      })()
    : collectPmSiyuanPagesFromNotebook(notebook);

  if (!pages) {
    return { state: "invalid-location", pages: [] };
  }

  if (pages.length === 0) {
    return { state: "empty", pages: [] };
  }

  return { state: "ready", pages };
}

export function filterPmSiyuanPages(pages: PmSiyuanPageRef[], keyword: string): PmSiyuanPageRef[] {
  const normalizedKeyword = normalizePmSiyuanKeyword(keyword);
  if (!normalizedKeyword) {
    return pages;
  }

  return pages.filter((page) =>
    [page.docTitle, page.docHpath, page.docPath ?? ""].some((field) =>
      field.toLowerCase().includes(normalizedKeyword),
    ),
  );
}

export function dedupePmSiyuanExtraPages(
  primaryPage: PmSiyuanPageRef | null | undefined,
  extraPages: PmSiyuanPageRef[],
): PmSiyuanPageRef[] {
  const seen = new Set<string>();
  if (primaryPage?.docId) {
    seen.add(primaryPage.docId);
  }

  const result: PmSiyuanPageRef[] = [];
  for (const page of extraPages) {
    if (!page.docId || seen.has(page.docId)) {
      continue;
    }
    seen.add(page.docId);
    result.push(page);
  }
  return result;
}

export function setPmSiyuanPrimaryPage(
  currentPrimary: PmSiyuanPageRef | null,
  currentExtraPages: PmSiyuanPageRef[],
  nextPrimary: PmSiyuanPageRef | null,
): { primaryPage: PmSiyuanPageRef | null; extraPages: PmSiyuanPageRef[] } {
  if (!nextPrimary) {
    return {
      primaryPage: null,
      extraPages: dedupePmSiyuanExtraPages(null, currentExtraPages),
    };
  }

  const nextExtraPages = currentExtraPages.filter((page) => page.docId !== nextPrimary.docId);
  if (currentPrimary && currentPrimary.docId !== nextPrimary.docId) {
    nextExtraPages.unshift(currentPrimary);
  }

  return {
    primaryPage: nextPrimary,
    extraPages: dedupePmSiyuanExtraPages(nextPrimary, nextExtraPages),
  };
}

export function addPmSiyuanExtraPage(
  currentPrimary: PmSiyuanPageRef | null,
  currentExtraPages: PmSiyuanPageRef[],
  page: PmSiyuanPageRef,
): PmSiyuanPageRef[] {
  return dedupePmSiyuanExtraPages(currentPrimary, [...currentExtraPages, page]);
}

export function removePmSiyuanPage(
  currentPrimary: PmSiyuanPageRef | null,
  currentExtraPages: PmSiyuanPageRef[],
  docId: string,
): { primaryPage: PmSiyuanPageRef | null; extraPages: PmSiyuanPageRef[] } {
  if (currentPrimary?.docId === docId) {
    return {
      primaryPage: null,
      extraPages: dedupePmSiyuanExtraPages(null, currentExtraPages),
    };
  }

  return {
    primaryPage: currentPrimary,
    extraPages: currentExtraPages.filter((page) => page.docId !== docId),
  };
}
