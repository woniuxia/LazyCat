<template>
  <div v-show="active" class="tab-page-scroll" :data-tab-scroll-id="tab.id">
    <KeepAlive>
      <PageContent v-if="active" />
    </KeepAlive>
  </div>
</template>

<script setup lang="ts">
import { defineComponent } from "vue";
import type { TabItem } from "../types/tabs";

const props = defineProps<{
  tab: TabItem;
  active: boolean;
}>();

const slots = defineSlots<{
  default(props: { tab: TabItem }): unknown;
}>();

const PageContent = defineComponent({
  name: "LazyCatTabPageContent",
  setup() {
    return () => slots.default?.({ tab: props.tab }) ?? null;
  },
});
</script>
