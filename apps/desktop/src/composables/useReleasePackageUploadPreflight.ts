import { ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ReleasePackageRemotePreflightResult,
  ReleasePackageRemoteProbeResult,
  ReleasePackageTarget,
} from "../types/release-package";

interface PreflightInput {
  projectId: number;
  targets: ReleasePackageTarget[];
  privateKeyPassphrase?: string;
}

export function useReleasePackageUploadPreflight() {
  const probeResult = ref<ReleasePackageRemoteProbeResult | null>(null);
  const preflightResult = ref<ReleasePackageRemotePreflightResult | null>(null);
  const preflightToken = ref("");
  const checking = ref(false);
  let requestToken = 0;

  function clearPreflight(): void {
    preflightResult.value = null;
    preflightToken.value = "";
  }

  async function probe(projectId: number): Promise<ReleasePackageRemoteProbeResult | null> {
    const token = ++requestToken;
    clearPreflight();
    checking.value = true;
    try {
      const result = await invokeToolByChannel("tool:release-package:remote-probe", {
        projectId,
      }) as ReleasePackageRemoteProbeResult;
      if (token !== requestToken) return null;
      probeResult.value = result;
      return result;
    } catch (error) {
      if (token === requestToken) probeResult.value = null;
      throw error;
    } finally {
      if (token === requestToken) checking.value = false;
    }
  }

  async function trustHost(
    projectId: number,
    replaceExisting: boolean,
  ): Promise<ReleasePackageRemoteProbeResult | null> {
    const probeToken = probeResult.value?.probeToken;
    if (!probeToken) throw new Error("请先探测服务器主机指纹");

    const token = ++requestToken;
    clearPreflight();
    checking.value = true;
    try {
      const result = await invokeToolByChannel("tool:release-package:host-trust", {
        projectId,
        probeToken,
        replaceExisting,
      }) as ReleasePackageRemoteProbeResult;
      if (token !== requestToken) return null;
      probeResult.value = result;
      return result;
    } catch (error) {
      throw error;
    } finally {
      if (token === requestToken) checking.value = false;
    }
  }

  async function check(
    input: PreflightInput,
  ): Promise<ReleasePackageRemotePreflightResult | null> {
    const probeToken = probeResult.value?.probeToken;
    if (!probeToken) throw new Error("请先探测并信任服务器主机指纹");

    const token = ++requestToken;
    clearPreflight();
    checking.value = true;
    try {
      const result = await invokeToolByChannel("tool:release-package:remote-preflight", {
        ...input,
        probeToken,
      }) as ReleasePackageRemotePreflightResult;
      if (token !== requestToken) return null;
      preflightResult.value = result;
      preflightToken.value = result.preflightToken;
      return result;
    } catch (error) {
      throw error;
    } finally {
      if (token === requestToken) checking.value = false;
    }
  }

  function reset(): void {
    requestToken += 1;
    probeResult.value = null;
    clearPreflight();
    checking.value = false;
  }

  return {
    probeResult,
    preflightResult,
    preflightToken,
    checking,
    probe,
    trustHost,
    check,
    reset,
  };
}
