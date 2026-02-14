import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tsconfigPaths from "vite-tsconfig-paths";

export default defineConfig({
  plugins: [vue(), tsconfigPaths()],
  server: {
    port: 5173,
    strictPort: true
  },
  build: {
    outDir: "dist-renderer"
  }
});
