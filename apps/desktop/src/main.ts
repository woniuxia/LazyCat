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

if (currentView === "global-notification") {
  import("./GlobalNotificationApp").then(({ default: mount }) => mount());
} else if (currentView === "pomodoro-prompt") {
  import("./PomodoroPromptApp").then(({ default: mount }) => mount());
} else if (currentView === "quick-capture") {
  import("./QuickCaptureApp").then(({ default: mount }) => mount());
} else if (currentView === "reference-card") {
  import("./ReferenceCardApp").then(({ default: mount }) => mount());
} else if (currentView === "widget-canvas") {
  import("./WidgetCanvasApp").then(({ default: mount }) => mount());
} else {
  createApp(App).mount("#app");
}
