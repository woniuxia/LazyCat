import type { SidebarItem, ToolDef, ToolSearchMeta, ToolSearchMetaMap } from "../types";
import { createSearchField, type SearchField } from "./fuzzy-match";

export interface IndexedTool {
  tool: ToolDef;
  groupName: string;
  fields: SearchField[];
}

const DEFAULT_META: ToolSearchMeta = {
  aliases: [],
  abbreviation: "",
  description: "",
};

export function buildToolIndex(items: SidebarItem[], metaMap: ToolSearchMetaMap): IndexedTool[] {
  const indexed: IndexedTool[] = [];

  for (const item of items) {
    if (item.kind === "tool") {
      const tool = item.tool;
      const meta = metaMap[tool.id] ?? DEFAULT_META;
      indexed.push({
        tool,
        groupName: "工具",
        fields: [
          createSearchField(tool.name, 1.2),
          createSearchField(tool.desc, 0.9),
          createSearchField(tool.id, 0.7),
          createSearchField(meta.abbreviation, 1.3),
          createSearchField(meta.description, 0.85),
          ...meta.aliases.map((alias) => createSearchField(alias, 1.05)),
        ],
      });
      continue;
    }

    for (const tool of item.group.tools) {
      const meta = metaMap[tool.id] ?? DEFAULT_META;
      indexed.push({
        tool,
        groupName: item.group.name,
        fields: [
          createSearchField(tool.name, 1.2),
          createSearchField(tool.desc, 0.9),
          createSearchField(tool.id, 0.7),
          createSearchField(item.group.name, 0.75),
          createSearchField(meta.abbreviation, 1.3),
          createSearchField(meta.description, 0.85),
          ...meta.aliases.map((alias) => createSearchField(alias, 1.05)),
        ],
      });
    }
  }

  return indexed;
}
