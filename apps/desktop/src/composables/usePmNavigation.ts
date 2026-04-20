import { ref } from "vue";

export interface PmFocusRequest {
  itemId: number;
  projectId: number | null;
}

const pendingFocus = ref<PmFocusRequest | null>(null);

export function usePmNavigation() {
  function requestFocus(itemId: number, projectId: number | null) {
    pendingFocus.value = { itemId, projectId };
  }

  function consumeFocus(): PmFocusRequest | null {
    const req = pendingFocus.value;
    pendingFocus.value = null;
    return req;
  }

  return { pendingFocus, requestFocus, consumeFocus };
}
