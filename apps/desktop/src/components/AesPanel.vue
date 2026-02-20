<template>
  <div class="panel-grid">
    <el-input v-model="cryptoInput" type="textarea" :rows="6" placeholder="明文 / 密文(Base64)" />
    <el-input v-model="cryptoOutput" type="textarea" :rows="6" readonly placeholder="输出" />
    <el-select v-model="symmetricAlgorithm" placeholder="算法">
      <el-option label="AES-256-CBC" value="aes-256-cbc" />
      <el-option label="AES-192-CBC" value="aes-192-cbc" />
      <el-option label="AES-128-CBC" value="aes-128-cbc" />
      <el-option label="3DES-CBC" value="des-ede3-cbc" />
      <el-option label="DES-CBC" value="des-cbc" />
    </el-select>
    <el-input v-model="symmetricIv" placeholder="IV（文本）" />
    <el-input class="panel-grid-full" v-model="symmetricKey" placeholder="Key（文本）" />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="symmetricEncrypt">加密</el-button>
        <el-button @click="symmetricDecrypt">解密</el-button>
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
const symmetricKey = ref("");
const symmetricIv = ref("");
const symmetricAlgorithm = ref("aes-256-cbc");

async function symmetricEncrypt() {
  try {
    const channel = symmetricAlgorithm.value.startsWith("aes")
      ? "tool:crypto:aes-encrypt"
      : "tool:crypto:des-encrypt";
    cryptoOutput.value = String(
      await invokeToolByChannel(channel, {
        plaintext: cryptoInput.value,
        key: symmetricKey.value,
        iv: symmetricIv.value,
        algorithm: symmetricAlgorithm.value,
      }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function symmetricDecrypt() {
  try {
    const channel = symmetricAlgorithm.value.startsWith("aes")
      ? "tool:crypto:aes-decrypt"
      : "tool:crypto:des-decrypt";
    cryptoOutput.value = String(
      await invokeToolByChannel(channel, {
        cipherTextBase64: cryptoInput.value,
        key: symmetricKey.value,
        iv: symmetricIv.value,
        algorithm: symmetricAlgorithm.value,
      }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>
