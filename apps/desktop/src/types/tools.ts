export interface ToolDef {
  id: string;
  name: string;
  desc: string;
}

export interface ToolSearchMeta {
  aliases: string[];
  abbreviation: string;
  description: string;
}

export type ToolSearchMetaMap = Record<string, ToolSearchMeta>;

export interface GroupDef {
  id: string;
  name: string;
  tools: ToolDef[];
}

/** 侧边栏条目：可以是分组或独立的一级工具 */
export type SidebarItem = { kind: "group"; group: GroupDef } | { kind: "tool"; tool: ToolDef };

export type ToolClickHistory = Record<string, number[]>;
