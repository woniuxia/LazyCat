export type PmProjectStatus = "active" | "archived";

export type PmItemType = "task" | "bug" | "feature" | "improvement";

export type PmPriority = "P0" | "P1" | "P2" | "P3";

export type PmItemStatus = "todo" | "in_progress" | "testing" | "done";

export interface PmSiyuanLocation {
  notebookId: string;
  notebookName: string;
  parentDocId: string | null;
  parentDocTitle: string | null;
  parentHpath: string | null;
  parentPath: string | null;
}

export interface PmSiyuanPageRef {
  docId: string;
  docTitle: string;
  docHpath: string;
  docPath: string | null;
  notebookId: string;
  notebookName: string;
}

export interface PmProject {
  id: number;
  name: string;
  description: string;
  color: string;
  status: PmProjectStatus;
  siyuanLocationOverride: PmSiyuanLocation | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface PmItem {
  id: number;
  projectId: number;
  title: string;
  description: string;
  linkUrl: string | null;
  itemType: PmItemType;
  priority: PmPriority;
  status: PmItemStatus;
  startAt: string | null;
  endAt: string | null;
  pinned: boolean;
  sortOrder: number;
  tags: string[];
  siyuanPrimaryPage: PmSiyuanPageRef | null;
  siyuanExtraPages: PmSiyuanPageRef[];
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
  projectName?: string | null;
  projectColor?: string | null;
}

export interface PmTagStat {
  tag: string;
  count: number;
}

export interface WeeklyWorkItem {
  id: number;
  title: string;
  priority: string;
  status: string;
  completedAt: string | null;
  createdAt: string;
  projectId: number | null;
  projectName: string | null;
  projectColor: string | null;
  projectStatus: string | null;
  source: "pm" | "todo";
  itemType?: string;
}

export interface WeeklyWorkResult {
  pmItems: WeeklyWorkItem[];
  todoItems: WeeklyWorkItem[];
  windowStart: string;
  windowEnd: string;
}

export interface PmSiyuanTreeNode {
  id: string;
  name: string;
  hpath: string;
  path: string | null;
  leaf: boolean;
  docCount?: number;
  children: PmSiyuanTreeNode[];
}

export interface PmSiyuanNotebookDirectory {
  id: string;
  name: string;
  icon: string | null;
  closed: boolean;
  docCount: number;
  children: PmSiyuanTreeNode[];
}

export interface PmSiyuanDirectoryResult {
  notebooks: PmSiyuanNotebookDirectory[];
  fetchedAt: string;
}

export interface PmSiyuanSearchResult {
  items: PmSiyuanPageRef[];
  scope: "location" | "all";
}

export const PM_STATUS_COLUMNS: { key: PmItemStatus; label: string }[] = [
  { key: "todo", label: "待办" },
  { key: "in_progress", label: "进行中" },
  { key: "testing", label: "测试中" },
  { key: "done", label: "已完成" },
];

export const PM_ITEM_TYPE_MAP: Record<PmItemType, { label: string; color: string }> = {
  task: { label: "任务", color: "#409eff" },
  bug: { label: "缺陷", color: "#f56c6c" },
  feature: { label: "功能", color: "#67c23a" },
  improvement: { label: "优化", color: "#e6a23c" },
};

export const PM_PRIORITY_MAP: Record<PmPriority, { label: string; color: string }> = {
  P0: { label: "P0 紧急", color: "#f56c6c" },
  P1: { label: "P1 高", color: "#e6a23c" },
  P2: { label: "P2 中", color: "#409eff" },
  P3: { label: "P3 低", color: "#909399" },
};
