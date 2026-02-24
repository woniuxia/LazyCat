import { ref } from "vue";
import type { ClipboardDetectResult } from "../utils/clipboard-detect";
import { detectClipboardContent } from "../utils/clipboard-detect";

// 模块级单例状态
const suggestion = ref<ClipboardDetectResult | null>(null);
const visible = ref(false);
const lastClipboardText = ref("");
const pendingInput = ref<{ toolId: string; text: string } | null>(null);

export function useClipboardSuggestion() {
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
        suggestion.value = result;
        visible.value = true;
      }
    } catch {
      // 剪贴板读取失败（权限不足等），静默忽略
    }
  }

  /**
   * 用户点击操作按钮后调用：设置 pendingInput，供目标面板消费。
   */
  function applyAction(toolId: string): void {
    pendingInput.value = { toolId, text: lastClipboardText.value };
    visible.value = false;
    suggestion.value = null;
  }

  /**
   * 目标工具在 onMounted 中调用，消费并清除 pendingInput。
   * 仅当 toolId 匹配时返回文本，否则返回 null。
   */
  function consumePendingInput(toolId: string): string | null {
    if (pendingInput.value && pendingInput.value.toolId === toolId) {
      const text = pendingInput.value.text;
      pendingInput.value = null;
      return text;
    }
    return null;
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
    applyAction,
    consumePendingInput,
    dismiss,
  };
}
