import { ref } from "vue";

export interface TodoFocusRequest {
  itemId: number;
}

export interface FollowUpFocusRequest {
  itemId: number | null;
  dueOnly: boolean;
}

const pendingFocus = ref<TodoFocusRequest | null>(null);
const pendingFollowUpFocus = ref<FollowUpFocusRequest | null>(null);

export function useTodoNavigation() {
  function requestFocus(itemId: number) {
    pendingFocus.value = { itemId };
  }

  function consumeFocus(): TodoFocusRequest | null {
    const req = pendingFocus.value;
    pendingFocus.value = null;
    return req;
  }

  function requestFollowUp(itemId: number | null, dueOnly = false) {
    pendingFollowUpFocus.value = { itemId, dueOnly };
  }

  function consumeFollowUpFocus(): FollowUpFocusRequest | null {
    const request = pendingFollowUpFocus.value;
    pendingFollowUpFocus.value = null;
    return request;
  }

  return {
    pendingFocus,
    pendingFollowUpFocus,
    requestFocus,
    consumeFocus,
    requestFollowUp,
    consumeFollowUpFocus,
  };
}
