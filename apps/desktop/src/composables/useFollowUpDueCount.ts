import { ref } from "vue";

const dueCount = ref(0);

export function useFollowUpDueCount() {
  return { dueCount };
}