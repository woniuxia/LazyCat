import type {
  ReleasePackageLogEvent,
  ReleasePackageProject,
  ReleasePackageProjectDraft,
} from "../types/release-package";

export function createEmptyReleasePackageDraft(): ReleasePackageProjectDraft {
  return {
    name: "",
    frontendProjectPath: "",
    frontendBuildCommand: "",
    frontendArtifactPath: "",
    frontendArtifactMode: "copy_directory",
    backendProjectPath: "",
    backendBuildCommand: "",
    backendArtifactPath: "",
  };
}

export function projectToReleasePackageDraft(project: ReleasePackageProject): ReleasePackageProjectDraft {
  return {
    name: project.name,
    frontendProjectPath: project.frontendProjectPath,
    frontendBuildCommand: project.frontendBuildCommand,
    frontendArtifactPath: project.frontendArtifactPath,
    frontendArtifactMode: project.frontendArtifactMode,
    backendProjectPath: project.backendProjectPath,
    backendBuildCommand: project.backendBuildCommand,
    backendArtifactPath: project.backendArtifactPath,
  };
}

export function normalizeReleasePackageDraft(draft: ReleasePackageProjectDraft): ReleasePackageProjectDraft {
  return Object.fromEntries(
    Object.entries(draft).map(([key, value]) => [key, typeof value === "string" ? value.trim() : value]),
  ) as unknown as ReleasePackageProjectDraft;
}

export function validateReleasePackageDraft(draft: ReleasePackageProjectDraft): string | null {
  const value = normalizeReleasePackageDraft(draft);
  if (!value.name) return "请输入项目名";
  if (!value.frontendProjectPath) return "请选择前端工程目录";
  if (!value.frontendBuildCommand) return "请输入前端构建命令";
  if (!value.frontendArtifactPath) return "请输入前端产物路径";
  if (!value.backendProjectPath) return "请选择后端工程目录";
  if (!value.backendBuildCommand) return "请输入后端构建命令";
  if (!value.backendArtifactPath) return "请输入后端产物路径";
  return null;
}

export function isReleasePackageDraftDirty(
  project: ReleasePackageProject | null,
  draft: ReleasePackageProjectDraft,
): boolean {
  if (!project) {
    return JSON.stringify(normalizeReleasePackageDraft(draft)) !== JSON.stringify(createEmptyReleasePackageDraft());
  }
  return JSON.stringify(projectToReleasePackageDraft(project)) !== JSON.stringify(normalizeReleasePackageDraft(draft));
}

export function acceptReleasePackageEvent(
  activeRunId: string | null,
  event: { runId: string },
): boolean {
  return activeRunId !== null && activeRunId === event.runId;
}

export function appendReleasePackageLog(
  current: ReleasePackageLogEvent[],
  event: ReleasePackageLogEvent,
  limit = 2_000,
): ReleasePackageLogEvent[] {
  const next = [...current, event];
  return next.length > limit ? next.slice(next.length - limit) : next;
}
