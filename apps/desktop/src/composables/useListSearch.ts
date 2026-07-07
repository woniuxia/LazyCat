import { computed, getCurrentScope, onScopeDispose, readonly, ref, watch } from "vue";
import type { ComputedRef, Ref } from "vue";

type SearchOptions = {
  debounceMs?: number;
};

const DEFAULT_DEBOUNCE_MS = 300;

export function useDebouncedKeyword(options: SearchOptions = {}): {
  keyword: Ref<string>;
  debouncedKeyword: Readonly<Ref<string>>;
} {
  const debounceMs = options.debounceMs ?? DEFAULT_DEBOUNCE_MS;
  const keyword = ref("");
  const debouncedKeyword = ref("");
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clearTimer() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  watch(
    keyword,
    (value) => {
      clearTimer();
      timer = setTimeout(() => {
        timer = null;
        debouncedKeyword.value = value.trim();
      }, debounceMs);
    },
    { flush: "sync" },
  );

  if (getCurrentScope()) onScopeDispose(clearTimer);

  return {
    keyword,
    debouncedKeyword: readonly(debouncedKeyword) as Readonly<Ref<string>>,
  };
}

export function useListSearch<T>(
  source: () => readonly T[],
  matcher: (item: T, keyword: string) => boolean,
  options: SearchOptions = {},
): {
  keyword: Ref<string>;
  debouncedKeyword: Readonly<Ref<string>>;
  filtered: ComputedRef<T[]>;
} {
  const search = useDebouncedKeyword(options);
  const filtered = computed(() => {
    const items = source();
    const keyword = search.debouncedKeyword.value;
    if (!keyword) return [...items];
    return items.filter((item) => matcher(item, keyword));
  });

  return { ...search, filtered };
}
