import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const root = new URL("../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), "utf-8");

describe("Vault security settings placement", () => {
  const globalSettings = read("components/SettingsPanel.vue");
  const vault = read("components/VaultPanel.vue");
  const securityDialog = read("components/VaultSecuritySettingsDialog.vue");

  it("keeps Vault-specific security controls out of global settings", () => {
    expect(globalSettings).not.toContain("vaultLockSettings");
    expect(globalSettings).not.toContain("加密与安全");
  });

  it("exposes the security settings from the password manager", () => {
    expect(vault).toContain("<span>安全设置</span>");
    expect(vault).toContain('<VaultSecuritySettingsDialog ref="securitySettingsDialog" />');
    expect(securityDialog).toContain("setVaultLockSettingAndWait");
    expect(securityDialog).toContain("密码库无活动自动锁定");
    expect(securityDialog).toContain("电脑无操作自动锁定");
  });
});
