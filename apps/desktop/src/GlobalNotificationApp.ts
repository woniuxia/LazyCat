import { createApp } from "vue";
import GlobalNotificationPopup from "./components/GlobalNotificationPopup.vue";

export default function mountGlobalNotificationApp() {
  createApp(GlobalNotificationPopup).mount("#app");
}
