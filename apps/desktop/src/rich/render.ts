import { generateHTML } from "@tiptap/core";
import type { JSONContent } from "@tiptap/core";

import { buildExtensions } from "./extensions";

export function renderRichDescription(doc: JSONContent): string {
  return generateHTML(doc, buildExtensions());
}
