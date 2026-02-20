import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tsconfigPaths from "vite-tsconfig-paths";
import AutoImport from "unplugin-auto-import/vite";
import Components from "unplugin-vue-components/vite";
import { ElementPlusResolver } from "unplugin-vue-components/resolvers";

export default defineConfig({
  plugins: [
    vue(),
    tsconfigPaths(),
    AutoImport({
      resolvers: [ElementPlusResolver()],
    }),
    Components({
      resolvers: [ElementPlusResolver()],
    }),
  ],
  server: {
    port: 5173,
    strictPort: true
  },
  build: {
    outDir: "dist-renderer",
    rollupOptions: {
      output: {
        manualChunks: {
          "element-plus": ["element-plus"],
          "monaco-editor": ["monaco-editor"],
        },
      },
    },
  }
});
