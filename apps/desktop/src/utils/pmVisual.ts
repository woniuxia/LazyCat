import type { PmProject } from "../types/pm";

export interface PmProjectItemCount {
  total: number;
  done: number;
}

export interface PmSidebarProject extends PmProject {
  totalCount: number;
  pendingCount: number;
}

export interface PmTagSummary {
  visibleTags: string[];
  hiddenCount: number;
}

export function getPmPendingCount(count?: PmProjectItemCount | null): number {
  if (!count) {
    return 0;
  }
  return Math.max(count.total - count.done, 0);
}

export function getPmTotalCount(count?: PmProjectItemCount | null): number {
  if (!count) {
    return 0;
  }
  return Math.max(count.total, 0);
}

export function sortPmProjectsForSidebar(
  projects: PmProject[],
  projectItemCounts: Record<number, PmProjectItemCount>,
): PmSidebarProject[] {
  return [...projects]
    .sort((left, right) => {
      const totalDiff = getPmTotalCount(projectItemCounts[right.id]) - getPmTotalCount(projectItemCounts[left.id]);
      if (totalDiff !== 0) {
        return totalDiff;
      }
      const sortOrderDiff = left.sortOrder - right.sortOrder;
      if (sortOrderDiff !== 0) {
        return sortOrderDiff;
      }
      const nameDiff = left.name.localeCompare(right.name, "zh-CN");
      if (nameDiff !== 0) {
        return nameDiff;
      }
      return left.id - right.id;
    })
    .map((project) => ({
      ...project,
      totalCount: getPmTotalCount(projectItemCounts[project.id]),
      pendingCount: getPmPendingCount(projectItemCounts[project.id]),
    }));
}

export function summarizePmItemTags(tags: string[], limit = 2): PmTagSummary {
  const safeLimit = Math.max(0, limit);
  const visibleTags = tags.slice(0, safeLimit);
  return {
    visibleTags,
    hiddenCount: Math.max(tags.length - visibleTags.length, 0),
  };
}
