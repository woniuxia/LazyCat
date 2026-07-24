import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./ReleasePackagePanel.vue", import.meta.url), "utf8");
const appSource = readFileSync(new URL("../App.vue", import.meta.url), "utf8");

describe("ReleasePackagePanel", () => {
  it("uses a master-detail workspace and explicit run confirmation", () => {
    expect(source).toContain('class="release-package-projects"');
    expect(source).toContain('class="release-package-editor"');
    expect(source).toMatch(
      /<section class="project-overview">[\s\S]*<header class="editor-header">[\s\S]*<div class="project-basics">/u,
    );
    expect(source).toContain('class="release-package-log"');
    expect(source).toContain("确认本地归档");
    expect(source).toContain("确认上传");
    expect(source).toContain("终止打包");
  });

  it("edits the project name from the header without a duplicate basics field", () => {
    expect(source).not.toContain('class="editor-hint"');
    expect(source).not.toContain('<el-form-item label="项目名称"');
    expect(source).toContain('ref="projectTitleInput"');
    expect(source).toContain('v-model="draft.name"');
    expect(source).toContain('@dblclick="startTitleEdit"');
    expect(source).toContain('@keydown.enter.prevent="startTitleEdit"');
    expect(source).toContain('@blur="finishTitleEdit"');
    expect(source).toContain('@keydown.enter.stop.prevent="finishTitleEdit"');
  });

  it("uses all release-package actions without global setting persistence", () => {
    for (const channel of [
      "project-list",
      "project-create",
      "project-update",
      "project-delete",
      "prepare",
      "target-check",
      "start",
    ]) {
      expect(source).toContain(`tool:release-package:${channel}`);
    }
    expect(source).not.toContain("setSettingAndWait");
    expect(source).not.toContain("release_package.output_root");
    expect(source).toContain("useReleasePackageRuntime");
    expect(source).toContain("tool:system:open-local-path");
  });

  it("keeps runtime listeners alive across panel navigation", () => {
    expect(source).toContain("await runtime.ensureListeners()");
    expect(source).not.toContain("onUnmounted");
  });

  it("does not persist logs or expose a persistent overwrite preference", () => {
    expect(source).not.toContain("localStorage");
    expect(source).not.toContain('v-model="overwriteExisting"');
  });

  it("checks an existing target before start and requires explicit overwrite confirmation", () => {
    expect(source).toContain("tool:release-package:target-check");
    expect(source).toContain("目标归档目录已存在。直接覆盖将完整替换其中的所有文件，此操作无法撤销。");
    expect(source).toContain('confirmButtonText: "直接覆盖"');
    expect(source).toContain('cancelButtonText: "取消"');
    expect(source).toContain("overwriteExisting");
    expect(source.indexOf("tool:release-package:target-check")).toBeLessThan(
      source.indexOf("runtime.beginStart"),
    );
  });

  it("stores the archive root in each project and validates Windows folder names", () => {
    expect(source).toContain('v-model="draft.outputRoot"');
    expect(source).toContain("readonly");
    expect(source).toContain("draft.outputRoot = path");
    expect(source).toContain("validateArchiveFolderName");
    expect(source).toContain("COM[1-9]");
    expect(source).toContain("cancelPendingStart");
  });

  it("restores the active runtime project and uses prepare paths after refresh", () => {
    expect(source).toContain("runtime.activeProjectId");
    expect(source).toContain(
      'const preferActiveProject = (selectedId.value === null && !dirty.value) || runtime.status.value === "running"',
    );
    expect(source).toContain('prepareResult.value?.packageType !== "local_archive"');
    expect(source).toContain("prepareResult.value.outputRoot");
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
    expect(source).toMatch(
      /\.engineering-grid\s*\{[^}]*grid-template-columns:\s*repeat\(auto-fit,\s*minmax\(min\(100%,\s*380px\),\s*1fr\)\);/s,
    );
    expect(source.match(/type="textarea"/g)).toHaveLength(2);
    expect(source.match(/:autosize="\{ minRows: 4, maxRows: 9 \}"/g)).toHaveLength(2);
    expect(source).toContain("同一 PowerShell 会话中顺序执行");
    expect(source).toContain("$LASTEXITCODE");
  });

  it("lets the outer page scroll through the full workspace", () => {
    expect(source).toMatch(/\.release-package-panel\s*\{[^}]*flex:\s*0 0 auto;/s);
    expect(source).toMatch(/\.release-package-workspace\s*\{[^}]*overflow:\s*visible;/s);
  });

  it("renders command examples and reports clipboard failures", () => {
    const copyFunctionStart = source.indexOf("async function copyCommandExample");
    const nextAsyncFunction = source.indexOf("\nasync function", copyFunctionStart + 1);
    const copyFunctionSource = source.slice(copyFunctionStart, nextAsyncFunction);

    expect(source).toContain("RELEASE_PACKAGE_COMMAND_EXAMPLES");
    expect(source.match(/常用示例/g)?.length).toBeGreaterThanOrEqual(2);
    expect(source).toContain("CopyDocument");
    expect(copyFunctionStart).toBeGreaterThan(-1);
    expect(nextAsyncFunction).toBeGreaterThan(copyFunctionStart);
    expect(copyFunctionSource).toContain("await writeReleasePackageCommand(");
    expect(copyFunctionSource).toContain("(value) => navigator.clipboard.writeText(value)");
    expect(copyFunctionSource).toContain('ElMessage.success("命令示例已复制")');
    expect(copyFunctionSource).toContain("showError(error)");
    expect(source.match(/popper-class="release-package-command-examples"/g) ?? []).toHaveLength(2);
    expect(source.match(/:aria-label="`复制\$\{example\.title\}命令`"/g) ?? []).toHaveLength(2);
    expect(source).toContain(":global(.release-package-command-examples)");
  });

  it("wraps logs in a white status card", () => {
    expect(source).toContain('class="release-package-log-card release-package-project-log"');
    expect(source).toContain('class="log-status"');
    expect(source).toContain("computed(() => releasePackageRunStatusLabel(status.value))");
    expect(source).toContain("{{ statusLabel }}");
    expect(source).toMatch(
      /<el-tag\s+class="log-status"\s+role="status"\s+aria-live="polite"\s+aria-atomic="true"/u,
    );
    expect(source).toMatch(/\.release-package-log\s*\{[^}]*background:\s*#fff;/s);
    expect(source).toMatch(/\.log-card-header p\s*\{[^}]*color:\s*#5f6b7a;/s);
    expect(source).toMatch(/\.log-meta\s*\{[^}]*color:\s*#5f6b7a;/s);
    for (const [variant, textColor] of [
      ["primary", "#1d4ed8"],
      ["success", "#237a3b"],
      ["info", "#4b5563"],
      ["warning", "#8a4b08"],
      ["danger", "#b42318"],
    ]) {
      expect(source).toContain(`:deep(.log-status.el-tag--${variant})`);
      expect(source).toContain(`--el-tag-text-color: ${textColor};`);
    }
    expect(source).toContain('ref="frontendLogContainer"');
    expect(source).toContain('ref="backendLogContainer"');
    expect(source).toContain('aria-live="polite"');
  });

  it("selects artifact paths, run targets, and renders project-scoped log columns", () => {
    expect(source).toContain("chooseFrontendArtifact");
    expect(source).toContain("chooseBackendArtifact");
    expect(source).toContain('chooseDirectory("选择前端产物目录")');
    expect(source).toContain('chooseFile("选择后端产物文件")');
    expect(source.indexOf("归档目录名")).toBeLessThan(source.indexOf("本次打包内容"));
    expect(source).toContain('label="前端包"');
    expect(source).toContain('label="后端包"');
    expect(source).toContain("createDefaultReleasePackageTargets()");
    expect(source).toContain("createReleasePackageStartPayload(packageType");
    expect(source).toContain("targets: selectedTargets.value");
    expect(source).toContain("release-package-project-log");
    expect(source).toContain('class="release-package-log-columns"');
    expect(source).toContain('ref="frontendLogContainer"');
    expect(source).toContain('ref="backendLogContainer"');
  });

  it("adds an accessible archive shortcut to both log lanes after a successful archive", () => {
    expect(source.match(/class="log-lane-actions"/g) ?? []).toHaveLength(2);
    expect(source.match(/aria-label="打开归档目录"/g) ?? []).toHaveLength(2);
    expect(source.match(/@click="openArchive"/g) ?? []).toHaveLength(3);
  });

  it("configures upload separately and preflights before runtime start", () => {
    expect(source).toContain('v-model="draft.packageType"');
    for (const model of [
      "draft.sshHost",
      "draft.sshPort",
      "draft.sshUsername",
      "draft.sshAuthType",
      "draft.sshPrivateKeyPath",
      "draft.frontendRemoteDir",
      "draft.backendRemotePath",
    ]) {
      expect(source).toContain(`v-model="${model}"`);
    }
    expect(source).toContain("useReleasePackageUploadPreflight");
    expect(source).toContain("tool:release-package:upload-retry");
    expect(source.indexOf("await uploadPreflight.check")).toBeLessThan(
      source.indexOf("runtime.beginStart"),
    );
    expect(source).toContain('type="password"');
    expect(source).toContain('credentialSecret.value = ""');
    expect(source).not.toContain("draft.password");
  });

  it("binds a Vault server credential for password auth without rendering a password field", () => {
    expect(source).toContain('label="密码库凭据"');
    expect(source).toContain('v-model="draft.vaultEntryId"');
    expect(source).toContain('tool:vault:meta-list');
    expect(source).toContain('v-if="draft.sshAuthType === \'password\'"');
    expect(source).toContain("密码由密码库提供");
    expect(source).not.toContain("请输入服务器密码");
    expect(source).not.toContain("? { password: credentialSecret.value }");
  });

  it("uses the Vault server port for password auth and keeps manual port input private-key only", () => {
    expect(source).toContain(
      '<el-form-item v-if="draft.sshAuthType === \'private_key\'" label="SSH 端口" required>',
    );
    expect(source).not.toContain('<el-form-item label="SSH 端口" required>');
    expect(source).toContain("port?: unknown");
    expect(source).toContain("normalizeVaultServerPort(entry.plainFields?.port)");
    expect(source).toContain("complete: Boolean(address && account && port !== null)");
    expect(source).toContain("{{ selectedVaultCredential.port }}");
    expect(source).toContain("缺少地址、端口、账号或密码");
  });

  it("keeps only the private-key passphrase input in the start dialog", () => {
    expect(source).toContain("draft.sshAuthType === 'private_key'");
    expect(source).toContain("privateKeyPassphrase: credentialSecret.value || undefined");
  });

  it("opens the Vault through the application tool navigation event", () => {
    expect(source).toContain('emit("open-tool", "vault")');
    expect(appSource).toContain('@open-tool="onSelect"');
  });

  it("renders an explicit state when the saved Vault binding no longer exists", () => {
    expect(source).toContain('class="vault-binding-invalid"');
    expect(source).toContain("绑定的密码库凭据已失效，请重新选择");
  });

  it("renders a separate upload lane and explicit remote replacement confirmation", () => {
    expect(source).toContain("上传日志");
    expect(source).toContain(
      '<section v-if="draft.packageType === \'server_upload\'" class="release-package-log-lane upload-log-lane">',
    );
    expect(source).toContain(":class=\"{ 'has-upload-lane': draft.packageType === 'server_upload' }\"");
    expect(source).toContain("uploadProgress");
    expect(source).toContain("完整替换以上远程目标");
    expect(source).toContain("package_succeeded_upload_failed");
    expect(source).toContain("重试上传");
  });

  it("renders mutually exclusive package types and type-specific fields", () => {
    expect(source).toContain('v-model="draft.packageType"');
    expect(source).toContain('value="local_archive"');
    expect(source).toContain('value="server_upload"');
    expect(source).toContain("draft.packageType === 'local_archive'");
    expect(source).toContain("draft.packageType === 'server_upload'");
    expect(source).not.toContain("draft.uploadEnabled");
    expect(source).not.toContain("startMode");
  });

  it("runs only the delivery checks required by the prepared package type", () => {
    const start = source.slice(source.indexOf("async function confirmStart"));
    expect(start).toContain("confirmArchiveOverwrite");
    expect(start).toContain("runUploadPreflight");
    expect(source).not.toContain("mode: startMode.value");
  });
});
