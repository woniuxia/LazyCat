<template>
  <div class="md-rendered" v-html="html"></div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { renderMarkdown } from "../utils/renderMarkdown";

const props = defineProps<{ content: string }>();

const html = computed(() => (props.content ? renderMarkdown(props.content) : ""));
</script>
