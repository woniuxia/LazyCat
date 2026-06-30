import { describe, expect, it } from "vitest";
import type { ApiWorkbenchCollection } from "../types/api-workbench";
import {
  buildApiWorkbenchNavMenuItems,
  buildApiWorkbenchFolderMoveTargets,
  buildApiWorkbenchRequestMoveTargets,
  buildApiWorkbenchTree,
  getApiWorkbenchFolderAncestorIds,
  moveApiWorkbenchOrderedId,
} from "./apiWorkbenchTree";

const collection: ApiWorkbenchCollection = {
  id: 1,
  name: "Demo",
  description: "",
  activeEnvironmentId: 10,
  folders: [
    { id: 2, collectionId: 1, parentId: null, name: "Admin", sortOrder: 1 },
    { id: 1, collectionId: 1, parentId: null, name: "Users", sortOrder: 0 },
    { id: 3, collectionId: 1, parentId: 1, name: "Profile", sortOrder: 0 },
  ],
  requests: [
    { id: 11, collectionId: 1, folderId: 1, name: "List users", method: "GET", url: "/users", sortOrder: 0 },
    { id: 10, collectionId: 1, folderId: null, name: "Health", method: "GET", url: "/health", sortOrder: 0 },
    { id: 12, collectionId: 1, folderId: 3, name: "Get profile", method: "GET", url: "/profile", sortOrder: 0 },
  ],
};

describe("apiWorkbenchTree", () => {
  it("builds roots, child folders, folder requests, and unassigned requests", () => {
    const tree = buildApiWorkbenchTree(collection);

    expect(tree.unassigned.requests.map((item) => item.name)).toEqual(["Health"]);
    expect(tree.roots.map((item) => item.name)).toEqual(["Users", "Admin"]);
    expect(tree.roots[0].children.map((item) => item.name)).toEqual(["Profile"]);
    expect(tree.roots[0].requests.map((item) => item.name)).toEqual(["List users"]);
    expect(tree.roots[0].children[0].requests.map((item) => item.name)).toEqual(["Get profile"]);
  });

  it("finds folder ancestors from root to parent", () => {
    expect(getApiWorkbenchFolderAncestorIds(collection.folders, 3)).toEqual([1]);
  });

  it("filters invalid folder move targets", () => {
    const targets = buildApiWorkbenchFolderMoveTargets(collection, 1);
    expect(targets.map((item) => item.folderId)).toEqual([null, 2]);
  });

  it("returns all request move targets including unassigned", () => {
    const targets = buildApiWorkbenchRequestMoveTargets(collection);
    expect(targets.map((item) => item.folderId)).toEqual([null, 1, 3, 2]);
  });

  it("moves ids up and down while preserving complete sibling order", () => {
    expect(moveApiWorkbenchOrderedId([1, 2, 3], 2, "up")).toEqual([2, 1, 3]);
    expect(moveApiWorkbenchOrderedId([1, 2, 3], 2, "down")).toEqual([1, 3, 2]);
    expect(moveApiWorkbenchOrderedId([1, 2, 3], 1, "up")).toEqual([1, 2, 3]);
  });

  it("offers create request actions from blank and folder menus", () => {
    expect(
      buildApiWorkbenchNavMenuItems({ type: "blank" }, { hasSelectedCollection: true }).map(
        (item) => item.key,
      ),
    ).toContain("request:create");
    expect(
      buildApiWorkbenchNavMenuItems(
        { type: "folder", collectionId: 1, folderId: 1 },
        { hasSelectedCollection: true },
      ).map((item) => item.key),
    ).toContain("request:create");
  });
});
