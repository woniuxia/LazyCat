import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ref, watch, onMounted } from "vue";
import type { ClipboardDetectResult } from "../utils/clipboard-detect";
import { detectClipboardContent } from "../utils/clipboard-detect";
import { getSetting } from "./useSettings";

export interface TodoPendingDraft {
  title?: string;
  description?: string;
}

export interface VaultPendingDraft {
  category?: "app" | "server" | "database";
  title?: string;
  environment?: string;
  fields?: Record<string, unknown>;
  tags?: string[];
}

export interface PendingToolInput {
  toolId: string;
  text: string;
  source?: "clipboard-suggestion" | "inbox";
  label?: string;
  todoDraft?: TodoPendingDraft;
  vaultDraft?: VaultPendingDraft;
  meta?: Record<string, unknown>;
}

// 模块级单例状态
const suggestion = ref<ClipboardDetectResult | null>(null);
const visible = ref(false);
const lastClipboardText = ref("");
const pendingInput = ref<PendingToolInput | null>(null);
let clipboardListenerPromise: Promise<UnlistenFn | null> | null = null;

export function useClipboardSuggestion() {
  function showSuggestion(next: ClipboardDetectResult, rawText?: string): void {
    suggestion.value = next;
    visible.value = true;
    if (rawText !== undefined) {
      lastClipboardText.value = rawText;
    }
  }

  /**
   * 读取剪贴板并检测内容类型。
   * 与上次检测的文本比较，相同则跳过（去重）。
   */
  async function detectClipboard(): Promise<void> {
    try {
      const text = await navigator.clipboard.readText();
      if (!text || text === lastClipboardText.value) return;
      lastClipboardText.value = text;

      const result = detectClipboardContent(text);
      if (result) {
        showSuggestion(result, text);
      }
    } catch {
      // 剪贴板读取失败（权限不足等），静默忽略
    }
  }

  async function ensureClipboardListener(): Promise<void> {
    if (!clipboardListenerPromise) {
      clipboardListenerPromise = (async () => {
        try {
          return await listen("clipboard-changed", async () => {
            if (getSetting("clipboard_detection") === "false") return;
            await detectClipboard();
          });
        } catch {
          return null;
        }
      })();
    }
    await clipboardListenerPromise;
  }

  function setPendingToolInput(input: PendingToolInput): void {
    pendingInput.value = input;
    visible.value = false;
    suggestion.value = null;
  }

  /**
   * 用户点击操作按钮后调用：设置 pendingInput，供目标面板消费。
   */
  function applyAction(toolId: string): void {
    setPendingToolInput({
      toolId,
      text: lastClipboardText.value,
      source: "clipboard-suggestion",
    });
  }

  /**
   * 目标工具在 onMounted 中调用，消费并清除 pendingInput。
   * 仅当 toolId 匹配时返回文本，否则返回 null。
   */
  function consumePendingInput(toolId: string): string | null {
    const input = consumePendingToolInput(toolId);
    return input?.text ?? null;
  }

  function consumePendingToolInput(toolId: string): PendingToolInput | null {
    if (pendingInput.value && pendingInput.value.toolId === toolId) {
      const input = pendingInput.value;
      pendingInput.value = null;
      return input;
    }
    return null;
  }

  /**
   * 在目标面板中调用：同时处理首次挂载和已挂载后的 pendingInput 变更。
   * 解决面板已打开时再次触发智能助手不会更新内容的问题。
   */
  function watchPendingInput(
    toolId: string | (() => string),
    apply: (text: string) => void,
  ): void {
    watchPendingToolInput(toolId, (input) => {
      if (input.text) apply(input.text);
    });
  }

  function watchPendingToolInput(
    toolId: string | (() => string),
    apply: (input: PendingToolInput) => void | Promise<void>,
  ): void {
    const resolveId = typeof toolId === "function" ? toolId : () => toolId;

    onMounted(() => {
      const pending = consumePendingToolInput(resolveId());
      if (pending) void apply(pending);
    });

    watch(pendingInput, (val) => {
      if (val && val.toolId === resolveId()) {
        const input = consumePendingToolInput(resolveId());
        if (input) void apply(input);
      }
    });
  }

  /**
   * 关闭通知。
   */
  function dismiss(): void {
    visible.value = false;
    suggestion.value = null;
  }

  return {
    suggestion,
    visible,
    pendingInput,
    detectClipboard,
    ensureClipboardListener,
    showSuggestion,
    applyAction,
    setPendingToolInput,
    consumePendingInput,
    consumePendingToolInput,
    watchPendingInput,
    watchPendingToolInput,
    dismiss,
  };
}
