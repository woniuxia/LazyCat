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

<script lang="ts">
const aesState = { input: "", output: "", key: "", iv: "", algorithm: "aes-256-cbc" };
</script>

<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const cryptoInput = ref(aesState.input);
const cryptoOutput = ref(aesState.output);
const symmetricKey = ref(aesState.key);
const symmetricIv = ref(aesState.iv);
const symmetricAlgorithm = ref(aesState.algorithm);

onBeforeUnmount(() => {
  aesState.input = cryptoInput.value;
  aesState.output = cryptoOutput.value;
  aesState.key = symmetricKey.value;
  aesState.iv = symmetricIv.value;
  aesState.algorithm = symmetricAlgorithm.value;
});

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
