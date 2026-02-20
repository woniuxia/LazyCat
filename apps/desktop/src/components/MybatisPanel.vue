<template>
  <div class="panel-grid">
    <el-input
      v-model="sqlTemplate"
      type="textarea"
      :rows="12"
      placeholder="输入 MyBatis SQL/XML 模板"
    />
    <el-input
      v-model="paramsJson"
      type="textarea"
      :rows="12"
      placeholder="输入参数 JSON"
    />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="renderSql">渲染 SQL</el-button>
        <el-button @click="lintTemplate">语法检查</el-button>
      </el-space>
    </div>
    <el-input
      v-model="renderedSql"
      class="panel-grid-full"
      type="textarea"
      :rows="8"
      readonly
      placeholder="渲染结果 SQL"
    />
    <el-table
      v-if="bindings.length"
      class="panel-grid-full"
      :data="bindings"
      border
      max-height="240"
    >
      <el-table-column prop="name" label="参数名" min-width="160" />
      <el-table-column prop="mode" label="模式" width="100" />
      <el-table-column prop="value" label="值" min-width="260" />
    </el-table>
    <el-table
      v-if="issues.length"
      class="panel-grid-full"
      :data="issues"
      border
      max-height="220"
    >
      <el-table-column prop="level" label="级别" width="100" />
      <el-table-column prop="message" label="信息" min-width="420" />
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const sqlTemplate = ref(`<select>
  SELECT id, name
  FROM user
  <where>
    <if test="name != null and name != ''">
      AND name = #{name}
    </if>
    <if test="ids != null">
      AND id IN
      <foreach collection="ids" item="id" open="(" separator="," close=")">
        #{id}
      </foreach>
    </if>
  </where>
</select>`);
const paramsJson = ref(`{"name":"lazycat","ids":[1,2,3]}`);
const renderedSql = ref("");
const bindings = ref<Array<{ name: string; mode: string; value: unknown }>>([]);
const issues = ref<Array<{ level: string; message: string }>>([]);

async function renderSql() {
  try {
    const data = (await invokeToolByChannel("tool:mybatis:render", {
      sqlTemplate: sqlTemplate.value,
      params: paramsJson.value,
      safeSubstitution: true,
    })) as {
      sql?: string;
      bindings?: Array<{ name: string; mode: string; value: unknown }>;
      warnings?: string[];
    };
    renderedSql.value = data?.sql ?? "";
    bindings.value = Array.isArray(data?.bindings) ? data.bindings : [];
    const warnings = Array.isArray(data?.warnings) ? data.warnings : [];
    issues.value = warnings.map((message) => ({ level: "warn", message }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function lintTemplate() {
  try {
    const data = (await invokeToolByChannel("tool:mybatis:lint", {
      sqlTemplate: sqlTemplate.value,
    })) as { issues?: Array<{ level: string; message: string }> };
    issues.value = Array.isArray(data?.issues) ? data.issues : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>
