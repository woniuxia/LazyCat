import { expect, test } from "@playwright/test";

test("renders lazycat shell and navigation", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/Lazycat/i).first()).toBeVisible();
  await expect(page.locator(".el-menu").first()).toBeVisible();
});

test("base64 tool shows bridge warning in web mode", async ({ page }) => {
  await page.goto("/");
  await page.locator("textarea").first().fill("lazycat");
  await page.getByRole("button", { name: /Base64/i }).first().click();
  await expect(page.getByText(/IPC bridge.*Tauri/i).first()).toBeVisible({ timeout: 10000 });
});
