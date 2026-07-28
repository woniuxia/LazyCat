import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  // Vitest and the app resolve different Vite major types in this workspace.
  plugins: [vue() as never],
  test: {
    include: ["src/**/*.test.ts"],
    exclude: ["e2e/**", "node_modules/**", "dist/**", "dist-renderer/**", "src-tauri/target/**"]
  }
});
