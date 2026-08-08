<template>
  <!-- Stable wrapper names let KeepAlive remove exactly the tabs no longer in the list. -->
  <KeepAlive :include="cacheNames">
    <component v-if="activeTab && activeComponent" :is="activeComponent" :key="activeTab.id" />
  </KeepAlive>
</template>

<script setup lang="ts">
import { computed, defineComponent, type Component } from "vue";
import type { TabItem } from "../types/tabs";

const props = defineProps<{
  tabs: readonly TabItem[];
  activeId: string;
}>();

const slots = defineSlots<{
  default(props: { tab: TabItem }): unknown;
}>();

const wrapperNames = new Map<string, string>();
const wrapperComponents = new Map<string, Component>();

function getWrapperName(id: string): string {
  let name = wrapperNames.get(id);
  if (!name) {
    const encodedId = Array.from(id)
      .map((character) => character.codePointAt(0)!.toString(16))
      .join("_");
    name = `LazyCatTabPage_${encodedId || "empty"}`;
    wrapperNames.set(id, name);
  }
  return name;
}

function getWrapperComponent(tab: TabItem): Component {
  let component = wrapperComponents.get(tab.id);
  if (!component) {
    component = defineComponent({
      name: getWrapperName(tab.id),
      setup() {
        return () => slots.default?.({ tab }) ?? null;
      },
    });
    wrapperComponents.set(tab.id, component);
  }
  return component;
}

const activeTab = computed(() => props.tabs.find((tab) => tab.id === props.activeId) ?? null);
const activeComponent = computed(() =>
  activeTab.value ? getWrapperComponent(activeTab.value) : null,
);
const cacheNames = computed(() => props.tabs.map((tab) => getWrapperName(tab.id)));
</script>
