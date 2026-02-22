<template>
  <div class="sql-entity-panel">
    <div class="sql-entity-toolbar">
      <el-select v-model="language" style="width: 140px">
        <el-option v-for="l in languages" :key="l.value" :label="l.label" :value="l.value" />
      </el-select>
      <el-select v-model="naming" style="width: 140px">
        <el-option v-for="n in namingStyles" :key="n.value" :label="n.label" :value="n.value" />
      </el-select>
      <el-checkbox v-model="comments">注释</el-checkbox>
      <el-button type="primary" :loading="generating" @click="generate">生成</el-button>
      <el-button @click="copyOutput">复制</el-button>
    </div>
    <div class="sql-entity-editors">
      <div class="editor-col">
        <div class="editor-label">SQL 输入 (CREATE TABLE)</div>
        <el-input
          v-model="sqlInput"
          type="textarea"
          :rows="18"
          resize="vertical"
          placeholder="粘贴 CREATE TABLE 语句，支持多表"
        />
      </div>
      <div class="editor-col">
        <div class="editor-label">
          {{ languageLabel }} 输出
          <el-tag v-if="tableCount > 0" size="small" type="info">{{ tableCount }} 张表</el-tag>
        </div>
        <el-input
          v-model="codeOutput"
          type="textarea"
          :rows="18"
          resize="vertical"
          readonly
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const languages = [
  { label: "Java", value: "java" },
  { label: "TypeScript", value: "typescript" },
  { label: "Go", value: "go" },
  { label: "Python", value: "python" },
  { label: "Kotlin", value: "kotlin" },
  { label: "C#", value: "csharp" },
];

const namingStyles = [
  { label: "camelCase", value: "camelCase" },
  { label: "snake_case", value: "snake_case" },
  { label: "原始", value: "original" },
];

const language = ref("java");
const naming = ref("camelCase");
const comments = ref(true);
const sqlInput = ref(`CREATE TABLE t_user (
  id BIGINT NOT NULL AUTO_INCREMENT COMMENT '主键',
  user_name VARCHAR(100) NOT NULL COMMENT '用户名',
  email VARCHAR(200) COMMENT '邮箱地址',
  age INT DEFAULT 0 COMMENT '年龄',
  balance DECIMAL(10,2) COMMENT '余额',
  active TINYINT(1) NOT NULL DEFAULT 1 COMMENT '是否启用',
  created_at DATETIME NOT NULL COMMENT '创建时间',
  PRIMARY KEY (id)
);`);
const codeOutput = ref("");
const tableCount = ref(0);
const generating = ref(false);

const languageLabel = computed(
  () => languages.find((l) => l.value === language.value)?.label ?? ""
);

async function generate() {
  if (!sqlInput.value.trim()) {
    ElMessage.warning("请输入 SQL 建表语句");
    return;
  }
  generating.value = true;
  try {
    const data = (await invokeToolByChannel("tool:convert:sql-to-entity", {
      sql: sqlInput.value,
      language: language.value,
      options: {
        comments: comments.value,
        naming: naming.value,
      },
    })) as { code: string; tables: unknown[] };
    codeOutput.value = data.code;
    tableCount.value = data.tables?.length ?? 0;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    generating.value = false;
  }
}

async function copyOutput() {
  if (!codeOutput.value) {
    ElMessage.warning("没有可复制的结果");
    return;
  }
  try {
    await navigator.clipboard.writeText(codeOutput.value);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}
</script>

<style scoped>
.sql-entity-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.sql-entity-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}
.sql-entity-editors {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
.editor-col {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.editor-label {
  font-weight: 600;
  font-size: 13px;
  display: flex;
  align-items: center;
  gap: 8px;
}
@media (max-width: 900px) {
  .sql-entity-editors {
    grid-template-columns: 1fr;
  }
}
</style>
