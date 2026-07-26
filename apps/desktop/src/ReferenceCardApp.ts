import { createApp } from "vue";
import ReferenceCard from "./components/ReferenceCard.vue";

export default function mountReferenceCardApp() {
  createApp(ReferenceCard).mount("#app");
}
