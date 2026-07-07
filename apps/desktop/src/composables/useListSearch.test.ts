import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { effectScope, ref, watch } from "vue";
import { useDebouncedKeyword, useListSearch } from "./useListSearch";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useDebouncedKeyword", () => {
  it("updates debouncedKeyword after 300ms with a trimmed value", () => {
    const search = useDebouncedKeyword();

    search.keyword.value = "  alpha  ";

    expect(search.debouncedKeyword.value).toBe("");
    vi.advanceTimersByTime(299);
    expect(search.debouncedKeyword.value).toBe("");
    vi.advanceTimersByTime(1);
    expect(search.debouncedKeyword.value).toBe("alpha");
  });

  it("restarts the debounce window and applies only the last input", () => {
    const search = useDebouncedKeyword();
    const changes: string[] = [];
    watch(search.debouncedKeyword, (value) => changes.push(value), { flush: "sync" });

    search.keyword.value = "alpha";
    vi.advanceTimersByTime(299);
    search.keyword.value = "beta";
    vi.advanceTimersByTime(299);

    expect(search.debouncedKeyword.value).toBe("");
    expect(changes).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(search.debouncedKeyword.value).toBe("beta");
    expect(changes).toEqual(["beta"]);
  });

  it("clears pending timers when the owning scope is disposed", () => {
    const scope = effectScope();
    let search: ReturnType<typeof useDebouncedKeyword> | undefined;
    scope.run(() => {
      search = useDebouncedKeyword();
    });
    expect(search).toBeDefined();

    search!.keyword.value = "stale";
    scope.stop();
    vi.advanceTimersByTime(300);

    expect(search!.debouncedKeyword.value).toBe("");
  });
});

describe("useListSearch", () => {
  type Row = { name: string; path: string };

  const rows = [
    { name: "Alpha", path: "C:/tools/alpha.exe" },
    { name: "Beta", path: "D:/apps/beta.exe" },
  ];

  function matches(row: Row, keyword: string) {
    const normalized = keyword.toLowerCase();
    return row.name.toLowerCase().includes(normalized) || row.path.toLowerCase().includes(normalized);
  }

  it("returns all items for empty or whitespace-only keywords", () => {
    const source = ref(rows);
    const search = useListSearch(() => source.value, matches);

    expect(search.filtered.value).toEqual(rows);
    search.keyword.value = "   ";
    vi.advanceTimersByTime(300);
    expect(search.debouncedKeyword.value).toBe("");
    expect(search.filtered.value).toEqual(rows);
  });

  it("filters with the matcher and supports custom debounceMs", () => {
    const source = ref(rows);
    const search = useListSearch(() => source.value, matches, { debounceMs: 50 });

    search.keyword.value = "D:/apps";
    vi.advanceTimersByTime(49);
    expect(search.filtered.value).toEqual(rows);
    vi.advanceTimersByTime(1);
    expect(search.filtered.value).toEqual([rows[1]]);
  });
});
