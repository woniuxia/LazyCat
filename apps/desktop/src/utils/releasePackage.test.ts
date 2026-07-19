import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import type {
  ReleasePackageLogEvent,
  ReleasePackageProject,
  ReleasePackageRunStatus,
} from "../types/release-package";
import {
  acceptReleasePackageEvent,
  appendReleasePackageLog,
  createDefaultReleasePackageTargets,
  createEmptyReleasePackageDraft,
  isReleasePackageDraftDirty,
  projectToReleasePackageDraft,
  releasePackageRunStatusLabel,
  RELEASE_PACKAGE_COMMAND_EXAMPLES,
  validateReleasePackageDraft,
  validateReleasePackageTargets,
  writeReleasePackageCommand,
} from "./releasePackage";

const project: ReleasePackageProject = {
  id: 7,
  name: "客户门户",
  outputRoot: "D:\\releases",
  frontendProjectPath: "D:\\work\\portal-web",
  frontendBuildCommand: "pnpm build",
  frontendArtifactPath: "dist",
  frontendArtifactMode: "copy_directory",
  backendProjectPath: "D:\\work\\portal-server",
  backendBuildCommand: "mvn clean package -Pprod",
  backendArtifactPath: "target\\portal.jar",
  createdAt: "2026-07-18 10:00:00",
  updatedAt: "2026-07-18 10:00:00",
};

function log(runId: string, line: string): ReleasePackageLogEvent {
  return { runId, projectId: 7, phase: "frontend", stream: "stdout", line };
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
        (example) => /[\u4e00-\u9fff]/u.test(example.title) && /[\u4e00-\u9fff]/u.test(example.description),
      ),
    ).toBe(true);
  });

  it("includes the required environment and Maven command fragments", () => {
    const commands = Object.fromEntries(
      RELEASE_PACKAGE_COMMAND_EXAMPLES.map((example) => [example.id, example.command]),
    );

    expect(commands["java-maven-env"]).toContain('$env:JAVA_HOME =');
    expect(commands["java-maven-env"]).toContain('$env:MAVEN_HOME =');
    expect(commands["java-maven-env"]).toContain('$env:JAVA_HOME\\bin');
    expect(commands["java-maven-env"]).toContain('$env:MAVEN_HOME\\bin');
    expect(commands["java-maven-env"]).toContain('$env:Path');
    expect(commands["maven-build"]).toBe(`mvn clean package -Pprod
if (-not $?) {
  $code = if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { $LASTEXITCODE } else { 1 }
  exit $code
}`);
  });

  describe.runIf(process.platform === "win32")("Maven PowerShell command failure propagation", () => {
    const mavenCommand = RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "maven-build")!.command;

    it("returns a non-zero status when the Maven command cannot be resolved", () => {
      const script = mavenCommand.replace("mvn clean package -Pprod", "__lazycat_missing_maven_command__");
      const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], {
        encoding: "utf8",
      });

      expect(result.status).not.toBe(0);
    });

    it("preserves a native command's non-zero exit code", () => {
      const script = mavenCommand.replace("mvn clean package -Pprod", "cmd.exe /c exit 7");
      const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], {
        encoding: "utf8",
      });

      expect(result.status).toBe(7);
    });
  });

  it("provides complete file copy and move commands", () => {
    expect(RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "copy-file")).toMatchObject({
      command: 'Copy-Item -LiteralPath "D:\\release\\app.jar" -Destination "D:\\deploy\\app.jar" -Force',
    });
    expect(RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "move-file")).toMatchObject({
      command: 'Move-Item -LiteralPath "D:\\release\\app.jar" -Destination "D:\\deploy\\app.jar" -Force',
    });
  });

  it("copies directory contents into an existing destination directory", () => {
    expect(RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "copy-directory")).toMatchObject({
      description: "递归复制目录内容到目标目录，并覆盖同名文件。",
      command: `New-Item -ItemType Directory -Path '.\\release\\config' -Force | Out-Null
Copy-Item -Path '.\\config\\*' -Destination '.\\release\\config' -Recurse -Force`,
    });
  });

  it("moves a directory only when the complete destination does not exist", () => {
    expect(RELEASE_PACKAGE_COMMAND_EXAMPLES.find((example) => example.id === "move-directory")).toMatchObject({
      description: "将指定目录移动到完整目标路径，目标目录需不存在。",
      command: "Move-Item -LiteralPath '.\\release' -Destination '.\\deploy\\release' -Force",
    });
  });

  it("creates a blank project draft with copy mode", () => {
    expect(createEmptyReleasePackageDraft()).toEqual({
      name: "",
      outputRoot: "",
      frontendProjectPath: "",
      frontendBuildCommand: "",
      frontendArtifactPath: "",
      frontendArtifactMode: "copy_directory",
      backendProjectPath: "",
      backendBuildCommand: "",
      backendArtifactPath: "",
    });
  });

  it("defaults each run to both package targets and rejects an empty selection", () => {
    expect(createDefaultReleasePackageTargets()).toEqual(["frontend", "backend"]);
    expect(validateReleasePackageTargets([])).toBe("请至少选择前端包或后端包");
    expect(validateReleasePackageTargets(["backend"])).toBeNull();
  });

  it("normalizes a project into an editable draft and detects dirty fields", () => {
    const draft = projectToReleasePackageDraft(project);
    expect(isReleasePackageDraftDirty(project, draft)).toBe(false);
    draft.frontendBuildCommand = "pnpm build:prod";
    expect(isReleasePackageDraftDirty(project, draft)).toBe(true);
    expect(isReleasePackageDraftDirty(null, createEmptyReleasePackageDraft())).toBe(false);
  });

  it("returns the first required field error", () => {
    const draft = createEmptyReleasePackageDraft();
    expect(validateReleasePackageDraft(draft)).toBe("请输入项目名");
    draft.name = "客户门户";
    expect(validateReleasePackageDraft(draft)).toBe("请选择归档根目录");
    draft.outputRoot = "D:\\releases";
    expect(validateReleasePackageDraft(draft)).toBe("请选择前端工程目录");
  });

  it("accepts events only for the active run", () => {
    expect(acceptReleasePackageEvent("run-1", { runId: "run-1" })).toBe(true);
    expect(acceptReleasePackageEvent("run-1", { runId: "run-2" })).toBe(false);
    expect(acceptReleasePackageEvent(null, { runId: "run-1" })).toBe(false);
  });

  it("bounds logs without reordering accepted lines", () => {
    expect(appendReleasePackageLog([log("run-1", "a"), log("run-1", "b")], log("run-1", "c"), 2))
      .toEqual([log("run-1", "b"), log("run-1", "c")]);
  });

  it.each([
    ["idle", "未运行"],
    ["running", "运行中"],
    ["succeeded", "已完成"],
    ["partially_succeeded", "部分成功"],
    ["failed", "失败"],
    ["cancelled", "已终止"],
  ] satisfies readonly [ReleasePackageRunStatus, string][])("maps %s status to %s", (status, label) => {
    expect(releasePackageRunStatusLabel(status)).toBe(label);
  });

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
