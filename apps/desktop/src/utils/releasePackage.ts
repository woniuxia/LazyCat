import type {
  ReleasePackageEnvironmentConfig,
  ReleasePackageEnvironmentDraft,
  ReleasePackageLogEvent,
  ReleasePackageProject,
  ReleasePackageProjectDraft,
  ReleasePackageRunStatus,
  ReleasePackageTarget,
} from "../types/release-package";

export interface ReleasePackageCommandExample {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly command: string;
}

export const RELEASE_PACKAGE_COMMAND_EXAMPLES = [
  {
    id: "java-maven-env",
    title: "配置 Java 与 Maven 环境",
    description: "在当前 PowerShell 会话中设置 Java、Maven 环境变量和命令搜索路径。",
    command: `$env:JAVA_HOME = "C:\\Program Files\\Java\\jdk-17"
$env:MAVEN_HOME = "C:\\Tools\\apache-maven-3.9.9"
$env:Path = "$env:JAVA_HOME\\bin;$env:MAVEN_HOME\\bin;$env:Path"`,
  },
  {
    id: "maven-build",
    title: "执行 Maven 生产构建",
    description: "使用生产配置构建，并在 Maven 命令失败时立即退出脚本。",
    command: `mvn clean package -Pprod
if (-not $?) {
  $code = if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { $LASTEXITCODE } else { 1 }
  exit $code
}`,
  },
  {
    id: "copy-file",
    title: "复制文件",
    description: "将指定文件复制到目标路径，并覆盖已存在的文件。",
    command: `Copy-Item -LiteralPath "D:\\release\\app.jar" -Destination "D:\\deploy\\app.jar" -Force`,
  },
  {
    id: "copy-directory",
    title: "复制目录",
    description: "递归复制目录内容到目标目录，并覆盖同名文件。",
    command: `New-Item -ItemType Directory -Path '.\\release\\config' -Force | Out-Null
Copy-Item -Path '.\\config\\*' -Destination '.\\release\\config' -Recurse -Force`,
  },
  {
    id: "move-file",
    title: "移动文件",
    description: "将指定文件移动到目标路径，并覆盖已存在的文件。",
    command: `Move-Item -LiteralPath "D:\\release\\app.jar" -Destination "D:\\deploy\\app.jar" -Force`,
  },
  {
    id: "move-directory",
    title: "移动目录",
    description: "将指定目录移动到完整目标路径，目标目录需不存在。",
    command: `Move-Item -LiteralPath '.\\release' -Destination '.\\deploy\\release' -Force`,
  },
] as const satisfies readonly ReleasePackageCommandExample[];

export function createDefaultReleasePackageTargets(): ReleasePackageTarget[] {
  return ["frontend", "backend"];
}

export function validateReleasePackageTargets(targets: readonly ReleasePackageTarget[]): string | null {
  return targets.length === 0 ? "请至少选择前端包或后端包" : null;
}

export interface ReleasePackageStartPayloadInput {
  environmentId: number;
  targets: readonly ReleasePackageTarget[];
  folderName: string;
  overwriteExisting: boolean;
  preflightToken: string;
  overwriteRemoteTargets: readonly ReleasePackageTarget[];
  productionConfirmed: boolean;
  actionDispatchId?: string;
}

export type ReleasePackageStartPayload =
  | {
      environmentId: number;
      targets: ReleasePackageTarget[];
      folderName: string;
      overwriteExisting: boolean;
      productionConfirmed?: true;
      actionDispatchId?: string;
    }
  | {
      environmentId: number;
      targets: ReleasePackageTarget[];
      preflightToken: string;
      overwriteRemoteTargets: ReleasePackageTarget[];
      productionConfirmed?: true;
      actionDispatchId?: string;
    };

export function createReleasePackageStartPayload(
  packageType: string | null | undefined,
  input: ReleasePackageStartPayloadInput,
): ReleasePackageStartPayload {
  const common = {
    environmentId: input.environmentId,
    targets: [...input.targets],
    ...(input.productionConfirmed === true
      ? { productionConfirmed: true as const }
      : {}),
    ...(input.actionDispatchId !== undefined
      ? { actionDispatchId: input.actionDispatchId }
      : {}),
  };
  if (packageType === "local_archive") {
    return {
      ...common,
      folderName: input.folderName,
      overwriteExisting: input.overwriteExisting,
    };
  }
  if (packageType === "server_upload") {
    return {
      ...common,
      preflightToken: input.preflightToken,
      overwriteRemoteTargets: [...input.overwriteRemoteTargets],
    };
  }
  throw new Error("打包类型无效，请重新打开确认窗口");
}

export function createEmptyReleasePackageProjectDraft(): ReleasePackageProjectDraft {
  return {
    name: "",
    frontendProjectPath: "",
    backendProjectPath: "",
  };
}

export function createEmptyReleasePackageEnvironmentDraft(): ReleasePackageEnvironmentDraft {
  return {
    packageType: "local_archive",
    outputRoot: "",
    frontendBuildCommand: "",
    frontendSuccessKeyword: "",
    frontendPostUploadCommand: "",
    frontendArtifactPath: "",
    frontendArtifactMode: "copy_directory",
    backendBuildCommand: "",
    backendSuccessKeyword: "",
    backendPostUploadCommand: "",
    backendArtifactPath: "",
    sshHost: "",
    sshPort: 22,
    sshUsername: "",
    sshAuthType: "password",
    vaultEntryId: null,
    sshPrivateKeyPath: "",
    frontendRemoteDir: "",
    backendRemotePath: "",
  };
}

export function normalizeVaultServerPort(value: unknown): number | null {
  if (value === undefined) return 22;
  return typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 65_535
    ? value
    : null;
}

export function projectToReleasePackageProjectDraft(project: ReleasePackageProject): ReleasePackageProjectDraft {
  return {
    name: project.name,
    frontendProjectPath: project.frontendProjectPath,
    backendProjectPath: project.backendProjectPath,
  };
}

export function environmentToReleasePackageDraft(
  environment: ReleasePackageEnvironmentConfig,
): ReleasePackageEnvironmentDraft {
  return {
    packageType: environment.packageType,
    outputRoot: environment.outputRoot,
    frontendBuildCommand: environment.frontendBuildCommand,
    frontendSuccessKeyword: environment.frontendSuccessKeyword,
    frontendPostUploadCommand: environment.frontendPostUploadCommand,
    frontendArtifactPath: environment.frontendArtifactPath,
    frontendArtifactMode: environment.frontendArtifactMode,
    backendBuildCommand: environment.backendBuildCommand,
    backendSuccessKeyword: environment.backendSuccessKeyword,
    backendPostUploadCommand: environment.backendPostUploadCommand,
    backendArtifactPath: environment.backendArtifactPath,
    sshHost: environment.sshHost,
    sshPort: environment.sshPort,
    sshUsername: environment.sshUsername,
    sshAuthType: environment.sshAuthType,
    vaultEntryId: environment.vaultEntryId,
    sshPrivateKeyPath: environment.sshPrivateKeyPath,
    frontendRemoteDir: environment.frontendRemoteDir,
    backendRemotePath: environment.backendRemotePath,
  };
}

function normalizeDraft<T extends object>(draft: T): T {
  return Object.fromEntries(
    Object.entries(draft).map(([key, value]) => [key, typeof value === "string" ? value.trim() : value]),
  ) as T;
}

export function normalizeReleasePackageProjectDraft(
  draft: ReleasePackageProjectDraft,
): ReleasePackageProjectDraft {
  return normalizeDraft(draft);
}

export function normalizeReleasePackageEnvironmentDraft(
  draft: ReleasePackageEnvironmentDraft,
): ReleasePackageEnvironmentDraft {
  return normalizeDraft(draft);
}

export function validateReleasePackageUpload(draft: ReleasePackageEnvironmentDraft): string | null {
  const value = normalizeReleasePackageEnvironmentDraft(draft);
  if (value.sshAuthType === "password" && value.vaultEntryId === null) {
    return "请选择密码库服务器凭据";
  }
  if (value.sshAuthType === "private_key") {
    if (!Number.isInteger(value.sshPort) || value.sshPort < 1 || value.sshPort > 65_535) {
      return "SSH 端口必须在 1 到 65535 之间";
    }
    if (!value.sshHost) return "请输入服务器地址";
    if (!value.sshUsername) return "请输入 SSH 用户名";
    if (!value.sshPrivateKeyPath) return "请选择 SSH 私钥文件";
  }
  if (!value.frontendRemoteDir) return "请输入前端远程目录";
  if (!value.frontendRemoteDir.startsWith("/") || value.frontendRemoteDir === "/") {
    return "前端远程目录必须是 Linux 绝对路径";
  }
  if (!isCanonicalLinuxPath(value.frontendRemoteDir)) {
    return "前端远程目录必须是规范的 Linux 绝对路径";
  }
  if (!value.backendRemotePath) return "请输入后端远程文件路径";
  if (!value.backendRemotePath.startsWith("/") || value.backendRemotePath === "/") {
    return "后端远程文件路径必须是 Linux 绝对路径";
  }
  if (!isCanonicalLinuxPath(value.backendRemotePath)) {
    return "后端远程文件路径必须是规范的 Linux 绝对路径";
  }
  return null;
}

function isCanonicalLinuxPath(value: string): boolean {
  return !value.endsWith("/")
    && !value.includes("//")
    && !value.includes("\\")
    && !value.includes("\0")
    && !value.split("/").some((segment) => segment === "." || segment === "..");
}
export function validateReleasePackageProjectDraft(draft: ReleasePackageProjectDraft): string | null {
  const value = normalizeReleasePackageProjectDraft(draft);
  if (!value.name) return "请输入项目名";
  if (!value.frontendProjectPath) return "请选择前端工程目录";
  if (!value.backendProjectPath) return "请选择后端工程目录";
  return null;
}

export function validateReleasePackageEnvironmentDraft(draft: ReleasePackageEnvironmentDraft): string | null {
  const value = normalizeReleasePackageEnvironmentDraft(draft);
  if (value.packageType === "local_archive" && !value.outputRoot) return "请选择归档根目录";
  if (!value.frontendBuildCommand) return "请输入前端构建命令";
  if (!value.frontendArtifactPath) return "请输入前端产物路径";
  if (!value.backendBuildCommand) return "请输入后端构建命令";
  if (!value.backendArtifactPath) return "请输入后端产物路径";
  return value.packageType === "server_upload" ? validateReleasePackageUpload(value) : null;
}

export function isReleasePackageDraftDirty(
  project: ReleasePackageProject | null,
  environment: ReleasePackageEnvironmentConfig | null,
  projectDraft: ReleasePackageProjectDraft,
  environmentDraft: ReleasePackageEnvironmentDraft,
): boolean {
  const savedProject = project
    ? projectToReleasePackageProjectDraft(project)
    : createEmptyReleasePackageProjectDraft();
  const savedEnvironment = environment
    ? environmentToReleasePackageDraft(environment)
    : createEmptyReleasePackageEnvironmentDraft();
  return JSON.stringify(savedProject) !== JSON.stringify(normalizeReleasePackageProjectDraft(projectDraft))
    || JSON.stringify(savedEnvironment) !== JSON.stringify(normalizeReleasePackageEnvironmentDraft(environmentDraft));
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

export function releasePackageRunStatusLabel(status: ReleasePackageRunStatus): string {
  const labels: Record<ReleasePackageRunStatus, string> = {
    idle: "未运行",
    prechecking: "预检中",
    running: "运行中",
    uploading: "上传中",
    succeeded: "已完成",
    partially_succeeded: "部分成功",
    package_succeeded_upload_failed: "构建完成，上传失败",
    failed: "失败",
    upload_succeeded_command_failed: "文件已上传，命令失败",
    cancelled: "已终止",
  };
  return labels[status];
}

export async function writeReleasePackageCommand(
  command: string,
  writeText: (value: string) => Promise<void>,
): Promise<void> {
  await writeText(command);
}
