<template>
  <div class="mybatis-panel">
    <!-- 上半区：SQL 模板 + 参数表格 -->
    <div class="top-grid">
      <!-- 左：SQL 模板 -->
      <div class="section">
        <div class="section-header">
          <span class="section-label">SQL 模板</span>
        </div>
        <el-input
          v-model="sqlTemplate"
          type="textarea"
          :rows="14"
          placeholder="输入 MyBatis SQL/XML 模板"
        />
      </div>

      <!-- 右：参数表格 -->
      <div class="section">
        <div class="section-header">
          <span class="section-label">参数 {{ paramRows.length }} 个</span>
          <div class="section-actions">
            <el-button size="small" @click="extractParams">从模板提取</el-button>
            <el-button size="small" type="primary" @click="addRow">+ 添加</el-button>
          </div>
        </div>
        <div class="param-table-wrapper">
          <table class="param-table">
            <thead>
              <tr>
                <th class="col-name">参数名</th>
                <th class="col-type">类型</th>
                <th class="col-value">值</th>
                <th class="col-del"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in paramRows" :key="row.id">
                <td>
                  <el-input v-model="row.name" size="small" placeholder="参数名" />
                </td>
                <td>
                  <el-select v-model="row.type" size="small" @change="onTypeChange(row)">
                    <el-option label="字符串" value="string" />
                    <el-option label="数字" value="number" />
                    <el-option label="布尔" value="boolean" />
                    <el-option label="数组" value="array" />
                    <el-option label="null" value="null" />
                  </el-select>
                </td>
                <td class="val-cell">
                  <el-switch
                    v-if="row.type === 'boolean'"
                    :model-value="row.value === 'true'"
                    size="small"
                    @update:model-value="(v) => (row.value = String(v))"
                  />
                  <el-input
                    v-else
                    v-model="row.value"
                    size="small"
                    :disabled="row.type === 'null'"
                    placeholder="值"
                  />
                </td>
                <td class="del-cell">
                  <el-button size="small" type="danger" link @click="removeRow(row.id)">
                    ×
                  </el-button>
                </td>
              </tr>
              <tr v-if="paramRows.length === 0">
                <td colspan="4" class="empty-hint">暂无参数，点击"从模板提取"或"+ 添加"</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <!-- 操作按钮 -->
    <div class="actions">
      <el-button type="primary" @click="renderSql">渲染 SQL</el-button>
      <el-button @click="lintTemplate">语法检查</el-button>
    </div>

    <!-- 渲染结果 -->
    <el-input
      v-model="renderedSql"
      type="textarea"
      :rows="6"
      readonly
      placeholder="渲染结果 SQL"
    />

    <!-- 参数绑定表格 -->
    <el-table v-if="bindings.length" :data="bindings" border max-height="220">
      <el-table-column prop="name" label="参数名" min-width="160" />
      <el-table-column prop="mode" label="模式" width="100" />
      <el-table-column prop="value" label="值" min-width="260" />
    </el-table>

    <!-- 问题列表 -->
    <el-table v-if="issues.length" :data="issues" border max-height="200">
      <el-table-column prop="level" label="级别" width="100" />
      <el-table-column prop="message" label="信息" min-width="420" />
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onUnmounted } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

interface ParamRow {
  id: number;
  name: string;
  type: "string" | "number" | "boolean" | "array" | "null";
  value: string;
}

let nextId = 3;

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

const paramRows = ref<ParamRow[]>([
  { id: 1, name: "name", type: "string", value: "lazycat" },
  { id: 2, name: "ids", type: "array", value: "[1, 2, 3]" },
]);

const renderedSql = ref("");
const bindings = ref<Array<{ name: string; mode: string; value: unknown }>>([]);
const issues = ref<Array<{ level: string; message: string }>>([]);

function inferType(name: string): ParamRow["type"] {
  if (/ids$/i.test(name) || /[Ll]ist$/.test(name)) return "array";
  if (/Id$/.test(name) && !/ids$/i.test(name)) return "number";
  if (/^is[A-Z]/.test(name) || /^has[A-Z]/.test(name) || /^enable/.test(name)) return "boolean";
  return "string";
}

function defaultValue(type: ParamRow["type"], name: string): string {
  switch (type) {
    case "string":
      return name;
    case "number":
      return "1";
    case "boolean":
      return "true";
    case "array":
      return "[1, 2, 3]";
    default:
      return "";
  }
}

function addRow() {
  paramRows.value.push({ id: nextId++, name: "", type: "string", value: "" });
}

function removeRow(id: number) {
  paramRows.value = paramRows.value.filter((r) => r.id !== id);
}

function onTypeChange(row: ParamRow) {
  row.value = defaultValue(row.type, row.name);
}

function buildParamsJson(): string {
  const obj: Record<string, unknown> = {};
  for (const row of paramRows.value) {
    const name = row.name.trim();
    if (!name) continue;
    switch (row.type) {
      case "string":
        obj[name] = row.value;
        break;
      case "number":
        obj[name] = parseFloat(row.value) || 0;
        break;
      case "boolean":
        obj[name] = row.value === "true";
        break;
      case "array": {
        try {
          obj[name] = JSON.parse(row.value);
        } catch {
          obj[name] = row.value
            .split(",")
            .map((s) => s.trim())
            .filter(Boolean);
        }
        break;
      }
      case "null":
        obj[name] = null;
        break;
    }
  }
  return JSON.stringify(obj);
}

async function extractParams() {
  const template = sqlTemplate.value;
  if (!template.trim()) return;
  try {
    const data = (await invokeToolByChannel("tool:mybatis:extract-params", {
      sqlTemplate: template,
    })) as { params?: string[] };
    const extracted = Array.isArray(data?.params) ? data.params : [];
    const existingNames = new Set(paramRows.value.map((r) => r.name));
    for (const name of extracted) {
      if (!existingNames.has(name)) {
        const type = inferType(name);
        paramRows.value.push({
          id: nextId++,
          name,
          type,
          value: defaultValue(type, name),
        });
        existingNames.add(name);
      }
    }
  } catch (error) {
    console.warn("extract params failed:", error);
  }
}

let extractTimer: ReturnType<typeof setTimeout> | null = null;

watch(sqlTemplate, () => {
  if (extractTimer) clearTimeout(extractTimer);
  extractTimer = setTimeout(() => {
    extractParams();
  }, 800);
});

onUnmounted(() => {
  if (extractTimer) clearTimeout(extractTimer);
});

async function renderSql() {
  try {
    const data = (await invokeToolByChannel("tool:mybatis:render", {
      sqlTemplate: sqlTemplate.value,
      params: buildParamsJson(),
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

<style scoped>
.mybatis-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.top-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.section-label {
  font-size: 13px;
  font-weight: 500;
  color: #606266;
}

.section-actions {
  display: flex;
  gap: 6px;
}

.param-table-wrapper {
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  overflow: auto;
  max-height: 340px;
}

.param-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.param-table th {
  background: #f5f7fa;
  padding: 8px 6px;
  text-align: left;
  font-weight: 500;
  color: #606266;
  border-bottom: 1px solid #ebeef5;
  white-space: nowrap;
}

.param-table td {
  padding: 4px 6px;
  border-bottom: 1px solid #f0f2f5;
  vertical-align: middle;
}

.param-table tbody tr:last-child td {
  border-bottom: none;
}

.col-name {
  width: 34%;
}
.col-type {
  width: 26%;
}
.col-value {
  width: 32%;
}
.col-del {
  width: 8%;
}

.val-cell {
  vertical-align: middle;
}

.del-cell {
  text-align: center;
}

.empty-hint {
  text-align: center;
  color: #c0c4cc;
  padding: 20px 0;
  font-size: 13px;
}

.actions {
  display: flex;
  gap: 8px;
}

/* 去掉参数表格内 el-input/el-select 的多余阴影，融入表格风格 */
:deep(.param-table .el-input__wrapper) {
  box-shadow: none;
  background: transparent;
}

:deep(.param-table .el-input__wrapper:hover),
:deep(.param-table .el-input__wrapper.is-focus) {
  box-shadow: 0 0 0 1px var(--el-input-focus-border-color, #409eff) inset;
  background: #fff;
}

:deep(.param-table .el-select .el-input__wrapper) {
  box-shadow: none;
  background: transparent;
}

:deep(.param-table .el-select .el-input__wrapper:hover),
:deep(.param-table .el-select .el-input__wrapper.is-focus) {
  box-shadow: 0 0 0 1px var(--el-input-focus-border-color, #409eff) inset;
  background: #fff;
}
</style>
