export type ReleasePackageArtifactMode = "copy_directory" | "zip_directory";
export type ReleasePackageSshAuthType = "password" | "private_key";
export type ReleasePackageTarget = "frontend" | "backend";
export type ReleasePackagePhase = ReleasePackageTarget | "overall";
export type ReleasePackageRunStatus =
  | "idle"
  | "running"
  | "succeeded"
  | "partially_succeeded"
  | "failed"
  | "cancelled";
export type ReleasePackageTargetStatus =
  | "idle"
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "skipped";

export interface ReleasePackageUploadConfig {
  uploadEnabled: boolean;
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuthType: ReleasePackageSshAuthType;
  sshPrivateKeyPath: string;
  frontendRemoteDir: string;
  backendRemotePath: string;
}

export interface ReleasePackageProjectDraft extends ReleasePackageUploadConfig {
  name: string;
  outputRoot: string;
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

export interface ReleasePackageTargetCheckResult {
  archivePath: string;
  exists: boolean;
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
