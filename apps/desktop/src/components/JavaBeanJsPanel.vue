<template>
  <div class="panel-grid java-bean-panel">
    <el-input
      class="java-bean-editor"
      v-model="beanInput"
      type="textarea"
      resize="none"
      placeholder="输入 Java Bean 源码"
    />
    <el-input
      class="java-bean-editor"
      v-model="jsonOutput"
      type="textarea"
      resize="none"
      readonly
      placeholder="JSON 输出"
    />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="beanToJson">Bean -> JSON</el-button>
        <el-button @click="jsonToJsObject">JSON -> JS Object</el-button>
        <el-button @click="beanToJsObject">一键生成</el-button>
      </el-space>
    </div>
    <el-input
      v-model="jsObjectOutput"
      class="panel-grid-full java-bean-output"
      type="textarea"
      resize="none"
      readonly
      placeholder="JS Object 输出"
    />
  </div>
</template>

<script lang="ts">
const javaBeanState = {
  beanInput: `public class UserDTO {
  private Long id;
  @JsonProperty("user_name")
  private String userName;
  private Boolean enabled;
}`,
  jsonOutput: "",
  jsObjectOutput: "",
};
</script>

<style scoped>
.java-bean-panel {
  flex: 1;
  min-height: 0;
  grid-template-rows: minmax(240px, 1fr) auto minmax(200px, 1fr);
}

.java-bean-editor,
.java-bean-output {
  height: 100%;
  min-height: 0;
}

.java-bean-editor :deep(.el-textarea__inner),
.java-bean-output :deep(.el-textarea__inner) {
  height: 100% !important;
  min-height: 200px;
}

@media (max-width: 1000px) {
  .java-bean-panel {
    grid-template-rows: minmax(200px, 1fr) minmax(200px, 1fr) auto minmax(200px, 1fr);
    overflow: auto;
  }
}
</style>

<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const beanInput = ref(javaBeanState.beanInput);
const jsonOutput = ref(javaBeanState.jsonOutput);
const jsObjectOutput = ref(javaBeanState.jsObjectOutput);

async function beanToJson() {
  try {
    const data = (await invokeToolByChannel("tool:convert:java-bean-to-json", {
      bean: beanInput.value,
    })) as { json?: string; warnings?: string[] };
    jsonOutput.value = data?.json ?? "{}";
    if (Array.isArray(data?.warnings) && data.warnings.length > 0) {
      ElMessage.warning(data.warnings.join("; "));
    }
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function jsonToJsObject() {
  try {
    const data = (await invokeToolByChannel("tool:convert:json-to-js-object", {
      json: jsonOutput.value,
      quoteStyle: "single",
    })) as { jsObject?: string };
    jsObjectOutput.value = data?.jsObject ?? "";
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function beanToJsObject() {
  try {
    const data = (await invokeToolByChannel("tool:convert:java-bean-to-js-object", {
      bean: beanInput.value,
      quoteStyle: "single",
    })) as { json?: string; jsObject?: string; warnings?: string[] };
    jsonOutput.value = data?.json ?? "{}";
    jsObjectOutput.value = data?.jsObject ?? "";
    if (Array.isArray(data?.warnings) && data.warnings.length > 0) {
      ElMessage.warning(data.warnings.join("; "));
    }
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

onBeforeUnmount(() => {
  javaBeanState.beanInput = beanInput.value;
  javaBeanState.jsonOutput = jsonOutput.value;
  javaBeanState.jsObjectOutput = jsObjectOutput.value;
});
</script>
