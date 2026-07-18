export type ReleasePackageArtifactMode = "copy_directory" | "zip_directory";
export type ReleasePackagePhase = "frontend" | "backend" | "archive";
export type ReleasePackageRunStatus = "idle" | "running" | "succeeded" | "failed" | "cancelled";

export interface ReleasePackageProjectDraft {
  name: string;
  frontendProjectPath: string;
  frontendBuildCommand: string;
  frontendArtifactPath: string;
  frontendArtifactMode: ReleasePackageArtifactMode;
  backendProjectPath: string;
  backendBuildCommand: string;
  backendArtifactPath: string;
}

export interface ReleasePackageProject extends ReleasePackageProjectDraft {
  id: number;
  createdAt: string;
  updatedAt: string;
}

export interface ReleasePackageProjectListResult { projects: ReleasePackageProject[] }

export interface ReleasePackagePrepareResult {
  defaultFolderName: string;
  outputRoot: string;
  archivePath: string;
  frontendArtifactMode: ReleasePackageArtifactMode;
}

export interface ReleasePackageStartResult { runId: string }
export interface ReleasePackageCancelResult { cancelRequested: boolean }

export interface ReleasePackageLogEvent {
  runId: string;
  projectId: number;
  phase: ReleasePackagePhase;
  stream: "stdout" | "stderr" | "system";
  line: string;
}

export interface ReleasePackageStatusEvent {
  runId: string;
  projectId: number;
  status: Exclude<ReleasePackageRunStatus, "idle">;
  phase: ReleasePackagePhase;
  archivePath?: string;
  error?: string;
}
