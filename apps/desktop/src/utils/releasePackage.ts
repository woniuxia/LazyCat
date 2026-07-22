import type {
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

export function createEmptyReleasePackageDraft(): ReleasePackageProjectDraft {
  return {
    name: "",
    outputRoot: "",
    frontendProjectPath: "",
    frontendBuildCommand: "",
    frontendArtifactPath: "",
    frontendArtifactMode: "copy_directory",
    backendProjectPath: "",
    backendBuildCommand: "",
    backendArtifactPath: "",
    uploadEnabled: false,
    sshHost: "",
    sshPort: 22,
    sshUsername: "",
    sshAuthType: "password",
    sshPrivateKeyPath: "",
    frontendRemoteDir: "",
    backendRemotePath: "",
  };
}

export function projectToReleasePackageDraft(project: ReleasePackageProject): ReleasePackageProjectDraft {
  return {
    name: project.name,
    outputRoot: project.outputRoot,
    frontendProjectPath: project.frontendProjectPath,
    frontendBuildCommand: project.frontendBuildCommand,
    frontendArtifactPath: project.frontendArtifactPath,
    frontendArtifactMode: project.frontendArtifactMode,
    backendProjectPath: project.backendProjectPath,
    backendBuildCommand: project.backendBuildCommand,
    backendArtifactPath: project.backendArtifactPath,
    uploadEnabled: project.uploadEnabled,
    sshHost: project.sshHost,
    sshPort: project.sshPort,
    sshUsername: project.sshUsername,
    sshAuthType: project.sshAuthType,
    sshPrivateKeyPath: project.sshPrivateKeyPath,
    frontendRemoteDir: project.frontendRemoteDir,
    backendRemotePath: project.backendRemotePath,
  };
}

export function normalizeReleasePackageDraft(draft: ReleasePackageProjectDraft): ReleasePackageProjectDraft {
  return Object.fromEntries(
    Object.entries(draft).map(([key, value]) => [key, typeof value === "string" ? value.trim() : value]),
  ) as unknown as ReleasePackageProjectDraft;
}

export function validateReleasePackageUpload(draft: ReleasePackageProjectDraft): string | null {
  const value = normalizeReleasePackageDraft(draft);
  if (!value.uploadEnabled) return null;
  if (!value.sshHost) return "请输入服务器地址";
  if (!Number.isInteger(value.sshPort) || value.sshPort < 1 || value.sshPort > 65_535) {
    return "SSH 端口必须在 1 到 65535 之间";
  }
  if (!value.sshUsername) return "请输入 SSH 用户名";
  if (value.sshAuthType === "private_key" && !value.sshPrivateKeyPath) return "请选择 SSH 私钥文件";
  if (!value.frontendRemoteDir) return "请输入前端远程目录";
  if (!value.frontendRemoteDir.startsWith("/") || value.frontendRemoteDir === "/") {
    return "前端远程目录必须是 Linux 绝对路径";
  }
  if (!value.backendRemotePath) return "请输入后端远程文件路径";
  if (!value.backendRemotePath.startsWith("/") || value.backendRemotePath === "/") {
    return "后端远程文件路径必须是 Linux 绝对路径";
  }
  return null;
}
export function validateReleasePackageDraft(draft: ReleasePackageProjectDraft): string | null {
  const value = normalizeReleasePackageDraft(draft);
  if (!value.name) return "请输入项目名";
  if (!value.outputRoot) return "请选择归档根目录";
  if (!value.frontendProjectPath) return "请选择前端工程目录";
  if (!value.frontendBuildCommand) return "请输入前端构建命令";
  if (!value.frontendArtifactPath) return "请输入前端产物路径";
  if (!value.backendProjectPath) return "请选择后端工程目录";
  if (!value.backendBuildCommand) return "请输入后端构建命令";
  if (!value.backendArtifactPath) return "请输入后端产物路径";
  return validateReleasePackageUpload(value);
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

export function releasePackageRunStatusLabel(status: ReleasePackageRunStatus): string {
  const labels: Record<ReleasePackageRunStatus, string> = {
    idle: "未运行",
    running: "运行中",
    succeeded: "已完成",
    partially_succeeded: "部分成功",
    failed: "失败",
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
