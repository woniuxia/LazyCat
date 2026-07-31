import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  test: {
    include: ["src/**/*.test.ts"],
    exclude: ["e2e/**", "node_modules/**", "dist/**", "dist-renderer/**", "src-tauri/target/**"],
  },
});
