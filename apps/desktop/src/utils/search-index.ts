import type { SidebarItem, ToolDef, ToolSearchMeta, ToolSearchMetaMap } from "../types";
import { toPinyinInitials } from "./fuzzy-match";

export interface IndexedTool {
  tool: ToolDef;
  groupName: string;
  fields: Array<{
    text: string;
    initials: string;
    weight: number;
  }>;
}

const DEFAULT_META: ToolSearchMeta = {
  aliases: [],
  abbreviation: "",
  description: "",
};

function cleanKeyword(value: string): string {
  return value.trim();
}

function makeField(text: string, weight: number) {
  const cleaned = cleanKeyword(text);
  return {
    text: cleaned,
    initials: toPinyinInitials(cleaned),
    weight,
  };
}

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
          makeField(tool.name, 1.2),
          makeField(tool.desc, 0.9),
          makeField(tool.id, 0.7),
          makeField(meta.abbreviation, 1.3),
          makeField(meta.description, 0.85),
          ...meta.aliases.map((alias) => makeField(alias, 1.05)),
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
          makeField(tool.name, 1.2),
          makeField(tool.desc, 0.9),
          makeField(tool.id, 0.7),
          makeField(item.group.name, 0.75),
          makeField(meta.abbreviation, 1.3),
          makeField(meta.description, 0.85),
          ...meta.aliases.map((alias) => makeField(alias, 1.05)),
        ],
      });
    }
  }

  return indexed;
}
