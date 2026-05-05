import { createApp } from "vue";
import "element-plus/dist/index.css";
import App from "./App.vue";
import "./styles/index.css";

declare global {
  interface Window {
    __LAZYCAT_VIEW__?: string;
  }
}

const params = new URLSearchParams(window.location.search);
const currentView = params.get("view") ?? window.__LAZYCAT_VIEW__;

if (currentView === "reminder-popup") {
  import("./ReminderPopupApp").then(({ default: mount }) => mount());
} else if (currentView === "quick-capture") {
  import("./QuickCaptureApp").then(({ default: mount }) => mount());
} else if (currentView === "spotlight") {
  import("./SpotlightApp").then(({ default: mount }) => mount());
} else if (currentView === "wallpaper-poc-canvas") {
  import("./WallpaperPocCanvasApp").then(({ default: mount }) => mount());
} else {
  createApp(App).mount("#app");
}
