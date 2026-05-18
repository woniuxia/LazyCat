import { ref } from "vue";

export interface TodoFocusRequest {
  itemId: number;
}

const pendingFocus = ref<TodoFocusRequest | null>(null);

export function useTodoNavigation() {
  function requestFocus(itemId: number) {
    pendingFocus.value = { itemId };
  }

  function consumeFocus(): TodoFocusRequest | null {
    const req = pendingFocus.value;
    pendingFocus.value = null;
    return req;
  }

  return { pendingFocus, requestFocus, consumeFocus };
}
