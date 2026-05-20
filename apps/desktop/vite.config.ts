import { defineConfig } from "vite";
import { resolve } from "path";
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
      resolvers: [ElementPlusResolver({ importStyle: false })],
    }),
    Components({
      resolvers: [ElementPlusResolver({ importStyle: false })],
    }),
  ],
  resolve: {
    alias: {
      "frappe-gantt/dist/frappe-gantt.css": resolve(__dirname, "node_modules/frappe-gantt/dist/frappe-gantt.css"),
    },
  },
  server: {
    port: 5173,
    strictPort: true
  },
  build: {
    outDir: "dist-renderer",
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        spotlight: resolve(__dirname, "spotlight.html"),
      },
      output: {
        manualChunks: {
          "element-plus": ["element-plus"],
        },
      },
    },
  }
});
