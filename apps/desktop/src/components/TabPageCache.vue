<template>
  <TabPageHost v-for="tab in tabs" :key="tab.id" :tab="tab" :active="tab.id === activeId">
    <template #default="slotProps">
      <slot v-bind="slotProps" />
    </template>
  </TabPageHost>
</template>

<script setup lang="ts">
import type { TabItem } from "../types/tabs";
import TabPageHost from "./TabPageHost.vue";

defineProps<{
  tabs: readonly TabItem[];
  activeId: string;
}>();

defineSlots<{
  default(props: { tab: TabItem }): unknown;
}>();
</script>
