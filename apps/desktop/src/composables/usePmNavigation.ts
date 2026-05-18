import { ref } from "vue";
import type { ViewId } from "./pmViewRegistry";

export interface PmFocusRequest {
  itemId: number;
  projectId: number | null;
  view?: ViewId;
}

const pendingFocus = ref<PmFocusRequest | null>(null);

export function usePmNavigation() {
  function requestFocus(itemId: number, projectId: number | null, view?: ViewId) {
    pendingFocus.value = { itemId, projectId, view };
  }

  function consumeFocus(): PmFocusRequest | null {
    const req = pendingFocus.value;
    pendingFocus.value = null;
    return req;
  }

  return { pendingFocus, requestFocus, consumeFocus };
}
