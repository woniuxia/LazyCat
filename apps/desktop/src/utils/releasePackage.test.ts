import { describe, expect, it } from "vitest";
import type { ReleasePackageLogEvent, ReleasePackageProject } from "../types/release-package";
import {
  acceptReleasePackageEvent,
  appendReleasePackageLog,
  createEmptyReleasePackageDraft,
  isReleasePackageDraftDirty,
  projectToReleasePackageDraft,
  RELEASE_PACKAGE_COMMAND_EXAMPLES,
  validateReleasePackageDraft,
} from "./releasePackage";

const project: ReleasePackageProject = {
  id: 7,
  name: "客户门户",
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
    expect(commands["maven-build"]).toMatch(
      /mvn clean package -Pprod\r?\nif \(\$LASTEXITCODE -ne 0\) \{ exit \$LASTEXITCODE \}/u,
    );
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
      frontendProjectPath: "",
      frontendBuildCommand: "",
      frontendArtifactPath: "",
      frontendArtifactMode: "copy_directory",
      backendProjectPath: "",
      backendBuildCommand: "",
      backendArtifactPath: "",
    });
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
});
