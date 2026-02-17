<template>
  <div class="panel-grid">
    <el-input :model-value="name" placeholder="配置名称，例如 local-dev" @update:model-value="emit('update:name', String($event ?? ''))" />
    <el-input
      :model-value="content"
      type="textarea"
      :rows="8"
      placeholder="hosts 内容"
      @update:model-value="emit('update:content', String($event ?? ''))"
    />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="emit('save')">保存配置</el-button>
        <el-button @click="emit('activate')">设为当前配置</el-button>
        <el-button type="danger" @click="emit('delete')">删除配置</el-button>
        <el-button @click="emit('refresh')">刷新列表</el-button>
      </el-space>
    </div>
    <el-table class="panel-grid-full" :data="profiles" border>
      <el-table-column prop="name" label="名称" />
      <el-table-column prop="enabled" label="启用" width="80">
        <template #default="{ row }">{{ row.enabled ? "Yes" : "No" }}</template>
      </el-table-column>
      <el-table-column prop="updatedAt" label="更新时间" />
      <el-table-column label="操作" width="110">
        <template #default="{ row }">
          <el-button size="small" @click="emit('pick', row)">载入</el-button>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<script setup lang="ts">
interface HostsProfile {
  id: number;
  name: string;
  content: string;
  enabled: boolean;
  updatedAt: string;
}

defineProps<{
  name: string;
  content: string;
  profiles: HostsProfile[];
}>();

const emit = defineEmits<{
  (event: "update:name", value: string): void;
  (event: "update:content", value: string): void;
  (event: "save"): void;
  (event: "activate"): void;
  (event: "delete"): void;
  (event: "refresh"): void;
  (event: "pick", profile: HostsProfile): void;
}>();
</script>
