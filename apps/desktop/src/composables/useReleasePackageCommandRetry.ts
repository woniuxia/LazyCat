import { ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ReleasePackageCommandRetryPreflightResult,
  ReleasePackageCommandRetryPrepareResult,
  ReleasePackageRemoteProbeResult,
  ReleasePackageStartResult,
} from "../types/release-package";

export function useReleasePackageCommandRetry() {
  const prepareResult = ref<ReleasePackageCommandRetryPrepareResult | null>(null);
  const authToken = ref("");
  const privateKeyPassphrase = ref("");
  const projectId = ref<number | null>(null);
  const retryToken = ref("");

  function clearState(): void {
    prepareResult.value = null;
    authToken.value = "";
    privateKeyPassphrase.value = "";
    projectId.value = null;
    retryToken.value = "";
  }

  async function discard(): Promise<void> {
    const probeToken = prepareResult.value?.probeToken;
    const preflightToken = authToken.value || undefined;
    clearState();
    if (!probeToken && !preflightToken) return;
    await invokeToolByChannel("tool:release-package:remote-discard", {
      probeToken,
      preflightToken,
    });
  }

  async function reset(): Promise<void> {
    await discard();
  }

  async function prepare(id: number, token: string): Promise<ReleasePackageCommandRetryPrepareResult> {
    await reset();
    projectId.value = id;
    retryToken.value = token;
    const result = await invokeToolByChannel("tool:release-package:command-retry-prepare", {
      projectId: id,
      retryToken: token,
    }) as ReleasePackageCommandRetryPrepareResult;
    prepareResult.value = result;
    return result;
  }

  async function trustHost(replaceExisting: boolean): Promise<ReleasePackageRemoteProbeResult> {
    if (!projectId.value || !prepareResult.value) throw new Error("请先准备命令重试");
    const result = await invokeToolByChannel("tool:release-package:host-trust", {
      projectId: projectId.value,
      probeToken: prepareResult.value.probeToken,
      replaceExisting,
    }) as ReleasePackageRemoteProbeResult;
    prepareResult.value = { ...prepareResult.value, ...result };
    return result;
  }

  async function preflight(): Promise<ReleasePackageCommandRetryPreflightResult> {
    if (!projectId.value || !retryToken.value || !prepareResult.value) throw new Error("请先准备命令重试");
    try {
      const result = await invokeToolByChannel("tool:release-package:command-retry-preflight", {
        projectId: projectId.value,
        retryToken: retryToken.value,
        probeToken: prepareResult.value.probeToken,
        privateKeyPassphrase: privateKeyPassphrase.value || undefined,
      }) as ReleasePackageCommandRetryPreflightResult;
      authToken.value = result.authToken;
      return result;
    } finally {
      privateKeyPassphrase.value = "";
    }
  }

  async function start(): Promise<ReleasePackageStartResult> {
    if (!projectId.value || !retryToken.value || !authToken.value) throw new Error("请先完成命令重试认证");
    try {
      return await invokeToolByChannel("tool:release-package:command-retry-start", {
        projectId: projectId.value,
        retryToken: retryToken.value,
        authToken: authToken.value,
      }) as ReleasePackageStartResult;
    } finally {
      privateKeyPassphrase.value = "";
      authToken.value = "";
    }
  }

  return {
    prepareResult,
    authToken,
    privateKeyPassphrase,
    projectId,
    retryToken,
    prepare,
    trustHost,
    preflight,
    start,
    discard,
    reset,
  };
}
