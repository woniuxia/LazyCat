import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

type InvokeWithLoadingOptions = {
  errorPrefix?: string;
};

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Composable wrapping IPC invoke with loading state and error handling.
 */
export function useToolInvoke() {
  const loading = ref(false);

  async function invoke<T = unknown>(
    channel: string,
    payload: Record<string, unknown>,
  ): Promise<T> {
    return invokeToolByChannel(channel, payload) as Promise<T>;
  }

  async function invokeWithLoading<T = unknown>(
    channel: string,
    payload: Record<string, unknown>,
    opts?: InvokeWithLoadingOptions,
  ): Promise<T | undefined> {
    loading.value = true;
    try {
      return (await invokeToolByChannel(channel, payload)) as T;
    } catch (error) {
      ElMessage.error(`${opts?.errorPrefix ?? ""}${getErrorMessage(error)}`);
      return undefined;
    } finally {
      loading.value = false;
    }
  }

  async function invokeSilent<T = unknown>(
    channel: string,
    payload: Record<string, unknown>,
  ): Promise<T | undefined> {
    try {
      return await invoke<T>(channel, payload);
    } catch {
      return undefined;
    }
  }

  async function invokeString(
    channel: string,
    payload: Record<string, unknown>,
  ): Promise<string> {
    const data = await invokeToolByChannel(channel, payload);
    return typeof data === "string" ? data : JSON.stringify(data, null, 2);
  }

  async function invokeStringWithError(
    channel: string,
    payload: Record<string, unknown>,
  ): Promise<string | undefined> {
    try {
      return await invokeString(channel, payload);
    } catch (error) {
      ElMessage.error((error as Error).message);
      return undefined;
    }
  }

  return { loading, invoke, invokeWithLoading, invokeSilent, invokeString, invokeStringWithError };
}
