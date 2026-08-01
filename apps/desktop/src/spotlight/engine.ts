import { computed, ref, type Ref } from "vue";
import type { UsageRef, UsageSummary } from "../types/usage";
import { mergeSpotlightProviderItems, shouldRunQueryProvider } from "./search";
import type {
  ProviderDescriptor,
  SpotlightItem,
  SpotlightProviderId,
  SpotlightSearchContext,
} from "./types";
import {
  collectUsageRefs,
  loadUsageSummaries,
  type SpotlightItemsMap,
  usageRefsSignature,
} from "./usage-repository";

export type SpotlightProviderPhase = "prefetch" | "search" | "usage";

export interface SpotlightProviderError {
  sourceId: SpotlightProviderId | "usage";
  phase: SpotlightProviderPhase;
  message: string;
}

export interface SpotlightEngineOptions {
  queryDebounceMs?: number;
  loadUsage?: (refs: ReadonlyMap<string, UsageRef>) => Promise<Map<string, UsageSummary>>;
}

interface RefreshProviderOptions {
  refreshUsage?: boolean;
}

const DEFAULT_QUERY_DEBOUNCE_MS = 120;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function errorKey(sourceId: SpotlightProviderError["sourceId"], phase: SpotlightProviderPhase) {
  return `${phase}:${sourceId}`;
}

export function createSpotlightEngine(options: SpotlightEngineOptions = {}) {
  const prefetchedItems: Ref<SpotlightItemsMap> = ref(new Map());
  const queryItems: Ref<SpotlightItemsMap> = ref(new Map());
  const usageSummaries = ref<Map<string, UsageSummary>>(new Map());
  const providerErrors = ref<Map<string, SpotlightProviderError>>(new Map());
  const prefetchLoading = ref(false);
  const queryLoading = ref(false);

  const mergedItems = computed(() =>
    mergeSpotlightProviderItems(prefetchedItems.value, queryItems.value),
  );

  const prefetchVersions = new Map<SpotlightProviderId, number>();
  const pendingPrefetches = new Set<symbol>();
  let queryVersion = 0;
  let queryTimer: ReturnType<typeof setTimeout> | null = null;
  let queryAbortController: AbortController | null = null;
  let usageRequestVersion = 0;
  let lastUsageSignature = "";
  let disposed = false;

  const queryDebounceMs = options.queryDebounceMs ?? DEFAULT_QUERY_DEBOUNCE_MS;
  const loadUsage = options.loadUsage ?? loadUsageSummaries;

  function setError(error: SpotlightProviderError) {
    const next = new Map(providerErrors.value);
    next.set(errorKey(error.sourceId, error.phase), error);
    providerErrors.value = next;
  }

  function clearError(sourceId: SpotlightProviderError["sourceId"], phase: SpotlightProviderPhase) {
    const key = errorKey(sourceId, phase);
    if (!providerErrors.value.has(key)) return;
    const next = new Map(providerErrors.value);
    next.delete(key);
    providerErrors.value = next;
  }

  function clearPhaseErrors(phase: SpotlightProviderPhase) {
    const next = new Map(providerErrors.value);
    let changed = false;
    for (const [key, error] of next) {
      if (error.phase !== phase) continue;
      next.delete(key);
      changed = true;
    }
    if (changed) providerErrors.value = next;
  }

  async function refreshUsage(items: SpotlightItemsMap = mergedItems.value, force = false) {
    const refs = collectUsageRefs(items);
    const signature = usageRefsSignature(refs);
    if (!force && signature === lastUsageSignature) return;
    lastUsageSignature = signature;
    const requestVersion = ++usageRequestVersion;
    if (refs.size === 0) {
      usageSummaries.value = new Map();
      clearError("usage", "usage");
      return;
    }

    try {
      const summaries = await loadUsage(refs);
      if (disposed || requestVersion !== usageRequestVersion) return;
      usageSummaries.value = summaries;
      clearError("usage", "usage");
    } catch (error) {
      if (disposed || requestVersion !== usageRequestVersion) return;
      setError({ sourceId: "usage", phase: "usage", message: errorMessage(error) });
      console.warn("[Spotlight] load usage summaries failed:", error);
    }
  }

  async function refreshProvider(
    provider: ProviderDescriptor,
    refreshOptions: RefreshProviderOptions = {},
  ): Promise<boolean> {
    const version = (prefetchVersions.get(provider.id) ?? 0) + 1;
    prefetchVersions.set(provider.id, version);
    const pendingToken = Symbol(provider.id);
    pendingPrefetches.add(pendingToken);
    prefetchLoading.value = true;

    try {
      const items = await provider.prefetch();
      if (disposed || prefetchVersions.get(provider.id) !== version) return false;
      const next = new Map(prefetchedItems.value);
      next.set(provider.id, items);
      prefetchedItems.value = next;
      clearError(provider.id, "prefetch");
      if (refreshOptions.refreshUsage) await refreshUsage(mergedItems.value, true);
      return true;
    } catch (error) {
      if (disposed || prefetchVersions.get(provider.id) !== version) return false;
      setError({ sourceId: provider.id, phase: "prefetch", message: errorMessage(error) });
      console.warn(`[Spotlight] provider ${provider.id} prefetch failed:`, error);
      return false;
    } finally {
      pendingPrefetches.delete(pendingToken);
      prefetchLoading.value = pendingPrefetches.size > 0;
    }
  }

  async function refreshAll(providers: readonly ProviderDescriptor[]): Promise<void> {
    const enabledIds = new Set(providers.map((provider) => provider.id));
    for (const providerId of prefetchVersions.keys()) {
      if (!enabledIds.has(providerId)) {
        prefetchVersions.set(providerId, (prefetchVersions.get(providerId) ?? 0) + 1);
      }
    }
    const retained = new Map<SpotlightProviderId, SpotlightItem[]>();
    for (const [providerId, items] of prefetchedItems.value) {
      if (enabledIds.has(providerId)) retained.set(providerId, items);
    }
    prefetchedItems.value = retained;

    const retainedErrors = new Map(providerErrors.value);
    for (const [key, error] of retainedErrors) {
      if (error.sourceId !== "usage" && !enabledIds.has(error.sourceId)) retainedErrors.delete(key);
    }
    providerErrors.value = retainedErrors;

    await Promise.all(providers.map((provider) => refreshProvider(provider)));
    if (!disposed) await refreshUsage(mergedItems.value, true);
  }

  function cancelQueryRefresh() {
    queryVersion += 1;
    if (queryTimer != null) {
      clearTimeout(queryTimer);
      queryTimer = null;
    }
    queryAbortController?.abort();
    queryAbortController = null;
    queryLoading.value = false;
  }

  async function runQueryProviders(
    version: number,
    query: string,
    scope: SpotlightProviderId | null,
    providers: readonly ProviderDescriptor[],
    abortController: AbortController,
  ) {
    const next = new Map<SpotlightProviderId, SpotlightItem[]>();
    await Promise.all(
      providers.map(async (provider) => {
        try {
          const context: SpotlightSearchContext = { scope, signal: abortController.signal };
          const items = await provider.search!(query, context);
          if (abortController.signal.aborted) return;
          next.set(provider.id, items);
          clearError(provider.id, "search");
        } catch (error) {
          if (abortController.signal.aborted) return;
          setError({ sourceId: provider.id, phase: "search", message: errorMessage(error) });
          console.warn(`[Spotlight] provider ${provider.id} query search failed:`, error);
        }
      }),
    );

    if (disposed || abortController.signal.aborted || version !== queryVersion) return;
    queryItems.value = next;
    queryLoading.value = false;
    void refreshUsage(mergedItems.value);
  }

  function scheduleQueryRefresh(
    query: string,
    scope: SpotlightProviderId | null,
    providers: readonly ProviderDescriptor[],
  ) {
    cancelQueryRefresh();
    clearPhaseErrors("search");
    queryItems.value = new Map();

    const selected = providers.filter(
      (provider) =>
        provider.search &&
        (!scope || provider.id === scope) &&
        shouldRunQueryProvider(query, scope, provider.id),
    );
    if (selected.length === 0 || disposed) return;

    const version = queryVersion;
    const abortController = new AbortController();
    queryAbortController = abortController;
    queryLoading.value = true;
    queryTimer = setTimeout(() => {
      queryTimer = null;
      void runQueryProviders(version, query, scope, selected, abortController);
    }, queryDebounceMs);
  }

  function resetQuery() {
    cancelQueryRefresh();
    queryItems.value = new Map();
    clearPhaseErrors("search");
  }

  function dispose() {
    disposed = true;
    cancelQueryRefresh();
    usageRequestVersion += 1;
    for (const providerId of prefetchVersions.keys()) {
      prefetchVersions.set(providerId, (prefetchVersions.get(providerId) ?? 0) + 1);
    }
  }

  return {
    prefetchedItems,
    queryItems,
    mergedItems,
    usageSummaries,
    providerErrors,
    prefetchLoading,
    queryLoading,
    refreshAll,
    refreshProvider,
    refreshUsage,
    scheduleQueryRefresh,
    resetQuery,
    dispose,
  };
}
