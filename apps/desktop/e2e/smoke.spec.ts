import { expect, test } from "@playwright/test";

test("renders lazycat shell and navigation", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/Lazycat/i).first()).toBeVisible();
  await expect(page.locator(".home-panel").first()).toBeVisible();
  await expect(page.locator(".home-tool-card").first()).toBeVisible();
});

test("base64 tool shows bridge warning in web mode", async ({ page }) => {
  await page.goto("/");
  const firstRunPrompt = page.getByRole("button", { name: "暂不启用" });
  if (await firstRunPrompt.isVisible({ timeout: 3000 }).catch(() => false)) {
    await firstRunPrompt.click();
  }
  await page
    .locator(".home-tool-card", { hasText: "Base64" })
    .first()
    .click();
  await page.locator("textarea").first().fill("lazycat");
  await page.getByRole("button", { name: /Base64 编码/ }).first().click();
  await expect(page.getByText(/IPC bridge.*Tauri/i).first()).toBeVisible({ timeout: 10000 });
});
