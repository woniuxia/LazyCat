import type {
  PmSiyuanLocation,
  PmSiyuanNotebookDirectory,
  PmSiyuanPageRef,
  PmSiyuanTreeNode,
} from "../types/pm";

type PmSiyuanDirectoryNode = PmSiyuanNotebookDirectory | PmSiyuanTreeNode;

export function resolvePmSiyuanEffectiveLocation(
  projectLocation: PmSiyuanLocation | null | undefined,
  globalLocation: PmSiyuanLocation | null | undefined,
): PmSiyuanLocation | null {
  return projectLocation ?? globalLocation ?? null;
}

export function formatPmSiyuanLocationLabel(
  location: PmSiyuanLocation | null | undefined,
): string {
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

function matchesPmSiyuanDirectoryNode(
  node: PmSiyuanDirectoryNode,
  keyword: string,
): boolean {
  const fields = isPmSiyuanNotebookDirectory(node)
    ? [node.name]
    : [node.name, node.hpath, node.path ?? ""];
  return fields.some((field) => field.toLowerCase().includes(keyword));
}

function filterPmSiyuanTreeNode(
  node: PmSiyuanTreeNode,
  keyword: string,
): PmSiyuanTreeNode | null {
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

export function collectPmSiyuanExpandedKeys(
  notebooks: PmSiyuanNotebookDirectory[],
): string[] {
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
