/** 树形模式体积闸门:1MB(1_000_000 字符),与 api-workbench 响应预览阈值一致。 */
export const JSON_TREE_TEXT_LIMIT = 1_000_000;

export type JsonTreeGateResult =
  | { ok: true; value: unknown }
  | { ok: false; reason: string };

/** 文本进入 JSON 树形模式的闸门:先查体积(避免超大文本白解析),再严格 JSON.parse。 */
export function canEnterJsonTree(text: string): JsonTreeGateResult {
  if (text.length > JSON_TREE_TEXT_LIMIT) {
    return { ok: false, reason: "内容超过 1MB 上限,树形模式不可用" };
  }
  try {
    return { ok: true, value: JSON.parse(text) };
  } catch (error) {
    return { ok: false, reason: `JSON 解析失败: ${(error as Error).message}` };
  }
}
