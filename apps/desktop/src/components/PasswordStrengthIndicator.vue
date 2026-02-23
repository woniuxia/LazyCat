<template>
  <div
    v-if="password && strengthResult"
    class="pw-strength-wrap"
    :class="{ 'is-weak': strengthResult.level === 'weak' || strengthResult.level === 'medium' }"
  >
    <div class="pw-strength-inline">
      <span
        v-for="i in 4"
        :key="i"
        class="pw-strength-inline__bar"
        :class="{
          'is-active': barActive(i),
          'is-weak': strengthResult.level === 'weak',
          'is-medium': strengthResult.level === 'medium',
          'is-strong': strengthResult.level === 'strong',
          'is-vstrong': strengthResult.level === 'very_strong',
        }"
      />
    </div>
    <Transition name="fade">
      <div v-if="showError && failedDetails.length" class="pw-strength-error">
        {{ failedDetails.join(' · ') }}
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";

interface StrengthDetail {
  rule: string;
  passed: boolean;
  message: string;
}

interface StrengthResult {
  score: number;
  level: string;
  details: StrengthDetail[];
}

const props = defineProps<{
  password: string;
  immediate?: boolean;
}>();

const strengthResult = ref<StrengthResult | null>(null);
const showError = ref(false);
let timer: ReturnType<typeof setTimeout> | null = null;
let isFirstAnalysis = ref(true);

const failedDetails = computed(() => {
  if (!strengthResult.value) return [];
  return strengthResult.value.details
    .filter((d) => !d.passed)
    .map((d) => d.message);
});

function barActive(index: number): boolean {
  if (!strengthResult.value) return false;
  const level = strengthResult.value.level;
  const levels = ["weak", "medium", "strong", "very_strong"];
  const currentIndex = levels.indexOf(level);
  return index <= currentIndex + 1;
}

async function analyzeStrength(pw: string) {
  try {
    const data = (await invokeToolByChannel("tool:gen:password-strength", {
      password: pw,
    })) as StrengthResult;
    strengthResult.value = data;
    showError.value = data.level === "weak" || data.level === "medium";
  } catch {
    strengthResult.value = null;
    showError.value = false;
  }
}

watch(
  () => props.password,
  (val) => {
    if (timer) clearTimeout(timer);
    if (!val) {
      strengthResult.value = null;
      showError.value = false;
      isFirstAnalysis.value = true;
      return;
    }
    if (isFirstAnalysis.value || props.immediate) {
      isFirstAnalysis.value = false;
      analyzeStrength(val);
    } else {
      timer = setTimeout(() => analyzeStrength(val), 300);
    }
  },
  { immediate: true }
);
</script>

<style scoped>
.pw-strength-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 6px;
  min-height: 20px;
}

.pw-strength-inline {
  display: flex;
  align-items: center;
  gap: 2px;
}

.pw-strength-inline__bar {
  width: 14px;
  height: 3px;
  border-radius: 2px;
  background: var(--lc-surface-3);
  transition: background-color var(--lc-duration) var(--lc-ease);
}

.pw-strength-inline__bar.is-active.is-weak {
  background: var(--lc-danger);
}

.pw-strength-inline__bar.is-active.is-medium {
  background: var(--lc-warning);
}

.pw-strength-inline__bar.is-active.is-strong {
  background: var(--lc-info);
}

.pw-strength-inline__bar.is-active.is-vstrong {
  background: var(--lc-success);
}

/* Error hint */
.pw-strength-error {
  font-size: 12px;
  line-height: 1.4;
  color: var(--lc-danger);
}

/* Fade transition for error */
.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--lc-duration) var(--lc-ease);
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
