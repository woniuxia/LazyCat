import { createApp } from "vue";
import QuickCapture from "./components/QuickCapture.vue";

export default function mountQuickCaptureApp() {
  createApp(QuickCapture).mount("#app");
}
