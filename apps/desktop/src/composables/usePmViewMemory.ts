import { ref, watch, type Ref } from "vue";
import { getSetting, setSetting } from "./useSettings";
import { hasView, type ViewId } from "./pmViewRegistry";

export type PmContextId = number | "overview";

const GLOBAL_KEY = "pm:view";

function defaultView(ctx: PmContextId): ViewId {
  return ctx === "overview" ? "list" : "kanban";
}

function readSavedView(): ViewId | null {
  const raw = getSetting(GLOBAL_KEY);
  if (raw && hasView(raw)) return raw;
  const legacy = getSetting("pm:viewMode");
  if (legacy && hasView(legacy)) return legacy;
  return null;
}

export function usePmViewMemory(contextRef: Ref<PmContextId | null>) {
  const saved = readSavedView();
  const currentView = ref<ViewId>(saved ?? "kanban");

  if (!saved) {
    const stop = watch(
      contextRef,
      (ctx) => {
        if (ctx === null) return;
        currentView.value = defaultView(ctx);
        stop();
      },
      { immediate: true },
    );
  }

  function setView(viewId: ViewId) {
    currentView.value = viewId;
    setSetting(GLOBAL_KEY, viewId);
  }

  return { currentView, setView };
}
