<template>
  <div class="network-panel">
    <header class="network-header">
      <h1>访问链路诊断</h1>

      <div class="mode-switch" role="group" aria-label="网络工具模式">
        <button
          id="network-mode-diagnosis"
          type="button"
          :class="{ 'is-active': activeMode === 'diagnosis' }"
          :aria-pressed="activeMode === 'diagnosis'"
          aria-controls="network-diagnosis-panel"
          @click="activeMode = 'diagnosis'"
        >
          链路诊断
        </button>
        <button
          id="network-mode-quick"
          type="button"
          :class="{ 'is-active': activeMode === 'quick' }"
          :aria-pressed="activeMode === 'quick'"
          aria-controls="network-quick-panel"
          @click="activeMode = 'quick'"
        >
          单项探测
        </button>
      </div>
    </header>

    <section
      id="network-diagnosis-panel"
      v-show="activeMode === 'diagnosis'"
      class="mode-panel diagnosis-panel"
      role="region"
      aria-labelledby="network-mode-diagnosis"
    >
      <NetworkDiagnosisWorkspace />
    </section>

    <section
      id="network-quick-panel"
      v-show="activeMode === 'quick'"
      class="mode-panel quick-panel"
      role="region"
      aria-labelledby="network-mode-quick"
    >
      <NetworkQuickProbe />
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import NetworkDiagnosisWorkspace from "./network/NetworkDiagnosisWorkspace.vue";
import NetworkQuickProbe from "./network/NetworkQuickProbe.vue";

type NetworkMode = "diagnosis" | "quick";

const activeMode = ref<NetworkMode>("diagnosis");
</script>

<style scoped>
.network-panel {
  display: flex;
  width: 100%;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  gap: 12px;
}

.network-header {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 2px 0 10px;
  border-bottom: 1px solid var(--lc-border);
}

.network-header h1 {
  margin: 0;
  color: var(--lc-text);
  font-family: var(--lc-font-display);
  font-size: 20px;
  font-weight: 700;
  letter-spacing: 0;
}

.mode-switch {
  display: inline-flex;
  flex: none;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--lc-border);
  border-radius: 7px;
  background: var(--lc-surface-2);
}

.mode-switch button {
  min-width: 88px;
  min-height: 30px;
  padding: 0 12px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--lc-text-secondary);
  cursor: pointer;
  font: 600 12px var(--lc-font-body);
  letter-spacing: 0;
  transition:
    color var(--lc-duration) var(--lc-ease),
    background-color var(--lc-duration) var(--lc-ease),
    box-shadow var(--lc-duration) var(--lc-ease);
}

.mode-switch button:hover {
  color: var(--lc-text);
}

.mode-switch button.is-active {
  background: var(--lc-surface-0);
  color: var(--lc-accent);
  box-shadow: var(--lc-shadow-sm);
}

.mode-switch button:focus-visible {
  outline: 2px solid var(--lc-accent);
  outline-offset: 2px;
}

.mode-panel {
  width: 100%;
  min-width: 0;
  min-height: 0;
}

.diagnosis-panel {
  overflow: hidden;
  border: 1px solid var(--lc-border);
  border-radius: 8px;
  background: var(--lc-surface-0);
}

@media (max-width: 560px) {
  .network-header {
    align-items: stretch;
    flex-direction: column;
    gap: 10px;
  }

  .mode-switch {
    align-self: flex-start;
  }

  .mode-switch button {
    min-width: 82px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .mode-switch button {
    transition-duration: 0.01ms;
  }
}
</style>
