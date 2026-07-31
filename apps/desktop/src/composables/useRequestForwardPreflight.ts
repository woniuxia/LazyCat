import { ref } from "vue";
import type {
  RequestForwardPreflightResult,
  RequestForwardRuleWriteInput,
} from "../types/request-forward";
import type { RequestForwardSelectionIntentState } from "../utils/requestForward";

interface RequestForwardPreflightContext {
  intent: RequestForwardSelectionIntentState;
  payload: RequestForwardRuleWriteInput;
}

interface UseRequestForwardPreflightOptions {
  currentContext: () => RequestForwardPreflightContext;
  execute: (payload: RequestForwardRuleWriteInput) => Promise<RequestForwardPreflightResult>;
  onError: (error: unknown) => void;
}

function snapshotContext(context: RequestForwardPreflightContext): string {
  const { intent, payload } = context;
  return JSON.stringify([
    intent.selectionToken,
    intent.selectedId,
    intent.draft,
    payload.name,
    payload.protocol,
    payload.bindHost,
    payload.listenPort,
    payload.targetUrl,
    payload.targetHost,
    payload.targetPort,
    payload.captureHttpHeaders,
    payload.captureHttpBody,
  ]);
}

export function useRequestForwardPreflight({
  currentContext,
  execute,
  onError,
}: UseRequestForwardPreflightOptions) {
  const result = ref<RequestForwardPreflightResult | null>(null);
  const loading = ref(false);
  let requestToken = 0;
  let acceptedContextSnapshot: string | null = null;

  function isRequestCurrent(token: number, contextSnapshot: string): boolean {
    return token === requestToken && contextSnapshot === snapshotContext(currentContext());
  }

  async function run(): Promise<RequestForwardPreflightResult | null> {
    const context = currentContext();
    const payload = { ...context.payload };
    const contextSnapshot = snapshotContext({
      intent: { ...context.intent },
      payload,
    });
    const token = ++requestToken;
    result.value = null;
    acceptedContextSnapshot = null;
    loading.value = true;

    try {
      const nextResult = await execute(payload);
      if (!isRequestCurrent(token, contextSnapshot)) return null;
      result.value = nextResult;
      acceptedContextSnapshot = contextSnapshot;
      return nextResult;
    } catch (error) {
      if (isRequestCurrent(token, contextSnapshot)) onError(error);
      return null;
    } finally {
      if (token === requestToken) loading.value = false;
    }
  }

  function invalidate() {
    requestToken += 1;
    result.value = null;
    acceptedContextSnapshot = null;
    loading.value = false;
  }

  function isAcceptedCurrent(): boolean {
    return result.value != null && acceptedContextSnapshot === snapshotContext(currentContext());
  }

  return { result, loading, run, invalidate, isAcceptedCurrent };
}
