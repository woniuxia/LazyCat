import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import type {
  ReleasePackageEnvironmentConfig,
  ReleasePackageLogEvent,
  ReleasePackageProject,
  ReleasePackageRunStatus,
  ReleasePackageType,
} from "../types/release-package";
import {
  acceptReleasePackageEvent,
  appendReleasePackageLog,
  createReleasePackageStartPayload,
  createDefaultReleasePackageTargets,
  createEmptyReleasePackageEnvironmentDraft,
  createEmptyReleasePackageProjectDraft,
  environmentToReleasePackageDraft,
  isReleasePackageDraftDirty,
  normalizeReleasePackageEnvironmentDraft,
  normalizeReleasePackageProjectDraft,
  normalizeVaultServerPort,
  projectToReleasePackageProjectDraft,
  releasePackageRunStatusLabel,
  RELEASE_PACKAGE_BACKEND_COMMAND_EXAMPLES,
  RELEASE_PACKAGE_COMMAND_EXAMPLES,
  validateReleasePackageEnvironmentDraft,
  validateReleasePackageProjectDraft,
  validateReleasePackageUpload,
  validateReleasePackageTargets,
  writeReleasePackageCommand,
} from "./releasePackage";

const project: ReleasePackageProject = {
  id: 7,
  name: "客户门户",
  recentUsageCount: 0,
  frontendProjectPath: "D:\\work\\portal-web",
  backendProjectPath: "D:\\work\\portal-server",
  environments: [],
  createdAt: "2026-07-18 10:00:00",
  updatedAt: "2026-07-18 10:00:00",
};

const testEnvironment: ReleasePackageEnvironmentConfig = {
  id: 11,
  projectId: 7,
  environment: "test",
  configured: true,
  packageType: "local_archive",
  outputRoot: "D:\\releases\\test",
  frontendExpectedBranch: "master",
  frontendBuildCommand: "pnpm build:test",
  frontendSuccessKeyword: "Build completed",
  frontendPostUploadCommand: "cd /srv/test/web\n./reload.sh",
  frontendArtifactPath: "dist-test",
  frontendArtifactMode: "copy_directory",
  backendExpectedBranch: "master",
  backendBuildCommand: "mvn clean package -Ptest",
  backendSuccessKeyword: "BUILD SUCCESS",
  backendPostUploadCommand: "systemctl restart portal-test",
  backendArtifactPath: "target\\portal-test.jar",
  sshHost: "",
  sshPort: 22,
  sshUsername: "",
  sshAuthType: "password",
  vaultEntryId: null,
  sshPrivateKeyPath: "",
  frontendRemoteDir: "/srv/test/web",
  backendRemotePath: "/srv/test/portal.jar",
  healthCheckEnabled: false,
  healthCheckUrl: "",
  healthCheckMaxRetries: 6,
  createdAt: "2026-07-18 10:00:00",
  updatedAt: "2026-07-18 10:00:00",
};

const productionEnvironment: ReleasePackageEnvironmentConfig = {
  ...testEnvironment,
  id: 12,
  environment: "production",
  outputRoot: "D:\\releases\\production",
  frontendBuildCommand: "pnpm build:prod",
  frontendPostUploadCommand: "cd /srv/prod/web\n./reload.sh",
  frontendArtifactPath: "dist-prod",
  backendBuildCommand: "mvn clean package -Pprod",
  backendPostUploadCommand: "systemctl restart portal",
  backendArtifactPath: "target\\portal.jar",
  frontendRemoteDir: "/srv/prod/web",
  backendRemotePath: "/srv/prod/portal.jar",
};

project.environments = [testEnvironment, productionEnvironment];

function log(runId: string, line: string): ReleasePackageLogEvent {
  return {
    runId,
    projectId: 7,
    environmentId: 11,
    environment: "test",
    phase: "frontend",
    stream: "stdout",
    line,
  };
}

describe("release package view helpers", () => {
  it("provides PowerShell command examples in the expected order", () => {
    expect(RELEASE_PACKAGE_COMMAND_EXAMPLES.map((example) => example.id)).toEqual([
      "java-maven-env",
      "maven-build",
      "copy-file",
      "copy-directory",
      "move-file",
      "move-directory",
    ]);
    expect(
      RELEASE_PACKAGE_COMMAND_EXAMPLES.every(
        (example) =>
          /[\u4e00-\u9fff]/u.test(example.title) && /[\u4e00-\u9fff]/u.test(example.description),
      ),
    ).toBe(true);
  });

  it("includes the required environment and Maven command fragments", () => {
    const commands = Object.fromEntries(
      RELEASE_PACKAGE_COMMAND_EXAMPLES.map((example) => [example.id, example.command]),
    );

    expect(commands["java-maven-env"]).toContain("$env:JAVA_HOME =");
    expect(commands["java-maven-env"]).toContain("$env:MAVEN_HOME =");
    expect(commands["java-maven-env"]).toContain("$env:JAVA_HOME\\bin");
    expect(commands["java-maven-env"]).toContain("$env:MAVEN_HOME\\bin");
    expect(commands["java-maven-env"]).toContain("$env:Path");
    expect(commands["maven-build"]).toBe(`mvn clean package -Pprod
if (-not $?) {
  $code = if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { $LASTEXITCODE } else { 1 }
  exit $code
}`);
  });

  it("provides a backend-only Maven settings file build example", () => {
    const example = RELEASE_PACKAGE_BACKEND_COMMAND_EXAMPLES.find(
      (item) => item.id === "maven-build-settings",
    );

    expect(example).toMatchObject({
      title: "使用指定 settings.xml 构建",
      description: "通过 Maven settings.xml 指定仓库、镜像和认证配置，并在构建失败时退出脚本。",
    });
    expect(example?.command).toContain(
      'mvn --settings "C:\\Tools\\maven\\conf\\settings.xml" clean package -Pprod',
    );
    expect(example?.command).toContain("$LASTEXITCODE");
    expect(
      RELEASE_PACKAGE_COMMAND_EXAMPLES.some((item) => String(item.id) === "maven-build-settings"),
    ).toBe(false);
  });

  describe.runIf(process.platform === "win32")(
    "Maven PowerShell command failure propagation",
    () => {
      const mavenCommand = RELEASE_PACKAGE_COMMAND_EXAMPLES.find(
        (example) => example.id === "maven-build",
      )!.command;

      it("returns a non-zero status when the Maven command cannot be resolved", () => {
        const script = mavenCommand.replace(
          "mvn clean package -Pprod",
          "__lazycat_missing_maven_command__",
        );
        const result = spawnSync(
          "powershell.exe",
          ["-NoProfile", "-NonInteractive", "-Command", script],
          {
            encoding: "utf8",
          },
        );

        expect(result.status).not.toBe(0);
      });

      it("preserves a native command's non-zero exit code", () => {
        const script = mavenCommand.replace("mvn clean package -Pprod", "cmd.exe /c exit 7");
        const result = spawnSync(
          "powershell.exe",
          ["-NoProfile", "-NonInteractive", "-Command", script],
          {
            encoding: "utf8",
          },
        );

        expect(result.status).toBe(7);
      });
    },
  );

  it("provides complete file copy and move commands", () => {
    expect(
      RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "copy-file"),
    ).toMatchObject({
      command:
        'Copy-Item -LiteralPath "D:\\release\\app.jar" -Destination "D:\\deploy\\app.jar" -Force',
    });
    expect(
      RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "move-file"),
    ).toMatchObject({
      command:
        'Move-Item -LiteralPath "D:\\release\\app.jar" -Destination "D:\\deploy\\app.jar" -Force',
    });
  });

  it("copies directory contents into an existing destination directory", () => {
    expect(
      RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "copy-directory"),
    ).toMatchObject({
      description: "递归复制目录内容到目标目录，并覆盖同名文件。",
      command: `New-Item -ItemType Directory -Path '.\\release\\config' -Force | Out-Null
Copy-Item -Path '.\\config\\*' -Destination '.\\release\\config' -Recurse -Force`,
    });
  });

  it("moves a directory only when the complete destination does not exist", () => {
    expect(
      RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "move-directory"),
    ).toMatchObject({
      description: "将指定目录移动到完整目标路径，目标目录需不存在。",
      command: "Move-Item -LiteralPath '.\\release' -Destination '.\\deploy\\release' -Force",
    });
  });

  it("creates an exact blank project draft", () => {
    expect(createEmptyReleasePackageProjectDraft()).toEqual({
      name: "",
      frontendProjectPath: "",
      backendProjectPath: "",
    });
  });

  it("creates a blank environment draft with all existing defaults", () => {
    expect(createEmptyReleasePackageEnvironmentDraft()).toEqual({
      packageType: "local_archive",
      outputRoot: "",
      frontendExpectedBranch: "master",
      frontendBuildCommand: "",
      frontendSuccessKeyword: "",
      frontendPostUploadCommand: "",
      frontendArtifactPath: "",
      frontendArtifactMode: "copy_directory",
      backendExpectedBranch: "master",
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
      healthCheckEnabled: false,
      healthCheckUrl: "",
      healthCheckMaxRetries: 6,
    });
  });

  it.each([
    [undefined, 22],
    [1, 1],
    [2200, 2200],
    [65_535, 65_535],
    [null, null],
    [-1, null],
    [0, null],
    [65_536, null],
    [22.5, null],
    ["22", null],
    [Number.NaN, null],
    [Number.POSITIVE_INFINITY, null],
  ])("normalizes Vault server port %s to %s", (value, expected) => {
    expect(normalizeVaultServerPort(value)).toBe(expected);
  });

  it("defaults each run to both package targets and rejects an empty selection", () => {
    expect(createDefaultReleasePackageTargets()).toEqual(["frontend", "backend"]);
    expect(validateReleasePackageTargets([])).toBe("请至少选择前端包或后端包");
    expect(validateReleasePackageTargets(["backend"])).toBeNull();
  });

  it("maps public and environment records into independent drafts", () => {
    const testDraft = environmentToReleasePackageDraft(testEnvironment);
    const productionDraft = environmentToReleasePackageDraft(productionEnvironment);

    expect(projectToReleasePackageProjectDraft(project)).toEqual({
      name: "客户门户",
      frontendProjectPath: "D:\\work\\portal-web",
      backendProjectPath: "D:\\work\\portal-server",
    });
    expect(testEnvironment.frontendBuildCommand).not.toBe(
      productionEnvironment.frontendBuildCommand,
    );
    expect(testEnvironment.frontendRemoteDir).not.toBe(productionEnvironment.frontendRemoteDir);
    expect(testEnvironment.backendRemotePath).not.toBe(productionEnvironment.backendRemotePath);
    expect(testDraft).toEqual({
      packageType: testEnvironment.packageType,
      outputRoot: testEnvironment.outputRoot,
      frontendExpectedBranch: testEnvironment.frontendExpectedBranch,
      frontendBuildCommand: testEnvironment.frontendBuildCommand,
      frontendSuccessKeyword: testEnvironment.frontendSuccessKeyword,
      frontendPostUploadCommand: testEnvironment.frontendPostUploadCommand,
      frontendArtifactPath: testEnvironment.frontendArtifactPath,
      frontendArtifactMode: testEnvironment.frontendArtifactMode,
      backendExpectedBranch: testEnvironment.backendExpectedBranch,
      backendBuildCommand: testEnvironment.backendBuildCommand,
      backendSuccessKeyword: testEnvironment.backendSuccessKeyword,
      backendPostUploadCommand: testEnvironment.backendPostUploadCommand,
      backendArtifactPath: testEnvironment.backendArtifactPath,
      sshHost: testEnvironment.sshHost,
      sshPort: testEnvironment.sshPort,
      sshUsername: testEnvironment.sshUsername,
      sshAuthType: testEnvironment.sshAuthType,
      vaultEntryId: testEnvironment.vaultEntryId,
      sshPrivateKeyPath: testEnvironment.sshPrivateKeyPath,
      frontendRemoteDir: testEnvironment.frontendRemoteDir,
      backendRemotePath: testEnvironment.backendRemotePath,
      healthCheckEnabled: testEnvironment.healthCheckEnabled,
      healthCheckUrl: testEnvironment.healthCheckUrl,
      healthCheckMaxRetries: testEnvironment.healthCheckMaxRetries,
    });
    expect(productionDraft).toEqual({
      packageType: productionEnvironment.packageType,
      outputRoot: productionEnvironment.outputRoot,
      frontendExpectedBranch: productionEnvironment.frontendExpectedBranch,
      frontendBuildCommand: productionEnvironment.frontendBuildCommand,
      frontendSuccessKeyword: productionEnvironment.frontendSuccessKeyword,
      frontendPostUploadCommand: productionEnvironment.frontendPostUploadCommand,
      frontendArtifactPath: productionEnvironment.frontendArtifactPath,
      frontendArtifactMode: productionEnvironment.frontendArtifactMode,
      backendExpectedBranch: productionEnvironment.backendExpectedBranch,
      backendBuildCommand: productionEnvironment.backendBuildCommand,
      backendSuccessKeyword: productionEnvironment.backendSuccessKeyword,
      backendPostUploadCommand: productionEnvironment.backendPostUploadCommand,
      backendArtifactPath: productionEnvironment.backendArtifactPath,
      sshHost: productionEnvironment.sshHost,
      sshPort: productionEnvironment.sshPort,
      sshUsername: productionEnvironment.sshUsername,
      sshAuthType: productionEnvironment.sshAuthType,
      vaultEntryId: productionEnvironment.vaultEntryId,
      sshPrivateKeyPath: productionEnvironment.sshPrivateKeyPath,
      frontendRemoteDir: productionEnvironment.frontendRemoteDir,
      backendRemotePath: productionEnvironment.backendRemotePath,
      healthCheckEnabled: productionEnvironment.healthCheckEnabled,
      healthCheckUrl: productionEnvironment.healthCheckUrl,
      healthCheckMaxRetries: productionEnvironment.healthCheckMaxRetries,
    });

    const projectDraft = projectToReleasePackageProjectDraft(project);
    expect(isReleasePackageDraftDirty(project, testEnvironment, projectDraft, testDraft)).toBe(
      false,
    );
    expect(
      isReleasePackageDraftDirty(project, productionEnvironment, projectDraft, productionDraft),
    ).toBe(false);
    expect(
      isReleasePackageDraftDirty(project, testEnvironment, projectDraft, productionDraft),
    ).toBe(true);
    expect(
      isReleasePackageDraftDirty(project, productionEnvironment, projectDraft, testDraft),
    ).toBe(true);
  });

  it("detects public and environment changes through four arguments", () => {
    const projectDraft = projectToReleasePackageProjectDraft(project);
    const environmentDraft = environmentToReleasePackageDraft(testEnvironment);

    expect(
      isReleasePackageDraftDirty(project, testEnvironment, projectDraft, environmentDraft),
    ).toBe(false);
    expect(
      isReleasePackageDraftDirty(
        project,
        testEnvironment,
        { ...projectDraft, name: "changed" },
        environmentDraft,
      ),
    ).toBe(true);
    expect(
      isReleasePackageDraftDirty(project, testEnvironment, projectDraft, {
        ...environmentDraft,
        frontendBuildCommand: "changed",
      }),
    ).toBe(true);
    expect(
      isReleasePackageDraftDirty(
        null,
        null,
        createEmptyReleasePackageProjectDraft(),
        createEmptyReleasePackageEnvironmentDraft(),
      ),
    ).toBe(false);
  });

  it("does not mark saved surrounding whitespace as dirty after draft normalization", () => {
    const savedProject = {
      ...project,
      name: "  客户门户  ",
      frontendProjectPath: "  D:\\work\\portal-web  ",
      backendProjectPath: "  D:\\work\\portal-server  ",
    };
    const savedEnvironment = {
      ...testEnvironment,
      outputRoot: "  D:\\releases\\test  ",
      frontendBuildCommand: "  pnpm build:test  ",
      frontendPostUploadCommand: "\n  cd /srv/test/web\n  ./reload.sh\n",
    };
    const projectDraft = projectToReleasePackageProjectDraft(savedProject);
    const environmentDraft = environmentToReleasePackageDraft(savedEnvironment);

    expect(
      isReleasePackageDraftDirty(
        savedProject,
        savedEnvironment,
        normalizeReleasePackageProjectDraft(projectDraft),
        normalizeReleasePackageEnvironmentDraft(environmentDraft),
      ),
    ).toBe(false);
  });

  it("does not mark equal drafts with different property insertion order as dirty", () => {
    const projectDraft = projectToReleasePackageProjectDraft(project);
    const reorderedProjectDraft = {
      backendProjectPath: projectDraft.backendProjectPath,
      frontendProjectPath: projectDraft.frontendProjectPath,
      name: projectDraft.name,
    };
    const environmentDraft = environmentToReleasePackageDraft(testEnvironment);
    const reorderedEnvironmentDraft = Object.fromEntries(
      Object.entries(environmentDraft).reverse(),
    ) as typeof environmentDraft;

    expect(
      isReleasePackageDraftDirty(project, testEnvironment, reorderedProjectDraft, environmentDraft),
    ).toBe(false);
    expect(
      isReleasePackageDraftDirty(project, testEnvironment, projectDraft, reorderedEnvironmentDraft),
    ).toBe(false);
  });

  it("trims only surrounding whitespace in public and environment strings", () => {
    const projectDraft = projectToReleasePackageProjectDraft(project);
    projectDraft.name = "  客户门户  ";
    const environmentDraft = environmentToReleasePackageDraft(testEnvironment);
    environmentDraft.frontendSuccessKeyword = "  Build completed  ";
    environmentDraft.frontendPostUploadCommand = "\n  cd /srv/web\n  ./reload.sh\n";
    environmentDraft.backendSuccessKeyword = "  BUILD SUCCESS  ";
    environmentDraft.backendPostUploadCommand = "\n  systemctl restart portal\n";

    expect(normalizeReleasePackageProjectDraft(projectDraft).name).toBe("客户门户");
    const normalized = normalizeReleasePackageEnvironmentDraft(environmentDraft);
    expect(normalized.frontendSuccessKeyword).toBe("Build completed");
    expect(normalized.frontendPostUploadCommand).toBe("cd /srv/web\n  ./reload.sh");
    expect(normalized.backendSuccessKeyword).toBe("BUILD SUCCESS");
    expect(normalized.backendPostUploadCommand).toBe("systemctl restart portal");
  });

  it("returns the first required public and environment field errors", () => {
    const projectDraft = createEmptyReleasePackageProjectDraft();
    expect(validateReleasePackageProjectDraft(projectDraft)).toBe("请输入项目名");
    projectDraft.name = "客户门户";
    expect(validateReleasePackageProjectDraft(projectDraft)).toBe("请选择前端工程目录");

    const environmentDraft = createEmptyReleasePackageEnvironmentDraft();
    expect(validateReleasePackageEnvironmentDraft(environmentDraft)).toBe("请选择归档根目录");
    environmentDraft.outputRoot = "D:\\releases";
    environmentDraft.frontendExpectedBranch = "";
    expect(validateReleasePackageEnvironmentDraft(environmentDraft)).toBe("请输入前端生产分支");
    environmentDraft.frontendExpectedBranch = "master";
    expect(validateReleasePackageEnvironmentDraft(environmentDraft)).toBe("请输入前端构建命令");
  });

  it("validates enabled server upload settings", () => {
    const draft = createEmptyReleasePackageEnvironmentDraft();
    draft.packageType = "server_upload";
    expect(validateReleasePackageUpload(draft)).toBe("请选择密码库服务器凭据");
    draft.vaultEntryId = 17;
    draft.frontendRemoteDir = "/srv/app/web";
    draft.backendRemotePath = "/srv/app/app.jar";
    expect(validateReleasePackageUpload(draft)).toBeNull();
    draft.frontendRemoteDir = "relative/web";
    expect(validateReleasePackageUpload(draft)).toBe("前端远程目录必须是 Linux 绝对路径");
    draft.frontendRemoteDir = "/srv/app/web";
    draft.sshAuthType = "private_key";
    expect(validateReleasePackageUpload(draft)).toBe("请输入服务器地址");
    draft.sshHost = "10.0.0.8";
    expect(validateReleasePackageUpload(draft)).toBe("请输入 SSH 用户名");
    draft.sshUsername = "deploy";
    expect(validateReleasePackageUpload(draft)).toBe("请选择 SSH 私钥文件");
  });

  it("validates deployment health check settings only when enabled", () => {
    const draft = createEmptyReleasePackageEnvironmentDraft();
    Object.assign(draft, {
      packageType: "server_upload",
      frontendBuildCommand: "pnpm build",
      frontendArtifactPath: "dist",
      backendBuildCommand: "mvn package",
      backendArtifactPath: "target/app.jar",
      vaultEntryId: 17,
      frontendRemoteDir: "/srv/app/web",
      backendRemotePath: "/srv/app/app.jar",
    });
    expect(validateReleasePackageEnvironmentDraft(draft)).toBeNull();

    draft.healthCheckEnabled = true;
    expect(validateReleasePackageEnvironmentDraft(draft)).toBe(
      "健康检查地址必须使用 http 或 https",
    );
    draft.healthCheckUrl = "https://portal.example.com/health";
    draft.healthCheckMaxRetries = 61;
    expect(validateReleasePackageEnvironmentDraft(draft)).toBe(
      "健康检查最多重试次数必须在 0 到 60 之间",
    );
    draft.healthCheckMaxRetries = 0;
    expect(validateReleasePackageEnvironmentDraft(draft)).toBeNull();
  });

  it("validates the project SSH port only for private-key upload", () => {
    const draft = createEmptyReleasePackageEnvironmentDraft();
    Object.assign(draft, {
      packageType: "server_upload",
      sshAuthType: "password",
      vaultEntryId: 17,
      sshPort: 0,
      frontendRemoteDir: "/srv/portal/web",
      backendRemotePath: "/srv/portal/app.jar",
    });

    expect(validateReleasePackageUpload(draft)).toBeNull();

    Object.assign(draft, {
      sshAuthType: "private_key",
      sshHost: "deploy.example.internal",
      sshUsername: "deploy",
      sshPrivateKeyPath: "C:\\Users\\deploy\\.ssh\\id_ed25519",
    });
    expect(validateReleasePackageUpload(draft)).toBe("SSH 端口必须在 1 到 65535 之间");
  });

  it("requires a Vault credential only for password upload", () => {
    const draft = createEmptyReleasePackageEnvironmentDraft();
    Object.assign(draft, {
      packageType: "server_upload",
      sshAuthType: "password",
      frontendRemoteDir: "/srv/portal/web",
      backendRemotePath: "/srv/portal/app.jar",
    });

    expect(validateReleasePackageUpload(draft)).toBe("请选择密码库服务器凭据");
    draft.vaultEntryId = 42;
    expect(validateReleasePackageUpload(draft)).toBeNull();

    draft.sshAuthType = "private_key";
    draft.vaultEntryId = null;
    expect(validateReleasePackageUpload(draft)).toBe("请输入服务器地址");
    draft.sshHost = "deploy.example.internal";
    expect(validateReleasePackageUpload(draft)).toBe("请输入 SSH 用户名");
    draft.sshUsername = "deploy";
    expect(validateReleasePackageUpload(draft)).toBe("请选择 SSH 私钥文件");
  });

  it("maps and compares vaultEntryId as part of the environment draft", () => {
    const withBinding = { ...testEnvironment, vaultEntryId: 9 };
    const draft = environmentToReleasePackageDraft(withBinding);
    expect(draft.vaultEntryId).toBe(9);
    expect(
      isReleasePackageDraftDirty(
        project,
        withBinding,
        projectToReleasePackageProjectDraft(project),
        draft,
      ),
    ).toBe(false);
    draft.vaultEntryId = 10;
    expect(
      isReleasePackageDraftDirty(
        project,
        withBinding,
        projectToReleasePackageProjectDraft(project),
        draft,
      ),
    ).toBe(true);
  });
  it("rejects non-canonical Linux deployment paths before preflight", () => {
    const draft = createEmptyReleasePackageEnvironmentDraft();
    Object.assign(draft, {
      packageType: "server_upload",
      sshHost: "10.0.0.8",
      sshUsername: "deploy",
      vaultEntryId: 17,
      frontendRemoteDir: "/srv/app/web/",
      backendRemotePath: "/srv/app/app.jar",
    });
    expect(validateReleasePackageUpload(draft)).toBe("前端远程目录必须是规范的 Linux 绝对路径");
    draft.frontendRemoteDir = "/srv/app/web";
    draft.backendRemotePath = "/srv//app.jar";
    expect(validateReleasePackageUpload(draft)).toBe("后端远程文件路径必须是规范的 Linux 绝对路径");
  });
  it("validates only fields required by the selected package type", () => {
    const draft = environmentToReleasePackageDraft(testEnvironment);
    draft.packageType = "server_upload";
    draft.outputRoot = "";
    draft.vaultEntryId = 17;
    draft.frontendRemoteDir = "/srv/app/web";
    draft.backendRemotePath = "/srv/app/app.jar";
    expect(validateReleasePackageEnvironmentDraft(draft)).toBeNull();

    draft.vaultEntryId = null;
    expect(validateReleasePackageEnvironmentDraft(draft)).toBe("请选择密码库服务器凭据");

    draft.packageType = "local_archive";
    expect(validateReleasePackageEnvironmentDraft(draft)).toBe("请选择归档根目录");
  });

  it.each([
    {
      packageType: "local_archive",
      expected: {
        environmentId: 11,
        targets: ["frontend", "backend"],
        folderName: "portal-20260722",
        overwriteExisting: true,
        productionConfirmed: true,
      },
    },
    {
      packageType: "server_upload",
      expected: {
        environmentId: 11,
        targets: ["frontend", "backend"],
        preflightToken: "preflight-1",
        overwriteRemoteTargets: ["frontend"],
        productionConfirmed: true,
      },
    },
  ] satisfies ReadonlyArray<{
    packageType: ReleasePackageType;
    expected: Record<string, unknown>;
  }>)("builds only $packageType start parameters", ({ packageType, expected }) => {
    expect(
      createReleasePackageStartPayload(packageType, {
        environmentId: 11,
        targets: ["frontend", "backend"],
        folderName: "portal-20260722",
        overwriteExisting: true,
        preflightToken: "preflight-1",
        overwriteRemoteTargets: ["frontend"],
        productionConfirmed: true,
      }),
    ).toEqual(expected);
  });

  it.each([undefined, "", "legacy_upload"])(
    "rejects invalid start package type %s",
    (packageType) => {
      expect(() =>
        createReleasePackageStartPayload(packageType, {
          environmentId: 11,
          targets: ["frontend"],
          folderName: "portal-20260722",
          overwriteExisting: false,
          preflightToken: "preflight-1",
          overwriteRemoteTargets: [],
          productionConfirmed: false,
        }),
      ).toThrow("打包类型无效，请重新打开确认窗口");
    },
  );

  it("adds a dispatch id only to action-triggered starts", () => {
    expect(
      createReleasePackageStartPayload("local_archive", {
        environmentId: 11,
        targets: ["frontend", "backend"],
        folderName: "20260725-客户门户",
        overwriteExisting: false,
        preflightToken: "",
        overwriteRemoteTargets: [],
        productionConfirmed: false,
        actionDispatchId: "dispatch-1",
      }),
    ).toMatchObject({ actionDispatchId: "dispatch-1" });

    expect(
      createReleasePackageStartPayload("local_archive", {
        environmentId: 11,
        targets: ["frontend"],
        folderName: "manual",
        overwriteExisting: false,
        preflightToken: "",
        overwriteRemoteTargets: [],
        productionConfirmed: false,
      }),
    ).not.toHaveProperty("actionDispatchId");
  });

  it("adds production confirmation only for strict true", () => {
    const input = {
      environmentId: 12,
      targets: ["frontend"] as const,
      folderName: "production",
      overwriteExisting: false,
      preflightToken: "",
      overwriteRemoteTargets: [] as const,
      productionConfirmed: false,
    };

    expect(createReleasePackageStartPayload("local_archive", input)).not.toHaveProperty(
      "productionConfirmed",
    );
    expect(
      createReleasePackageStartPayload("local_archive", {
        ...input,
        productionConfirmed: "true" as unknown as boolean,
      }),
    ).not.toHaveProperty("productionConfirmed");
    expect(
      createReleasePackageStartPayload("local_archive", {
        ...input,
        productionConfirmed: 1 as unknown as boolean,
      }),
    ).not.toHaveProperty("productionConfirmed");
    expect(
      createReleasePackageStartPayload("local_archive", { ...input, productionConfirmed: true }),
    ).toHaveProperty("productionConfirmed", true);
  });

  it("accepts events only for the active run", () => {
    expect(acceptReleasePackageEvent("run-1", { runId: "run-1" })).toBe(true);
    expect(acceptReleasePackageEvent("run-1", { runId: "run-2" })).toBe(false);
    expect(acceptReleasePackageEvent(null, { runId: "run-1" })).toBe(false);
  });

  it("bounds logs without reordering accepted lines", () => {
    expect(
      appendReleasePackageLog([log("run-1", "a"), log("run-1", "b")], log("run-1", "c"), 2),
    ).toEqual([log("run-1", "b"), log("run-1", "c")]);
  });

  it.each([
    ["idle", "未运行"],
    ["running", "运行中"],
    ["prechecking", "预检中"],
    ["uploading", "上传中"],
    ["succeeded", "已完成"],
    ["partially_succeeded", "部分成功"],
    ["package_succeeded_upload_failed", "构建完成，上传失败"],
    ["failed", "失败"],
    ["cancelled", "已终止"],
  ] satisfies readonly [ReleasePackageRunStatus, string][])(
    "maps %s status to %s",
    (status, label) => {
      expect(releasePackageRunStatusLabel(status)).toBe(label);
    },
  );

  it("writes the exact command once", async () => {
    const written: string[] = [];
    const writeText = async (value: string): Promise<void> => {
      written.push(value);
    };

    await writeReleasePackageCommand("mvn clean package -Pprod", writeText);

    expect(written).toEqual(["mvn clean package -Pprod"]);
  });

  it("propagates the original clipboard write error", async () => {
    const originalError = new Error("clipboard denied");
    const writeText = async (): Promise<void> => {
      throw originalError;
    };

    await expect(writeReleasePackageCommand("pnpm build", writeText)).rejects.toBe(originalError);
  });
});
