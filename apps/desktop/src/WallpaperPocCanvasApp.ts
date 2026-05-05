// PoC canvas mount entry — 渲染 360×800 仪表盘 mock，挂载完成后通知后端
import { createApp } from "vue";
import { invoke } from "@tauri-apps/api/core";
import WallpaperPocCanvas from "./components/WallpaperPocCanvas.vue";
import "./styles/index.css";

export default function mount() {
  const app = createApp(WallpaperPocCanvas);
  app.mount("#app");

  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const t = performance.now();
      // 通知后端 canvas 已就绪（PoC 暂不消费，仅记录）
      invoke("__noop_canvas_ready__", { t }).catch(() => {
        // 该 command 不存在是预期的；目的是让 IPC 链路打通后再触发后端 capture
      });
      // eslint-disable-next-line no-console
      console.log("[wallpaper-poc] canvas ready at", t);
    });
  });
}
