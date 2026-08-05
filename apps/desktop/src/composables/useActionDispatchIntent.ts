import { onMounted, watch } from "vue";

import type { ActionDispatchRequest } from "../types";
import { useNavigationHandoff } from "./useNavigationHandoff";

export function useActionDispatchIntent() {
  const handoff = useNavigationHandoff();

  function setPendingIntent(intent: ActionDispatchRequest) {
    handoff.setPendingIntent(intent);
  }

  function consumePendingIntent(toolId: string) {
    return handoff.consumePendingIntent(toolId);
  }

  function watchPendingIntent(
    toolId: string,
    apply: (intent: ActionDispatchRequest) => void | Promise<void>,
  ) {
    onMounted(() => {
      const current = consumePendingIntent(toolId);
      if (current) void apply(current);
    });

    watch(handoff.pendingIntent, (value) => {
      if (value?.targetToolId !== toolId) return;
      const current = consumePendingIntent(toolId);
      if (current) void apply(current);
    });
  }

  return {
    pendingIntent: handoff.pendingIntent,
    setPendingIntent,
    consumePendingIntent,
    watchPendingIntent,
  };
}
