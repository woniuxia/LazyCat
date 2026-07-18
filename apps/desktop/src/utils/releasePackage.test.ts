import { describe, expect, it } from "vitest";
import type { ReleasePackageLogEvent, ReleasePackageProject } from "../types/release-package";
import {
  acceptReleasePackageEvent,
  appendReleasePackageLog,
  createEmptyReleasePackageDraft,
  isReleasePackageDraftDirty,
  projectToReleasePackageDraft,
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
