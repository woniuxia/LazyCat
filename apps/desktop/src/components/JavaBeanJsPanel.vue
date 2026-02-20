<template>
  <div class="panel-grid">
    <el-input
      v-model="beanInput"
      type="textarea"
      :rows="12"
      placeholder="输入 Java Bean 源码"
    />
    <el-input
      v-model="jsonOutput"
      type="textarea"
      :rows="12"
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
      class="panel-grid-full"
      type="textarea"
      :rows="10"
      readonly
      placeholder="JS Object 输出"
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const beanInput = ref(`public class UserDTO {
  private Long id;
  @JsonProperty("user_name")
  private String userName;
  private Boolean enabled;
}`);
const jsonOutput = ref("");
const jsObjectOutput = ref("");

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
</script>
