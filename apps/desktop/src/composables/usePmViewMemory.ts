import { ref, watch, type Ref } from "vue";
import { getSetting, setSetting } from "./useSettings";
import { hasView, type ViewId } from "./pmViewRegistry";

export type PmContextId = number | "overview";

function settingsKey(ctx: PmContextId): string {
  return `pm:view:${ctx === "overview" ? "overview" : `project-${ctx}`}`;
}

function defaultView(ctx: PmContextId): ViewId {
  return ctx === "overview" ? "list" : "kanban";
}

function readSavedView(ctx: PmContextId): ViewId | null {
  const raw = getSetting(settingsKey(ctx));
  if (raw && hasView(raw)) return raw;
  const legacy = getSetting("pm:viewMode");
  if (legacy && hasView(legacy)) return legacy;
  return null;
}

export function usePmViewMemory(contextRef: Ref<PmContextId | null>) {
  const currentView = ref<ViewId>("kanban");

  watch(
    contextRef,
    (ctx) => {
      if (ctx === null) return;
      const saved = readSavedView(ctx);
      currentView.value = saved ?? defaultView(ctx);
    },
    { immediate: true },
  );

  function setView(viewId: ViewId) {
    const ctx = contextRef.value;
    currentView.value = viewId;
    if (ctx === null) return;
    setSetting(settingsKey(ctx), viewId);
  }

  return { currentView, setView };
}
