<template>
  <div class="panel-grid">
    <el-input :model-value="pattern" placeholder="正则表达式" @update:model-value="emit('update:pattern', String($event ?? ''))" />
    <el-input :model-value="flags" placeholder="flags，例如 gi" @update:model-value="emit('update:flags', String($event ?? ''))" />
    <el-input
      class="panel-grid-full"
      :model-value="input"
      type="textarea"
      :rows="8"
      placeholder="待匹配文本"
      @update:model-value="emit('update:input', String($event ?? ''))"
    />
    <el-input class="panel-grid-full" :model-value="output" type="textarea" :rows="8" readonly placeholder="匹配结果" />
    <el-select :model-value="kind" placeholder="常用模板" @update:model-value="emit('update:kind', $event)">
      <el-option label="邮箱" value="email" />
      <el-option label="IPv4" value="ipv4" />
      <el-option label="URL" value="url" />
      <el-option label="中国手机号" value="phone-cn" />
    </el-select>
    <div>
      <el-space>
        <el-button type="primary" @click="emit('run')">执行匹配</el-button>
        <el-button @click="emit('applyTemplate')">填充模板</el-button>
        <el-button @click="emit('loadTemplates')">查看模板库</el-button>
      </el-space>
    </div>
    <el-input
      class="panel-grid-full"
      :model-value="templatesDisplay"
      type="textarea"
      :rows="6"
      readonly
      placeholder="模板库"
    />
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

type RegexKind = "email" | "ipv4" | "url" | "phone-cn";

const props = defineProps<{
  pattern: string;
  flags: string;
  input: string;
  output: string;
  kind: RegexKind;
  templates: unknown[];
}>();

const emit = defineEmits<{
  (event: "update:pattern", value: string): void;
  (event: "update:flags", value: string): void;
  (event: "update:input", value: string): void;
  (event: "update:kind", value: RegexKind): void;
  (event: "run"): void;
  (event: "applyTemplate"): void;
  (event: "loadTemplates"): void;
}>();

const templatesDisplay = computed(() => JSON.stringify(props.templates, null, 2));
</script>
