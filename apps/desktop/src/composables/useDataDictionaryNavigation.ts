import { ref } from "vue";

export interface DataDictionaryFocusRequest {
  recordId: number;
}

const pendingFocus = ref<DataDictionaryFocusRequest | null>(null);

export function useDataDictionaryNavigation() {
  function requestFocus(recordId: number) {
    pendingFocus.value = { recordId };
  }

  function consumeFocus(): DataDictionaryFocusRequest | null {
    const req = pendingFocus.value;
    pendingFocus.value = null;
    return req;
  }

  return { pendingFocus, requestFocus, consumeFocus };
}
