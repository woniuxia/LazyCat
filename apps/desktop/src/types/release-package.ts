export type ReleasePackageArtifactMode = "copy_directory" | "zip_directory";
export type ReleasePackageSshAuthType = "password" | "private_key";
export type ReleasePackageType = "local_archive" | "server_upload";
export type ReleasePackageTarget = "frontend" | "backend";
export type ReleasePackagePhase = ReleasePackageTarget | "upload" | "overall";
export type ReleasePackageRunStatus =
  | "idle"
  | "prechecking"
  | "running"
  | "uploading"
  | "succeeded"
  | "partially_succeeded"
  | "package_succeeded_upload_failed"
  | "failed"
  | "upload_succeeded_command_failed"
  | "cancelled";
export type ReleasePackageTargetStatus =
  | "idle"
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "skipped";
export type ReleasePackageCommandStatus = "skipped" | "pending" | "running" | "succeeded" | "failed" | "cancelled";

export interface ReleasePackageUploadConfig {
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuthType: ReleasePackageSshAuthType;
  vaultEntryId: number | null;
  sshPrivateKeyPath: string;
  frontendRemoteDir: string;
  backendRemotePath: string;
}

export interface ReleasePackageProjectDraft extends ReleasePackageUploadConfig {
  name: string;
  packageType: ReleasePackageType;
  outputRoot: string;
  frontendProjectPath: string;
  frontendBuildCommand: string;
  frontendSuccessKeyword: string;
  frontendPostUploadCommand: string;
  frontendArtifactPath: string;
  frontendArtifactMode: ReleasePackageArtifactMode;
  backendProjectPath: string;
  backendBuildCommand: string;
  backendSuccessKeyword: string;
  backendPostUploadCommand: string;
  backendArtifactPath: string;
}

export interface ReleasePackageProject extends ReleasePackageProjectDraft {
  id: number;
  createdAt: string;
  updatedAt: string;
}

export interface ReleasePackageProjectListResult { projects: ReleasePackageProject[] }

export type ReleasePackagePrepareResult =
  | {
      packageType: "local_archive";
      defaultFolderName: string;
      outputRoot: string;
      archivePath: string;
    }
  | { packageType: "server_upload" };

export interface ReleasePackageTargetCheckResult {
  archivePath: string;
  exists: boolean;
}

export interface ReleasePackageRemoteProbeResult {
  probeToken: string;
  host: string;
  port: number;
  keyType: string;
  fingerprintSha256: string;
  trust: "trusted" | "unknown" | "changed";
  previousFingerprintSha256?: string;
}
export interface ReleasePackageRemotePreflightInput {
  projectId: number;
  targets: ReleasePackageTarget[];
  probeToken: string;
  privateKeyPassphrase?: string;
}

export interface ReleasePackageRemoteTargetCheck {
  target: ReleasePackageTarget;
  remotePath: string;
  exists: boolean;
  parentReady: boolean;
  writable: boolean;
}

export interface ReleasePackageCommandRetryPrepareResult extends ReleasePackageRemoteProbeResult {
  targets: ReleasePackageTarget[];
  authType: ReleasePackageSshAuthType;
  username: string;
}

export interface ReleasePackageCommandRetryPreflightResult {
  authToken: string;
  expiresAt: string;
}

export interface ReleasePackageRemotePreflightResult {
  preflightToken: string;
  expiresAt: string;
  targets: ReleasePackageRemoteTargetCheck[];
}
export interface ReleasePackageStartResult { runId: string }
export interface ReleasePackageCancelResult { cancelRequested: boolean }

export interface ReleasePackageUploadProgress {
  uploadedBytes: number;
  totalBytes: number;
  currentPath: string;
}

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
  commandTarget?: ReleasePackageTarget;
  commandStatus?: ReleasePackageCommandStatus;
  commandRetryToken?: string;
  error?: string;
  uploadedBytes?: number;
  totalBytes?: number;
  currentPath?: string;
  retryToken?: string;
}
