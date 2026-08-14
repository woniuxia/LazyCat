import { nextTick, ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";

interface VaultStatus {
  setup: boolean;
  unlocked: boolean;
}

type Continuation = () => Promise<void>;

export function useVaultInlineUnlock() {
  const visible = ref(false);
  const credentialLabel = ref("");
  const masterPassword = ref("");
  const error = ref("");
  const submitting = ref(false);
  const focusNonce = ref(0);
  let continuation: Continuation | null = null;
  let generation = 0;

  function clearSecret(): void {
    masterPassword.value = "";
  }

  function request(label: string, resume: Continuation): void {
    credentialLabel.value = label;
    continuation = resume;
    visible.value = true;
    error.value = "";
    clearSecret();
    void nextTick(() => {
      focusNonce.value += 1;
    });
  }

  async function requireUnlocked(label: string, resume: Continuation): Promise<boolean> {
    const status = (await invokeToolByChannel("tool:vault:status", {})) as VaultStatus;
    if (!status?.setup) throw new Error("vault_not_initialized");
    if (status.unlocked) return true;
    request(label, resume);
    return false;
  }

  function mapUnlockError(cause: unknown): string {
    const message = cause instanceof Error ? cause.message : String(cause);
    if (message.includes("wrong_password") || message.includes("bad_master_password")) {
      return "主密码错误，请重试";
    }
    return message || "Vault 解锁失败";
  }

  async function submit(): Promise<void> {
    if (submitting.value || !visible.value) return;
    const password = masterPassword.value;
    if (!password) {
      error.value = "请输入主密码";
      focusNonce.value += 1;
      return;
    }

    const attemptGeneration = generation;
    submitting.value = true;
    error.value = "";
    let resume: Continuation | null = null;
    try {
      await invokeToolByChannel("tool:vault:unlock", { masterPassword: password });
      if (attemptGeneration !== generation) return;
      resume = continuation;
      continuation = null;
      visible.value = false;
    } catch (cause) {
      if (attemptGeneration !== generation) return;
      error.value = mapUnlockError(cause);
      await nextTick();
      focusNonce.value += 1;
    } finally {
      if (attemptGeneration === generation) {
        clearSecret();
        submitting.value = false;
      }
    }
    if (resume) await resume();
  }

  function reset(): void {
    generation += 1;
    continuation = null;
    visible.value = false;
    credentialLabel.value = "";
    error.value = "";
    submitting.value = false;
    clearSecret();
  }

  return {
    visible,
    credentialLabel,
    masterPassword,
    error,
    submitting,
    focusNonce,
    request,
    requireUnlocked,
    submit,
    reset,
  };
}
