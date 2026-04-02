import { describe, expect, it } from "vitest";
import type {
  PmSiyuanLocation,
  PmSiyuanNotebookDirectory,
  PmSiyuanPageRef,
} from "../types/pm";
import {
  addPmSiyuanExtraPage,
  collectPmSiyuanPagesForLocation,
  collectPmSiyuanExpandedKeys,
  filterPmSiyuanDirectory,
  filterPmSiyuanPages,
  formatPmSiyuanLocationLabel,
  formatPmSiyuanLocationPathLabel,
  formatPmSiyuanLocationTargetLabel,
  removePmSiyuanPage,
  resolvePmSiyuanEffectiveLocation,
  setPmSiyuanPrimaryPage,
} from "./pmSiyuan";

const rootLocation: PmSiyuanLocation = {
  notebookId: "nb-root",
  notebookName: "研发笔记",
  parentDocId: null,
  parentDocTitle: null,
  parentHpath: null,
  parentPath: null,
};

const childLocation: PmSiyuanLocation = {
  notebookId: "nb-root",
  notebookName: "研发笔记",
  parentDocId: "doc-parent",
  parentDocTitle: "项目页",
  parentHpath: "/项目页",
  parentPath: "/doc-parent.sy",
};

function createPage(docId: string, title: string): PmSiyuanPageRef {
  return {
    docId,
    docTitle: title,
    docHpath: `/${title}`,
    docPath: `/${docId}.sy`,
    notebookId: "nb-root",
    notebookName: "研发笔记",
  };
}

function createNotebookTree(): PmSiyuanNotebookDirectory[] {
  return [
    {
      id: "nb-root",
      name: "研发笔记",
      icon: null,
      closed: false,
      docCount: 3,
      children: [
        {
          id: "doc-parent",
          name: "项目页",
          hpath: "/项目页",
          path: "/doc-parent.sy",
          leaf: false,
          children: [
            {
              id: "doc-child",
              name: "需求整理",
              hpath: "/项目页/需求整理",
              path: "/doc-child.sy",
              leaf: true,
              children: [],
            },
          ],
        },
        {
          id: "doc-alone",
          name: "迭代记录",
          hpath: "/迭代记录",
          path: "/doc-alone.sy",
          leaf: true,
          children: [],
        },
      ],
    },
    {
      id: "nb-other",
      name: "产品笔记",
      icon: null,
      closed: false,
      docCount: 1,
      children: [
        {
          id: "doc-parent",
          name: "其它项目页",
          hpath: "/其它项目页",
          path: "/other-parent.sy",
          leaf: true,
          children: [],
        },
      ],
    },
    {
      id: "nb-empty",
      name: "空笔记本",
      icon: null,
      closed: false,
      docCount: 0,
      children: [],
    },
  ];
}

describe("pmSiyuan utils", () => {
  it("prefers project override over global location", () => {
    expect(resolvePmSiyuanEffectiveLocation(childLocation, rootLocation)).toEqual(childLocation);
    expect(resolvePmSiyuanEffectiveLocation(null, rootLocation)).toEqual(rootLocation);
  });

  it("formats root and child locations", () => {
    expect(formatPmSiyuanLocationLabel(rootLocation)).toBe("研发笔记 · 笔记本根目录");
    expect(formatPmSiyuanLocationLabel(childLocation)).toBe("研发笔记 · /项目页");
    expect(formatPmSiyuanLocationTargetLabel(rootLocation)).toBe("笔记本根目录");
    expect(formatPmSiyuanLocationTargetLabel(childLocation)).toBe("项目页");
    expect(formatPmSiyuanLocationPathLabel(rootLocation)).toBe("/");
    expect(formatPmSiyuanLocationPathLabel(childLocation)).toBe("/项目页");
  });

  it("promotes an extra page to primary and demotes old primary", () => {
    const primary = createPage("doc-a", "主页面");
    const extra = createPage("doc-b", "附加页面");
    const result = setPmSiyuanPrimaryPage(primary, [extra], extra);

    expect(result.primaryPage?.docId).toBe("doc-b");
    expect(result.extraPages.map((page) => page.docId)).toEqual(["doc-a"]);
  });

  it("dedupes extra pages against primary page", () => {
    const primary = createPage("doc-a", "主页面");
    const duplicate = createPage("doc-a", "主页面");
    const extra = createPage("doc-b", "附加页面");

    expect(addPmSiyuanExtraPage(primary, [duplicate], extra).map((page) => page.docId)).toEqual([
      "doc-b",
    ]);
  });

  it("removes primary page without auto-promoting extras", () => {
    const primary = createPage("doc-a", "主页面");
    const extra = createPage("doc-b", "附加页面");
    const result = removePmSiyuanPage(primary, [extra], "doc-a");

    expect(result.primaryPage).toBeNull();
    expect(result.extraPages.map((page) => page.docId)).toEqual(["doc-b"]);
  });

  it("filters directory tree while preserving ancestor chain", () => {
    const filtered = filterPmSiyuanDirectory(createNotebookTree(), "需求");

    expect(filtered).toHaveLength(1);
    expect(filtered[0].children).toHaveLength(1);
    expect(filtered[0].children[0].id).toBe("doc-parent");
    expect(filtered[0].children[0].children.map((node) => node.id)).toEqual(["doc-child"]);
  });

  it("keeps matched notebook node without forcing all descendants", () => {
    const filtered = filterPmSiyuanDirectory(createNotebookTree(), "研发");

    expect(filtered).toHaveLength(1);
    expect(filtered[0].id).toBe("nb-root");
    expect(filtered[0].children).toHaveLength(0);
  });

  it("collects expanded keys for visible notebook branches", () => {
    const filtered = filterPmSiyuanDirectory(createNotebookTree(), "需求");

    expect(collectPmSiyuanExpandedKeys(filtered)).toEqual(["nb-root", "doc-parent"]);
  });

  it("collects all pages for a notebook root location", () => {
    const result = collectPmSiyuanPagesForLocation(createNotebookTree(), rootLocation);

    expect(result.state).toBe("ready");
    expect(result.pages.map((page) => page.docId)).toEqual(["doc-parent", "doc-child", "doc-alone"]);
  });

  it("collects parent page and descendants for a child location", () => {
    const result = collectPmSiyuanPagesForLocation(createNotebookTree(), childLocation);

    expect(result.state).toBe("ready");
    expect(result.pages.map((page) => page.docId)).toEqual(["doc-parent", "doc-child"]);
  });

  it("filters pages by title, hpath, and path", () => {
    const pages = [
      createPage("doc-a", "需求整理"),
      {
        ...createPage("doc-b", "周报"),
        docHpath: "/项目页/里程碑",
      },
      {
        ...createPage("doc-c", "归档"),
        docPath: "/archive/doc-c.sy",
      },
    ];

    expect(filterPmSiyuanPages(pages, "需求").map((page) => page.docId)).toEqual(["doc-a"]);
    expect(filterPmSiyuanPages(pages, "里程碑").map((page) => page.docId)).toEqual(["doc-b"]);
    expect(filterPmSiyuanPages(pages, "archive").map((page) => page.docId)).toEqual(["doc-c"]);
  });

  it("returns the original page list when the filter keyword is empty", () => {
    const pages = [createPage("doc-a", "需求整理"), createPage("doc-b", "周报")];

    expect(filterPmSiyuanPages(pages, "   ")).toBe(pages);
  });

  it("locates parent pages by notebook id and parent doc id together", () => {
    const location: PmSiyuanLocation = {
      notebookId: "nb-other",
      notebookName: "产品笔记",
      parentDocId: "doc-parent",
      parentDocTitle: "其它项目页",
      parentHpath: "/其它项目页",
      parentPath: "/other-parent.sy",
    };

    const result = collectPmSiyuanPagesForLocation(createNotebookTree(), location);

    expect(result.state).toBe("ready");
    expect(result.pages.map((page) => page.docId)).toEqual(["doc-parent"]);
    expect(result.pages[0]?.notebookId).toBe("nb-other");
  });

  it("returns invalid-location when notebook is closed or target node is missing", () => {
    const closedNotebookTree = createNotebookTree().map((notebook) =>
      notebook.id === "nb-root" ? { ...notebook, closed: true } : notebook,
    );
    const missingNodeLocation: PmSiyuanLocation = {
      ...childLocation,
      parentDocId: "doc-missing",
    };

    expect(collectPmSiyuanPagesForLocation(closedNotebookTree, rootLocation)).toEqual({
      state: "invalid-location",
      pages: [],
    });
    expect(collectPmSiyuanPagesForLocation(createNotebookTree(), missingNodeLocation)).toEqual({
      state: "invalid-location",
      pages: [],
    });
  });

  it("returns empty when the location is valid but contains no documents", () => {
    const emptyLocation: PmSiyuanLocation = {
      notebookId: "nb-empty",
      notebookName: "空笔记本",
      parentDocId: null,
      parentDocTitle: null,
      parentHpath: null,
      parentPath: null,
    };

    expect(collectPmSiyuanPagesForLocation(createNotebookTree(), emptyLocation)).toEqual({
      state: "empty",
      pages: [],
    });
  });
});
