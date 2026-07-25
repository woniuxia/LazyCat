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

  async function discardTokens(tokens: {
    probeToken?: string;
    preflightToken?: string;
  }): Promise<void> {
    if (!tokens.probeToken && !tokens.preflightToken) return;
    await invokeToolByChannel("tool:release-package:remote-discard", tokens);
  }

  async function discardCurrentPreflight(): Promise<void> {
    const token = preflightToken.value;
    clearPreflight();
    await discardTokens({ preflightToken: token || undefined });
  }

  async function discardCurrentState(): Promise<void> {
    const tokens = {
      probeToken: probeResult.value?.probeToken,
      preflightToken: preflightToken.value || undefined,
    };
    probeResult.value = null;
    clearPreflight();
    await discardTokens(tokens);
  }

  async function probe(projectId: number): Promise<ReleasePackageRemoteProbeResult | null> {
    const token = ++requestToken;
    await discardCurrentState();
    if (token !== requestToken) return null;
    checking.value = true;
    try {
      const result = await invokeToolByChannel("tool:release-package:remote-probe", {
        projectId,
      }) as ReleasePackageRemoteProbeResult;
      if (token !== requestToken) {
        await discardTokens({ probeToken: result.probeToken });
        return null;
      }
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
    await discardCurrentPreflight();
    if (token !== requestToken) return null;
    checking.value = true;
    try {
      const result = await invokeToolByChannel("tool:release-package:host-trust", {
        projectId,
        probeToken,
        replaceExisting,
      }) as ReleasePackageRemoteProbeResult;
      if (token !== requestToken) {
        await discardTokens({ probeToken: result.probeToken });
        return null;
      }
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
    await discardCurrentPreflight();
    if (token !== requestToken) return null;
    checking.value = true;
    try {
      const result = await invokeToolByChannel("tool:release-package:remote-preflight", {
        ...input,
        probeToken,
      }) as ReleasePackageRemotePreflightResult;
      if (token !== requestToken) {
        await discardTokens({ preflightToken: result.preflightToken });
        return null;
      }
      preflightResult.value = result;
      preflightToken.value = result.preflightToken;
      return result;
    } catch (error) {
      throw error;
    } finally {
      if (token === requestToken) checking.value = false;
    }
  }

  async function reset(): Promise<void> {
    const tokens = {
      probeToken: probeResult.value?.probeToken,
      preflightToken: preflightToken.value || undefined,
    };
    const token = ++requestToken;
    probeResult.value = null;
    clearPreflight();
    checking.value = false;
    try {
      await discardTokens(tokens);
    } finally {
      if (token === requestToken) {
        probeResult.value = null;
        clearPreflight();
        checking.value = false;
      }
    }
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
