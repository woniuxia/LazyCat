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
    expect(source).toContain("prepareResult.value?.outputRoot");
    expect(source).toContain("prepareResult.value.archivePath");
    expect(source).toContain("const refreshed = await loadProjects()");
  });
});
