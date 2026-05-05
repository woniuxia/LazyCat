import { createApp } from "vue";
import SpotlightPanel from "./components/SpotlightPanel.vue";

export default function mountSpotlightApp() {
  createApp(SpotlightPanel).mount("#app");
}
