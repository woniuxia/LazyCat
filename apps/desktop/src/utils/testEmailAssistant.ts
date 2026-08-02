export const DEFAULT_TEST_EMAIL_TEMPLATE =
  "{{称呼}}：\n\n{{功能需求内容}}测试开发已完成，请进行测试。\n测试步骤：\n{{测试步骤}}";

const PLACEHOLDER_OPEN = "{{";
const PLACEHOLDER_CLOSE = "}}";
const MULTILINE_FIELD_KEYWORDS = ["内容", "步骤", "说明", "描述", "备注"] as const;

export function normalizePlaceholderName(rawName: string): string {
  return rawName.trim();
}

/** Finds only flat, single-line placeholders. Invalid nested candidates are skipped as a whole. */
export function extractPlaceholders(text: string): string[] {
  const names: string[] = [];
  const seen = new Set<string>();
  let cursor = 0;

  while (cursor < text.length) {
    const openIndex = text.indexOf(PLACEHOLDER_OPEN, cursor);
    if (openIndex < 0) break;

    const closeIndex = text.indexOf(PLACEHOLDER_CLOSE, openIndex + PLACEHOLDER_OPEN.length);
    if (closeIndex < 0) break;

    const rawName = text.slice(openIndex + PLACEHOLDER_OPEN.length, closeIndex);
    const name = normalizePlaceholderName(rawName);
    const isFlat = !rawName.includes("{") && !rawName.includes("}");
    const isSingleLine = !/[\r\n]/u.test(rawName);

    if (isFlat && isSingleLine && name && !seen.has(name)) {
      seen.add(name);
      names.push(name);
    }

    cursor = closeIndex + PLACEHOLDER_CLOSE.length;
  }

  return names;
}

export function mergePlaceholders(...sources: string[]): string[] {
  return sources.reduce<string[]>((merged, source) => {
    for (const name of extractPlaceholders(source)) {
      if (!merged.includes(name)) merged.push(name);
    }
    return merged;
  }, []);
}

export function isMultilineFieldName(name: string): boolean {
  return MULTILINE_FIELD_KEYWORDS.some((keyword) => name.includes(keyword));
}

export function getMissingPlaceholders(
  placeholders: readonly string[],
  values: Readonly<Record<string, string | undefined>>,
): string[] {
  return placeholders.filter((name) => !(values[name] ?? "").trim());
}

export function renderEmailTemplate(
  template: string,
  values: Readonly<Record<string, string | undefined>>,
): string {
  let cursor = 0;
  let rendered = "";

  while (cursor < template.length) {
    const openIndex = template.indexOf(PLACEHOLDER_OPEN, cursor);
    if (openIndex < 0) {
      rendered += template.slice(cursor);
      break;
    }

    rendered += template.slice(cursor, openIndex);
    const closeIndex = template.indexOf(PLACEHOLDER_CLOSE, openIndex + PLACEHOLDER_OPEN.length);
    if (closeIndex < 0) {
      rendered += template.slice(openIndex);
      break;
    }

    const rawName = template.slice(openIndex + PLACEHOLDER_OPEN.length, closeIndex);
    const name = normalizePlaceholderName(rawName);
    const isRecognized =
      !rawName.includes("{") && !rawName.includes("}") && !/[\r\n]/u.test(rawName) && !!name;
    rendered += isRecognized && Object.prototype.hasOwnProperty.call(values, name)
      ? (values[name] ?? "")
      : template.slice(openIndex, closeIndex + PLACEHOLDER_CLOSE.length);
    cursor = closeIndex + PLACEHOLDER_CLOSE.length;
  }

  return rendered;
}
