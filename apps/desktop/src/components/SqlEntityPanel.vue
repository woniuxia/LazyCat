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
      <el-checkbox v-if="language === 'java'" v-model="mybatisPlus">
        MyBatis-Plus 注解
      </el-checkbox>
      <el-select
        v-if="language === 'java'"
        v-model="selectedBaseClassIds"
        multiple
        collapse-tags
        :max-collapse-tags="1"
        clearable
        placeholder="参与字段排除"
        style="width: 210px"
        @change="syncBaseClassSelection"
      >
        <el-option
          v-for="item in baseClasses"
          :key="item.id"
          :label="item.alias"
          :value="item.id"
        />
      </el-select>
      <el-select
        v-if="language === 'java'"
        v-model="parentBaseClassId"
        :disabled="selectedBaseClasses.length === 0"
        placeholder="实际父类"
        style="width: 170px"
      >
        <el-option
          v-for="item in selectedBaseClasses"
          :key="item.id"
          :label="item.alias"
          :value="item.id"
        />
      </el-select>
      <el-button v-if="language === 'java'" @click="baseClassDialog?.open()">基类管理</el-button>
      <el-button type="primary" :loading="generating" @click="generate">生成</el-button>
      <el-button @click="copyOutput">复制</el-button>
    </div>
    <div class="sql-entity-editors">
      <div class="editor-col">
        <div class="editor-label">SQL 输入 (CREATE TABLE)</div>
        <el-input
          v-model="sqlInput"
          type="textarea"
          resize="none"
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
          resize="none"
          readonly
        />
      </div>
    </div>
    <SqlEntityBaseClassDialog ref="baseClassDialog" @changed="handleBaseClassesChanged" />
  </div>
</template>

<script lang="ts">
const sqlEntityState = {
  language: "java",
  naming: "camelCase",
  comments: true,
  mybatisPlus: false,
  selectedBaseClassIds: [] as number[],
  parentBaseClassId: null as number | null,
  sqlInput: `CREATE TABLE t_user (
  id BIGINT NOT NULL AUTO_INCREMENT COMMENT '主键',
  user_name VARCHAR(100) NOT NULL COMMENT '用户名',
  email VARCHAR(200) COMMENT '邮箱地址',
  age INT DEFAULT 0 COMMENT '年龄',
  balance DECIMAL(10,2) COMMENT '余额',
  active TINYINT(1) NOT NULL DEFAULT 1 COMMENT '是否启用',
  created_at DATETIME NOT NULL COMMENT '创建时间',
  PRIMARY KEY (id)
);`,
  codeOutput: "",
};
</script>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  SqlEntityBaseClass,
  SqlEntityBaseClassListResponse,
} from "../types/sql-entity";
import { reconcileBaseClassSelection } from "../utils/sqlEntityBaseClass";
import SqlEntityBaseClassDialog from "./SqlEntityBaseClassDialog.vue";

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

const language = ref(sqlEntityState.language);
const naming = ref(sqlEntityState.naming);
const comments = ref(sqlEntityState.comments);
const mybatisPlus = ref(sqlEntityState.mybatisPlus);
const baseClasses = ref<SqlEntityBaseClass[]>([]);
const selectedBaseClassIds = ref<number[]>([...sqlEntityState.selectedBaseClassIds]);
const parentBaseClassId = ref<number | null>(sqlEntityState.parentBaseClassId);
const baseClassDialog = ref<InstanceType<typeof SqlEntityBaseClassDialog> | null>(null);
const sqlInput = ref(sqlEntityState.sqlInput);
const codeOutput = ref(sqlEntityState.codeOutput);
const tableCount = ref(0);
const generating = ref(false);

const languageLabel = computed(
  () => languages.find((l) => l.value === language.value)?.label ?? ""
);

const selectedBaseClasses = computed(() => {
  const selected = new Set(selectedBaseClassIds.value);
  return baseClasses.value.filter((item) => selected.has(item.id));
});

function syncBaseClassSelection() {
  const next = reconcileBaseClassSelection(
    selectedBaseClassIds.value,
    parentBaseClassId.value,
    baseClasses.value.map((item) => item.id),
  );
  selectedBaseClassIds.value = next.selectedIds;
  parentBaseClassId.value = next.parentId;
}

async function loadBaseClasses() {
  try {
    const result = (await invokeToolByChannel(
      "tool:sql-entity:base-class-list",
      {},
    )) as SqlEntityBaseClassListResponse;
    baseClasses.value = result.items;
    syncBaseClassSelection();
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function handleBaseClassesChanged(items: SqlEntityBaseClass[]) {
  baseClasses.value = items;
  syncBaseClassSelection();
}

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
        mybatisPlus: language.value === "java" && mybatisPlus.value,
        ...(language.value === "java"
          ? {
              baseClasses: selectedBaseClasses.value.map((item) => ({
                id: item.id,
                alias: item.alias,
                qualifiedName: item.qualifiedName,
                fields: item.fields,
              })),
              parentBaseClassId: parentBaseClassId.value,
            }
          : {}),
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

onMounted(loadBaseClasses);

onBeforeUnmount(() => {
  sqlEntityState.language = language.value;
  sqlEntityState.naming = naming.value;
  sqlEntityState.comments = comments.value;
  sqlEntityState.mybatisPlus = mybatisPlus.value;
  sqlEntityState.selectedBaseClassIds = [...selectedBaseClassIds.value];
  sqlEntityState.parentBaseClassId = parentBaseClassId.value;
  sqlEntityState.sqlInput = sqlInput.value;
  sqlEntityState.codeOutput = codeOutput.value;
});
</script>

<style scoped>
.sql-entity-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-height: 0;
}
.sql-entity-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  flex-shrink: 0;
}
.sql-entity-editors {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  flex: 1;
  min-height: 0;
}
.editor-col {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  min-height: 0;
}
.editor-label {
  font-weight: 600;
  font-size: 13px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.editor-col :deep(.el-textarea) {
  flex: 1;
  min-height: 0;
}
.editor-col :deep(.el-textarea__inner) {
  height: 100% !important;
  min-height: 240px;
}
@media (max-width: 900px) {
  .sql-entity-editors {
    grid-template-columns: 1fr;
    overflow: auto;
  }
}
</style>
