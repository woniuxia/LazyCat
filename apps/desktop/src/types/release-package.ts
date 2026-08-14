export type ReleasePackageArtifactMode = "copy_directory" | "zip_directory";
export type ReleasePackageSshAuthType = "password" | "private_key";
export type ReleasePackageType = "local_archive" | "server_upload";
export type ReleasePackageEnvironmentKind = "test" | "production";
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
  | "deployed_health_check_failed"
  | "cancelled";
export type ReleasePackageTargetStatus =
  | "idle"
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "skipped";
export type ReleasePackageCommandStatus =
  | "skipped"
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

export interface ReleasePackageUploadConfig {
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuthType: ReleasePackageSshAuthType;
  vaultEntryId: number | null;
  sshPrivateKeyPath: string;
  frontendRemoteDir: string;
  backendRemotePath: string;
  postUploadCommandTimeoutSeconds: number;
}

export interface ReleasePackageProjectDraft {
  name: string;
  frontendProjectPath: string;
  backendProjectPath: string;
}

export interface ReleasePackageEnvironmentDraft extends ReleasePackageUploadConfig {
  packageType: ReleasePackageType;
  outputRoot: string;
  frontendExpectedBranch: string;
  frontendBuildCommand: string;
  frontendSuccessKeyword: string;
  frontendPostUploadCommand: string;
  frontendArtifactPath: string;
  frontendArtifactMode: ReleasePackageArtifactMode;
  backendExpectedBranch: string;
  backendBuildCommand: string;
  backendSuccessKeyword: string;
  backendPostUploadCommand: string;
  backendArtifactPath: string;
  healthCheckEnabled: boolean;
  healthCheckUrl: string;
  healthCheckMaxRetries: number;
}

export interface ReleasePackageEnvironmentConfig extends ReleasePackageEnvironmentDraft {
  id: number;
  projectId: number;
  environment: ReleasePackageEnvironmentKind;
  configured: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ReleasePackageProject extends ReleasePackageProjectDraft {
  id: number;
  environments: ReleasePackageEnvironmentConfig[];
  recentUsageCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface ReleasePackageProjectListResult {
  projects: ReleasePackageProject[];
}

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

export interface ReleasePackageBranchCheck {
  target: ReleasePackageTarget;
  expectedBranch: string;
  currentBranch?: string;
  detachedCommit?: string;
  matches: boolean;
}

export interface ReleasePackageBranchCheckResult {
  checks: ReleasePackageBranchCheck[];
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
  vaultEntryId: number | null;
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
export interface ReleasePackageStartResult {
  runId: string;
}
export interface ReleasePackageCancelResult {
  cancelRequested: boolean;
}

export interface ReleasePackageUploadProgress {
  uploadedBytes: number;
  totalBytes: number;
  currentPath: string;
}

export interface ReleasePackageLogEvent {
  runId: string;
  environmentId: number;
  projectId: number;
  environment: ReleasePackageEnvironmentKind;
  phase: ReleasePackagePhase;
  stream: "stdout" | "stderr" | "system";
  line: string;
}

export interface ReleasePackageStatusEvent {
  runId: string;
  environmentId: number;
  projectId: number;
  environment: ReleasePackageEnvironmentKind;
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
