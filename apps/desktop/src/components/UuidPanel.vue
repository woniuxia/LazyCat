<template>
  <div class="panel-grid">
    <el-input-number v-model="passwordLength" :min="4" :max="128" />
    <el-switch v-model="passwordSymbols" active-text="含符号" />
    <el-switch v-model="passwordNumbers" active-text="含数字" />
    <el-switch v-model="passwordUppercase" active-text="含大写" />
    <el-switch v-model="passwordLowercase" active-text="含小写" />
    <el-input class="panel-grid-full" v-model="idOutput" type="textarea" :rows="8" readonly />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="generateUuid">生成 UUID</el-button>
        <el-button @click="generateGuid">生成 GUID</el-button>
        <el-button @click="generatePassword">生成密码</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const passwordLength = ref(20);
const passwordSymbols = ref(true);
const passwordNumbers = ref(true);
const passwordUppercase = ref(true);
const passwordLowercase = ref(true);
const idOutput = ref("");

async function generateUuid() {
  try {
    idOutput.value = String(await invokeToolByChannel("tool:gen:uuid", {}));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function generateGuid() {
  try {
    idOutput.value = String(await invokeToolByChannel("tool:gen:guid", {}));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function generatePassword() {
  try {
    idOutput.value = String(
      await invokeToolByChannel("tool:gen:password", {
        length: passwordLength.value,
        symbols: passwordSymbols.value,
        numbers: passwordNumbers.value,
        uppercase: passwordUppercase.value,
        lowercase: passwordLowercase.value,
      }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>
