<template>
  <div class="panel-grid">
    <el-input v-model="cryptoInput" type="textarea" :rows="8" placeholder="明文 / 密文(Base64)" />
    <el-input v-model="cryptoOutput" type="textarea" :rows="8" readonly placeholder="输出" />
    <el-input v-model="publicKeyPem" class="panel-grid-full" type="textarea" :rows="6" placeholder="RSA 公钥 PEM" />
    <el-input v-model="privateKeyPem" class="panel-grid-full" type="textarea" :rows="6" placeholder="RSA 私钥 PEM" />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="rsaEncrypt">RSA 加密</el-button>
        <el-button @click="rsaDecrypt">RSA 解密</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const cryptoInput = ref("");
const cryptoOutput = ref("");
const publicKeyPem = ref("");
const privateKeyPem = ref("");

async function rsaEncrypt() {
  try {
    cryptoOutput.value = String(
      await invokeToolByChannel("tool:crypto:rsa-encrypt", {
        plaintext: cryptoInput.value,
        publicKeyPem: publicKeyPem.value,
      }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function rsaDecrypt() {
  try {
    cryptoOutput.value = String(
      await invokeToolByChannel("tool:crypto:rsa-decrypt", {
        cipherTextBase64: cryptoInput.value,
        privateKeyPem: privateKeyPem.value,
      }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>
