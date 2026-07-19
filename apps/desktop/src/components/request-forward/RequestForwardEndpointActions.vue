<script setup lang="ts">
import type { RequestForwardProtocol } from "../../types/request-forward";

defineProps<{
  protocol: RequestForwardProtocol;
}>();

const emit = defineEmits<{
  "copy-listen": [];
  "copy-target": [];
  "open-local": [];
  "copy-command": [command: "powershell" | "curl"];
}>();

function handleCommand(command: string | number | object) {
  if (command === "powershell" || command === "curl") {
    emit("copy-command", command);
  }
}
</script>

<template>
  <div class="endpoint-actions" aria-label="监听端点快捷操作">
    <el-button size="small" @click="emit('copy-listen')">
      复制监听地址
    </el-button>
    <el-button size="small" @click="emit('copy-target')">
      复制目标地址
    </el-button>
    <template v-if="protocol === 'http'">
      <el-button size="small" @click="emit('open-local')">
        浏览器打开
      </el-button>
      <el-dropdown trigger="click" @command="handleCommand">
        <el-button size="small">命令示例</el-button>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item command="powershell">
              复制 PowerShell 命令
            </el-dropdown-item>
            <el-dropdown-item command="curl">
              复制 curl 命令
            </el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </template>
  </div>
</template>

<style scoped>
.endpoint-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.endpoint-actions :deep(.el-button + .el-button) {
  margin-left: 0;
}
</style>
