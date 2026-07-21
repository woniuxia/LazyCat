import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const rootUrl = new URL("../../../../", import.meta.url);

function readRootFile(path: string): string {
  const url = new URL(path, rootUrl);
  return existsSync(url) ? readFileSync(url, "utf-8") : "";
}

const packageJson = JSON.parse(readRootFile("package.json")) as {
  scripts: Record<string, string>;
};
const wrapperSource = readRootFile("scripts/package-lite-win.ps1");
const nsisBuildSource = readRootFile("scripts/build-tauri-win.ps1");
const agentsSource = readRootFile("AGENTS.md");
const claudeSource = readRootFile("CLAUDE.md");
const readmeSource = readRootFile("README.md");

describe("Windows packaging command guardrails", () => {
  it("exposes one local lite portable packaging command", () => {
    expect(packageJson.scripts["package:win"]).toBe(
      "powershell -ExecutionPolicy Bypass -File scripts/package-lite-win.ps1",
    );
  });

  it("derives the tag from package.json and always skips upload", () => {
    expect(wrapperSource).toContain('$rootPackageJsonPath = Join-Path $repoRoot "package.json"');
    expect(wrapperSource).toContain('$version = [string]$rootPackage.version');
    expect(wrapperSource).toContain('[string]::IsNullOrWhiteSpace($version)');
    expect(wrapperSource).toContain('& $releaseScript -Tag "v$version" -SkipUpload');
    expect(wrapperSource).not.toContain("-AllPackages");
  });

  it("warns when the NSIS build entry is used", () => {
    expect(nsisBuildSource).toContain(
      'Write-Warning "This command builds a Windows NSIS installer. For the default lite portable zip, run: pnpm package:win"',
    );
  });

  it("documents the same default command for agents and users", () => {
    const agentRule = "只说“打包”或“本地打包”时，必须执行 `pnpm package:win`";
    expect(agentsSource).toContain(agentRule);
    expect(claudeSource).toContain(agentRule);
    expect(readmeSource).toContain("`pnpm package:win`");
    expect(readmeSource).toContain("本地构建 lite portable");
  });
});
