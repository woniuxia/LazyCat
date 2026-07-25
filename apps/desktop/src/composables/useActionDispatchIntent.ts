import { onMounted, ref, watch } from "vue";

import type { ActionDispatchRequest } from "../types";

const pendingIntent = ref<ActionDispatchRequest | null>(null);

export function useActionDispatchIntent() {
  function setPendingIntent(intent: ActionDispatchRequest) {
    pendingIntent.value = intent;
  }

  function consumePendingIntent(toolId: string) {
    if (pendingIntent.value?.targetToolId !== toolId) return null;
    const current = pendingIntent.value;
    pendingIntent.value = null;
    return current;
  }

  function watchPendingIntent(
    toolId: string,
    apply: (intent: ActionDispatchRequest) => void | Promise<void>,
  ) {
    onMounted(() => {
      const current = consumePendingIntent(toolId);
      if (current) void apply(current);
    });

    watch(pendingIntent, (value) => {
      if (value?.targetToolId !== toolId) return;
      const current = consumePendingIntent(toolId);
      if (current) void apply(current);
    });
  }

  return {
    pendingIntent,
    setPendingIntent,
    consumePendingIntent,
    watchPendingIntent,
  };
}
