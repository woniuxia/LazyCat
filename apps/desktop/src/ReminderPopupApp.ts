import { createApp } from "vue";
import ReminderPopup from "./components/ReminderPopup.vue";

export default function mountReminderPopupApp() {
  createApp(ReminderPopup).mount("#app");
}
