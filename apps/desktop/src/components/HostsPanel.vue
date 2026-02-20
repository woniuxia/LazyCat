<template>
  <div class="panel-grid">
    <el-input v-model="hostsName" placeholder="配置名称，例如 local-dev" />
    <el-input
      v-model="hostsContent"
      type="textarea"
      :rows="8"
      placeholder="hosts 内容"
    />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="saveHosts">保存配置</el-button>
        <el-button @click="activateHosts">设为当前配置</el-button>
        <el-button type="danger" @click="deleteHosts">删除配置</el-button>
        <el-button @click="loadHostsProfiles">刷新列表</el-button>
      </el-space>
    </div>
    <el-table class="panel-grid-full" :data="hostsProfiles" border>
      <el-table-column prop="name" label="名称" />
      <el-table-column prop="enabled" label="启用" width="80">
        <template #default="{ row }">{{ row.enabled ? "Yes" : "No" }}</template>
      </el-table-column>
      <el-table-column prop="updatedAt" label="更新时间" />
      <el-table-column label="操作" width="110">
        <template #default="{ row }">
          <el-button size="small" @click="pickHosts(row)">载入</el-button>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { HostsProfile } from "../types";

const hostsName = ref("");
const hostsContent = ref("");
const hostsProfiles = ref<HostsProfile[]>([]);

async function loadHostsProfiles() {
  try {
    const data = await invokeToolByChannel("tool:hosts:list", {});
    hostsProfiles.value = Array.isArray(data) ? (data as HostsProfile[]) : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function pickHosts(profile: HostsProfile) {
  hostsName.value = profile.name;
  hostsContent.value = profile.content;
}

async function saveHosts() {
  try {
    await invokeToolByChannel("tool:hosts:save", {
      name: hostsName.value,
      content: hostsContent.value,
    });
    await loadHostsProfiles();
    ElMessage.success("hosts 配置已保存");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function activateHosts() {
  try {
    const data = await invokeToolByChannel("tool:hosts:activate", {
      profileName: hostsName.value,
      content: hostsContent.value,
    });
    await loadHostsProfiles();
    ElMessage.success(`切换成功: ${JSON.stringify(data)}`);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function deleteHosts() {
  try {
    await invokeToolByChannel("tool:hosts:delete", { name: hostsName.value });
    await loadHostsProfiles();
    ElMessage.success("hosts 配置已删除");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

onMounted(() => loadHostsProfiles());
</script>
