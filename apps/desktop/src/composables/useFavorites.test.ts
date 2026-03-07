import { describe, expect, it, vi } from "vitest";

vi.mock("element-plus", () => ({
  ElMessage: {
    success: vi.fn(),
  },
}));

vi.mock("./useSettings", () => ({
  getSettingJson: vi.fn(),
  setSettingJson: vi.fn(),
}));

import {
  bootstrapFavoriteToolIds,
  normalizeFavoriteToolIds,
  TODO_TOOL_ID,
} from "./useFavorites";

const knownToolIds = new Set([TODO_TOOL_ID, "formatter", "snippets", "vault"]);

function isRealToolId(id: string) {
  return knownToolIds.has(id);
}

describe("normalizeFavoriteToolIds", () => {
  it("filters invalid ids and removes duplicates", () => {
    expect(
      normalizeFavoriteToolIds([TODO_TOOL_ID, "formatter", "missing", TODO_TOOL_ID, 123], isRealToolId),
    ).toEqual([TODO_TOOL_ID, "formatter"]);
  });
});

describe("bootstrapFavoriteToolIds", () => {
  it("seeds todo to the first position on first bootstrap", () => {
    expect(
      bootstrapFavoriteToolIds(["formatter", "snippets"], false, isRealToolId),
    ).toEqual({
      favoriteToolIds: [TODO_TOOL_ID, "formatter", "snippets"],
      shouldMarkTodoSeeded: true,
    });
  });

  it("does not duplicate todo and still marks bootstrap as completed", () => {
    expect(
      bootstrapFavoriteToolIds(["formatter", TODO_TOOL_ID], false, isRealToolId),
    ).toEqual({
      favoriteToolIds: ["formatter", TODO_TOOL_ID],
      shouldMarkTodoSeeded: true,
    });
  });

  it("does not re-add todo after the bootstrap flag is already set", () => {
    expect(
      bootstrapFavoriteToolIds(["formatter", "snippets"], true, isRealToolId),
    ).toEqual({
      favoriteToolIds: ["formatter", "snippets"],
      shouldMarkTodoSeeded: false,
    });
  });

  it("skips todo seeding when todo is not a real tool", () => {
    expect(
      bootstrapFavoriteToolIds(["formatter", "snippets"], false, (id) => id !== TODO_TOOL_ID),
    ).toEqual({
      favoriteToolIds: ["formatter", "snippets"],
      shouldMarkTodoSeeded: false,
    });
  });
});
