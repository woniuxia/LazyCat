import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./ReleasePackagePanel.vue", import.meta.url), "utf8");

describe("ReleasePackagePanel", () => {
  it("uses a master-detail workspace and explicit run confirmation", () => {
    expect(source).toContain('class="release-package-projects"');
    expect(source).toContain('class="release-package-editor"');
    expect(source).toContain('class="release-package-log"');
    expect(source).toContain("确认打包");
    expect(source).toContain("终止打包");
  });

  it("uses all release-package actions and awaited global setting persistence", () => {
    for (const channel of [
      "project-list",
      "project-create",
      "project-update",
      "project-delete",
      "prepare",
      "start",
    ]) {
      expect(source).toContain(`tool:release-package:${channel}`);
    }
    expect(source).toContain("setSettingAndWait");
    expect(source).toContain("useReleasePackageRuntime");
    expect(source).toContain("tool:system:open-local-path");
  });

  it("keeps runtime listeners alive across panel navigation", () => {
    expect(source).toContain("await initSettings()");
    expect(source).toContain("await runtime.ensureListeners()");
    expect(source).not.toContain("onUnmounted");
  });

  it("does not persist logs or silently overwrite archives", () => {
    expect(source).not.toContain("localStorage");
    expect(source).not.toContain("overwrite");
  });

  it("keeps the archive root chooser authoritative and validates Windows folder names", () => {
    expect(source).toContain('v-model="outputRoot"');
    expect(source).toContain("readonly");
    expect(source).toContain("validateArchiveFolderName");
    expect(source).toContain("COM[1-9]");
    expect(source).toContain("cancelPendingStart");
  });

  it("restores the active runtime project and uses prepare paths after refresh", () => {
    expect(source).toContain("runtime.activeProjectId");
    expect(source).toContain(
      'const preferActiveProject = (selectedId.value === null && !dirty.value) || runtime.status.value === "running"',
    );
    expect(source).toContain("prepareResult.value?.outputRoot");
    expect(source).toContain("prepareResult.value.archivePath");
    expect(source).toContain("const refreshed = await loadProjects()");
  });

  it("clears a deleted project before attempting to refresh", () => {
    const deleteStart = source.indexOf("async function deleteProject");
    const removeProject = source.indexOf("projects.value = projects.value.filter", deleteStart);
    const clearSelection = source.indexOf("selectedId.value = null", deleteStart);
    const clearDraft = source.indexOf("Object.assign(draft, createEmptyReleasePackageDraft())", deleteStart);
    const refresh = source.indexOf("const refreshed = await loadProjects()", deleteStart);

    expect(deleteStart).toBeGreaterThan(-1);
    expect(removeProject).toBeGreaterThan(deleteStart);
    expect(clearSelection).toBeGreaterThan(removeProject);
    expect(clearDraft).toBeGreaterThan(clearSelection);
    expect(refresh).toBeGreaterThan(clearDraft);
  });

  it("uses a responsive engineering workspace with multiline command editors", () => {
    expect(source).toContain('class="engineering-grid"');
    expect(source).toContain('class="engineering-card frontend-card"');
    expect(source).toContain('class="engineering-card backend-card"');
    expect(source.match(/type="textarea"/g)).toHaveLength(2);
    expect(source.match(/:autosize="\{ minRows: 4, maxRows: 9 \}"/g)).toHaveLength(2);
    expect(source).toContain("同一 PowerShell 会话中顺序执行");
    expect(source).toContain("$LASTEXITCODE");
  });

  it("renders command examples and reports clipboard failures", () => {
    expect(source).toContain("RELEASE_PACKAGE_COMMAND_EXAMPLES");
    expect(source.match(/常用示例/g)?.length).toBeGreaterThanOrEqual(2);
    expect(source).toContain("CopyDocument");
    expect(source).toContain("async function copyCommandExample(command: string)");
    expect(source).toContain("await navigator.clipboard.writeText(command)");
    expect(source).toContain('ElMessage.success("命令示例已复制")');
    expect(source).toContain("showError(error)");
    expect(source).toContain('popper-class="release-package-command-examples"');
    expect(source).toContain(":global(.release-package-command-examples)");
  });

  it("wraps logs in a white status card", () => {
    expect(source).toContain('class="release-package-log-card"');
    expect(source).toContain('class="log-status-tag"');
    expect(source).toContain("statusLabels");
    for (const label of ["未运行", "运行中", "已完成", "失败", "已终止"]) {
      expect(source).toContain(label);
    }
    expect(source).toMatch(/\.release-package-log\s*\{[^}]*background:\s*#fff;/s);
    expect(source).toContain('ref="logContainer"');
    expect(source).toContain('aria-live="polite"');
  });
});
