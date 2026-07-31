import { suppressClipboardCapture } from "../bridge/tauri";

const DEFAULT_CLEAR_MS = 30_000;

let pendingClearTimer: ReturnType<typeof setTimeout> | null = null;

export async function writeSecretToClipboard(value: string): Promise<void> {
  await suppressClipboardCapture(value);
  await navigator.clipboard.writeText(value);
}

export function scheduleClipboardClear(secret: string, delayMs: number = DEFAULT_CLEAR_MS): void {
  if (pendingClearTimer) clearTimeout(pendingClearTimer);
  pendingClearTimer = setTimeout(async () => {
    pendingClearTimer = null;
    try {
      const current = await navigator.clipboard.readText();
      if (current === secret) await navigator.clipboard.writeText("");
    } catch {
      try {
        await navigator.clipboard.writeText("");
      } catch {
        /* ignore */
      }
    }
  }, delayMs);
}

export function cancelScheduledClipboardClear(): void {
  if (pendingClearTimer) {
    clearTimeout(pendingClearTimer);
    pendingClearTimer = null;
  }
}
