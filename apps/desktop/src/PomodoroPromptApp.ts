import { createApp } from "vue";
import PomodoroPrompt from "./components/PomodoroPrompt.vue";

export default function mountPomodoroPromptApp() {
  createApp(PomodoroPrompt).mount("#app");
}
