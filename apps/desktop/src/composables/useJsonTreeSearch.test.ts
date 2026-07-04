import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ref } from "vue";
import { buildJsonTree, encodeJsonTreePath } from "../utils/jsonTreeView";
import { JSON_TREE_SEARCH_DEBOUNCE_MS, useJsonTreeSearch } from "./useJsonTreeSearch";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useJsonTreeSearch", () => {
  it("debounces query input and lands on the first match", () => {
    const tree = ref(buildJsonTree({ user: { name: "Alice" }, alias: "bob" }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "ali";
    expect(search.matches.value).toEqual([]);
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS - 1);
    expect(search.matches.value).toEqual([]);
    vi.advanceTimersByTime(1);

    expect(search.matches.value.map((m) => [m.key, m.field])).toEqual([
      [encodeJsonTreePath(["user", "name"]), "value"],
      [encodeJsonTreePath(["alias"]), "key"],
    ]);
    expect(search.activeIndex.value).toBe(0);
    expect(search.activeKey.value).toBe(encodeJsonTreePath(["user", "name"]));
  });

  it("restarts the debounce window when the query keeps changing", () => {
    const tree = ref(buildJsonTree({ alias: "bob" }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "ali";
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS - 1);
    search.query.value = "alias";
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS - 1);
    expect(search.matches.value).toEqual([]);
    vi.advanceTimersByTime(1);

    expect(search.matches.value).toEqual([
      { key: encodeJsonTreePath(["alias"]), path: ["alias"], field: "key" },
    ]);
  });

  it("cycles matches with goNext and goPrev", () => {
    const tree = ref(buildJsonTree({ alpha: "x", beta: "alpha", gamma: { alpha: 1 } }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "alpha";
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS);
    expect(search.matches.value.length).toBe(3);
    expect(search.activeIndex.value).toBe(0);

    search.goNext();
    expect(search.activeIndex.value).toBe(1);
    search.goNext();
    expect(search.activeIndex.value).toBe(2);
    search.goNext();
    expect(search.activeIndex.value).toBe(0);
    search.goPrev();
    expect(search.activeIndex.value).toBe(2);
    expect(search.activeKey.value).toBe(encodeJsonTreePath(["gamma", "alpha"]));
  });

  it("flushes a pending debounce on goNext without skipping the first match", () => {
    const tree = ref(buildJsonTree({ first: "hit", second: "hit" }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "hit";
    search.goNext();

    expect(search.matches.value.length).toBe(2);
    expect(search.activeIndex.value).toBe(0);
  });

  it("does nothing on navigation when there are no matches", () => {
    const tree = ref(buildJsonTree({ first: "hit" }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "missing";
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS);
    search.goNext();
    search.goPrev();

    expect(search.matches.value).toEqual([]);
    expect(search.activeIndex.value).toBe(-1);
    expect(search.activeKey.value).toBeNull();
  });

  it("keeps pointing at the active match when the tree changes and its key survives", () => {
    const tree = ref(buildJsonTree({ first: "hit", second: "hit" }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "hit";
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS);
    search.goNext();
    expect(search.activeKey.value).toBe(encodeJsonTreePath(["second"]));

    tree.value = buildJsonTree({ second: "hit", zero: "hit" });

    expect(search.matches.value.length).toBe(2);
    expect(search.activeKey.value).toBe(encodeJsonTreePath(["second"]));
    expect(search.activeIndex.value).toBe(0);
  });

  it("falls back to the first match when the active key disappears", () => {
    const tree = ref(buildJsonTree({ first: "hit", second: "hit" }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "hit";
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS);
    search.goNext();
    expect(search.activeKey.value).toBe(encodeJsonTreePath(["second"]));

    tree.value = buildJsonTree({ alpha: "hit", beta: "hit" });

    expect(search.activeIndex.value).toBe(0);
    expect(search.activeKey.value).toBe(encodeJsonTreePath(["alpha"]));
  });

  it("clears matches immediately when the query is emptied", () => {
    const tree = ref(buildJsonTree({ first: "hit" }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "hit";
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS);
    expect(search.matches.value.length).toBe(1);

    search.query.value = "";

    expect(search.matches.value).toEqual([]);
    expect(search.activeIndex.value).toBe(-1);
  });

  it("exposes matched ids and ancestor reveal keys for the active match", () => {
    const tree = ref(buildJsonTree({ user: { profile: { city: "Paris" } } }));
    const search = useJsonTreeSearch(tree);

    search.query.value = "paris";
    vi.advanceTimersByTime(JSON_TREE_SEARCH_DEBOUNCE_MS);

    const cityKey = encodeJsonTreePath(["user", "profile", "city"]);
    expect(search.matchedIds.value).toEqual(new Set([`value:${cityKey}`]));
    expect(search.activeMatchId.value).toBe(`value:${cityKey}`);
    expect(search.revealKeys.value).toEqual(
      new Set([
        encodeJsonTreePath([]),
        encodeJsonTreePath(["user"]),
        encodeJsonTreePath(["user", "profile"]),
      ]),
    );

    search.query.value = "";
    expect(search.matchedIds.value).toEqual(new Set());
    expect(search.activeMatchId.value).toBeNull();
    expect(search.revealKeys.value).toEqual(new Set());
  });
});
