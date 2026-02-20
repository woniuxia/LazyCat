<template>
  <div class="panel-grid">
    <el-input v-model="binValue" placeholder="二进制 (Binary)">
      <template #prepend>BIN</template>
    </el-input>
    <el-input v-model="octValue" placeholder="八进制 (Octal)">
      <template #prepend>OCT</template>
    </el-input>
    <el-input v-model="decValue" placeholder="十进制 (Decimal)">
      <template #prepend>DEC</template>
    </el-input>
    <el-input v-model="hexValue" placeholder="十六进制 (Hex)">
      <template #prepend>HEX</template>
    </el-input>
    <div class="panel-grid-full">
      <el-button @click="clearAll">清空</el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";

const binValue = ref("");
const octValue = ref("");
const decValue = ref("");
const hexValue = ref("");

let updating = false;

function convert(source: "bin" | "oct" | "dec" | "hex", value: string) {
  if (updating) return;
  updating = true;
  try {
    const trimmed = value.trim();
    if (!trimmed) {
      binValue.value = "";
      octValue.value = "";
      decValue.value = "";
      hexValue.value = "";
      return;
    }
    let num: bigint;
    try {
      switch (source) {
        case "bin": num = BigInt("0b" + trimmed); break;
        case "oct": num = BigInt("0o" + trimmed); break;
        case "dec": num = BigInt(trimmed); break;
        case "hex": num = BigInt("0x" + trimmed); break;
      }
    } catch {
      return;
    }
    if (source !== "bin") binValue.value = num.toString(2);
    if (source !== "oct") octValue.value = num.toString(8);
    if (source !== "dec") decValue.value = num.toString(10);
    if (source !== "hex") hexValue.value = num.toString(16).toUpperCase();
  } finally {
    updating = false;
  }
}

watch(binValue, (v) => convert("bin", v));
watch(octValue, (v) => convert("oct", v));
watch(decValue, (v) => convert("dec", v));
watch(hexValue, (v) => convert("hex", v));

function clearAll() {
  updating = true;
  binValue.value = "";
  octValue.value = "";
  decValue.value = "";
  hexValue.value = "";
  updating = false;
}
</script>
