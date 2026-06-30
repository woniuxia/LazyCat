import { describe, expect, it } from "vitest";
import type { ApiWorkbenchCollection } from "../types/api-workbench";
import { filterApiWorkbenchCollection } from "./apiWorkbenchSearch";

const collection: ApiWorkbenchCollection = {
  id: 1,
  name: "Demo",
  description: "",
  activeEnvironmentId: 10,
  folders: [
    { id: 1, collectionId: 1, parentId: null, name: "Users", sortOrder: 0 },
    { id: 2, collectionId: 1, parentId: 1, name: "Profile", sortOrder: 0 },
    { id: 3, collectionId: 1, parentId: null, name: "Orders", sortOrder: 1 },
  ],
  requests: [
    { id: 10, collectionId: 1, folderId: null, name: "Health", method: "GET", url: "/health", sortOrder: 0 },
    { id: 11, collectionId: 1, folderId: 1, name: "List users", method: "GET", url: "/users", sortOrder: 0 },
    { id: 12, collectionId: 1, folderId: 2, name: "Get profile", method: "GET", url: "/profile", sortOrder: 0 },
    { id: 13, collectionId: 1, folderId: 3, name: "Create order", method: "POST", url: "/orders", sortOrder: 0 },
  ],
};

describe("apiWorkbenchSearch", () => {
  it("filters requests by name, method, and url while keeping ancestor folders", () => {
    const filtered = filterApiWorkbenchCollection(collection, "profile");

    expect(filtered.requests.map((item) => item.id)).toEqual([12]);
    expect(filtered.folders.map((item) => item.id)).toEqual([1, 2]);
  });

  it("keeps folder direct structure when folder name matches", () => {
    const filtered = filterApiWorkbenchCollection(collection, "users");

    expect(filtered.folders.map((item) => item.id)).toEqual([1, 2]);
    expect(filtered.requests.map((item) => item.id)).toEqual([11]);
  });

  it("returns original collection for blank query", () => {
    expect(filterApiWorkbenchCollection(collection, "   ")).toBe(collection);
  });
});
