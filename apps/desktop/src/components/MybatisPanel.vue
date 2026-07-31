<template>
  <div class="mybatis-panel">
    <div class="tool-strip">
      <div class="example-picker">
        <span class="strip-label">常用示例</span>
        <el-select
          :model-value="activeExampleId"
          class="example-select"
          placeholder="选择示例"
          @change="applyExample"
        >
          <el-option
            v-for="example in examples"
            :key="example.id"
            :label="example.label"
            :value="example.id"
          >
            <div class="example-option">
              <span>{{ example.label }}</span>
              <span>{{ example.summary }}</span>
            </div>
          </el-option>
        </el-select>
        <span class="example-summary">{{ activeExample?.summary ?? "自定义模板" }}</span>
      </div>

      <div class="run-actions">
        <el-button :loading="lintLoading" @click="lintTemplate">
          <el-icon><CircleCheck /></el-icon>
          语法检查
        </el-button>
        <el-button type="primary" :loading="renderLoading" @click="renderSql">
          <el-icon><VideoPlay /></el-icon>
          渲染 SQL
        </el-button>
      </div>
    </div>

    <div class="workspace-grid">
      <section class="workspace-section">
        <div class="section-header">
          <div class="section-title-wrap">
            <span class="section-title">SQL 模板</span>
            <el-tag size="small" effect="plain">MyBatis XML</el-tag>
          </div>
        </div>
        <el-input
          v-model="sqlTemplate"
          class="template-input"
          type="textarea"
          resize="none"
          spellcheck="false"
          placeholder="输入 MyBatis SQL/XML 模板"
        />
      </section>

      <section class="workspace-section">
        <div class="section-header">
          <div class="section-title-wrap">
            <span class="section-title">参数</span>
            <span class="section-count">{{ paramRows.length }}</span>
          </div>
          <div class="section-actions">
            <el-button size="small" :loading="extractLoading" @click="extractParams(true)">
              <el-icon><Refresh /></el-icon>
              同步参数
            </el-button>
            <el-button size="small" type="primary" plain @click="addRow">
              <el-icon><Plus /></el-icon>
              添加
            </el-button>
          </div>
        </div>

        <div class="param-table-wrapper">
          <table class="param-table">
            <thead>
              <tr>
                <th class="col-name">参数名</th>
                <th class="col-type">类型</th>
                <th class="col-value">值</th>
                <th class="col-delete"><span class="visually-hidden">操作</span></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in paramRows" :key="row.id">
                <td><el-input v-model="row.name" size="small" placeholder="参数名" /></td>
                <td>
                  <el-select v-model="row.type" size="small" @change="onTypeChange(row)">
                    <el-option label="字符串" value="string" />
                    <el-option label="数字" value="number" />
                    <el-option label="布尔" value="boolean" />
                    <el-option label="数组" value="array" />
                    <el-option label="null" value="null" />
                  </el-select>
                </td>
                <td class="value-cell">
                  <el-switch
                    v-if="row.type === 'boolean'"
                    :model-value="row.value === 'true'"
                    size="small"
                    @update:model-value="(value) => (row.value = String(value))"
                  />
                  <el-input
                    v-else
                    v-model="row.value"
                    size="small"
                    :disabled="row.type === 'null'"
                    :placeholder="row.type === 'array' ? '[1, 2, 3]' : '值'"
                  />
                </td>
                <td class="delete-cell">
                  <el-tooltip content="删除参数" placement="top">
                    <el-button
                      :icon="Delete"
                      size="small"
                      type="danger"
                      text
                      :aria-label="`删除参数 ${row.name || '未命名'}`"
                      @click="removeRow(row.id)"
                    />
                  </el-tooltip>
                </td>
              </tr>
              <tr v-if="paramRows.length === 0">
                <td colspan="4" class="empty-hint">模板中暂无可提取参数</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </div>

    <section class="result-section">
      <el-tabs v-model="activeResultTab" class="result-tabs">
        <el-tab-pane name="sql">
          <template #label>渲染结果</template>
          <div class="result-pane">
            <div class="result-meta">
              <el-tag v-if="resultStale" size="small" type="warning" effect="plain">
                内容已修改
              </el-tag>
              <span v-else class="result-hint">
                {{ renderedSql ? "已生成可执行 SQL" : "尚未渲染" }}
              </span>
              <el-tooltip content="复制 SQL" placement="top">
                <el-button
                  :icon="CopyDocument"
                  size="small"
                  text
                  :disabled="!renderedSql"
                  aria-label="复制渲染 SQL"
                  @click="copyRenderedSql"
                />
              </el-tooltip>
            </div>
            <pre v-if="renderedSql" class="sql-output">{{ renderedSql }}</pre>
            <el-empty v-else :image-size="52" description="渲染结果将在这里显示" />
          </div>
        </el-tab-pane>

        <el-tab-pane name="bindings">
          <template #label>
            <span class="tab-label">
              参数绑定
              <span v-if="bindings.length" class="tab-count">{{ bindings.length }}</span>
            </span>
          </template>
          <div class="result-pane">
            <el-table v-if="bindings.length" :data="bindings" size="small" max-height="240">
              <el-table-column prop="name" label="参数名" min-width="160" />
              <el-table-column prop="mode" label="模式" width="90" />
              <el-table-column label="值" min-width="260">
                <template #default="scope">
                  <code class="binding-value">{{ formatValue(scope.row.value) }}</code>
                </template>
              </el-table-column>
            </el-table>
            <el-empty v-else :image-size="52" description="暂无参数绑定" />
          </div>
        </el-tab-pane>

        <el-tab-pane name="issues">
          <template #label>
            <span class="tab-label">
              检查问题
              <span v-if="issues.length" class="tab-count issue-count">{{ issues.length }}</span>
            </span>
          </template>
          <div class="result-pane issues-pane">
            <div v-if="issues.length" class="issue-list">
              <div
                v-for="(issue, index) in issues"
                :key="`${issue.level}-${index}`"
                class="issue-row"
              >
                <el-tag
                  size="small"
                  :type="issue.level === 'error' ? 'danger' : 'warning'"
                  effect="light"
                >
                  {{ issue.level === "error" ? "错误" : "警告" }}
                </el-tag>
                <span>{{ issue.message }}</span>
              </div>
            </div>
            <el-empty
              v-else
              :image-size="52"
              :description="lintPassed ? '未发现语法问题' : '尚未执行语法检查'"
            />
          </div>
        </el-tab-pane>
      </el-tabs>
    </section>
  </div>
</template>

<script lang="ts">
type ParamType = "string" | "number" | "boolean" | "array" | "null";

interface ParamRow {
  id: number;
  name: string;
  type: ParamType;
  value: string;
}

interface MybatisExample {
  id: string;
  label: string;
  summary: string;
  sqlTemplate: string;
  params: Array<Omit<ParamRow, "id">>;
}

const MYBATIS_EXAMPLES: MybatisExample[] = [
  {
    id: "dynamic-query",
    label: "动态条件查询",
    summary: "where + if：按非空参数拼接查询条件",
    sqlTemplate: `<select>
  SELECT id, name, status
  FROM users
  <where>
    <if test="name != null and name != ''">
      AND name LIKE #{name}
    </if>
    <if test="status != null">
      AND status = #{status}
    </if>
  </where>
  ORDER BY id DESC
</select>`,
    params: [
      { name: "name", type: "string", value: "%lazycat%" },
      { name: "status", type: "number", value: "1" },
    ],
  },
  {
    id: "foreach-query",
    label: "IN 批量查询",
    summary: "foreach：把数组展开为安全的 IN 参数列表",
    sqlTemplate: `<select>
  SELECT id, name, email
  FROM users
  <where>
    <if test="ids != null">
      AND id IN
      <foreach collection="ids" item="id" open="(" separator="," close=")">
        #{id}
      </foreach>
    </if>
  </where>
</select>`,
    params: [{ name: "ids", type: "array", value: "[101, 102, 103]" }],
  },
  {
    id: "dynamic-update",
    label: "动态字段更新",
    summary: "set + if：只更新本次传入的字段并清理尾逗号",
    sqlTemplate: `<update>
  UPDATE users
  <set>
    <if test="name != null">name = #{name},</if>
    <if test="email != null">email = #{email},</if>
    <if test="enabled != null">enabled = #{enabled},</if>
  </set>
  WHERE id = #{id}
</update>`,
    params: [
      { name: "id", type: "number", value: "101" },
      { name: "name", type: "string", value: "Lazycat" },
      { name: "email", type: "null", value: "" },
      { name: "enabled", type: "boolean", value: "true" },
    ],
  },
  {
    id: "choose-branch",
    label: "条件分支",
    summary: "choose：在多个排序方案中命中一个分支",
    sqlTemplate: `<select>
  SELECT id, name, created_at
  FROM users
  <choose>
    <when test="sortMode == 'name'">ORDER BY name ASC</when>
    <when test="sortMode == 'created'">ORDER BY created_at DESC</when>
    <otherwise>ORDER BY id DESC</otherwise>
  </choose>
</select>`,
    params: [{ name: "sortMode", type: "string", value: "created" }],
  },
  {
    id: "batch-insert",
    label: "批量写入关联",
    summary: "foreach：复用外层参数批量生成 VALUES 记录",
    sqlTemplate: `<insert>
  INSERT INTO user_role (user_id, role_id)
  VALUES
  <foreach collection="roleIds" item="roleId" separator=",">
    (#{userId}, #{roleId})
  </foreach>
</insert>`,
    params: [
      { name: "userId", type: "number", value: "101" },
      { name: "roleIds", type: "array", value: "[2, 5, 8]" },
    ],
  },
  {
    id: "range-query",
    label: "范围条件查询",
    summary: ">= / <=：在 XML 中使用 &gt;= 与 &lt;= 转义比较运算符",
    sqlTemplate: `<select>
  SELECT id, name, score, created_at
  FROM users
  <where>
    <if test="minScore != null and minScore &gt;= 0">
      AND score &gt;= #{minScore}
    </if>
    <if test="maxScore != null and maxScore &lt;= 100">
      AND score &lt;= #{maxScore}
    </if>
    <if test="startAt != null and startAt != ''">
      AND created_at &gt;= #{startAt}
    </if>
    <if test="endAt != null and endAt != ''">
      AND created_at &lt;= #{endAt}
    </if>
  </where>
</select>`,
    params: [
      { name: "minScore", type: "number", value: "60" },
      { name: "maxScore", type: "number", value: "90" },
      { name: "startAt", type: "string", value: "2026-07-01 00:00:00" },
      { name: "endAt", type: "string", value: "2026-07-31 23:59:59" },
    ],
  },
  {
    id: "trim-where",
    label: "Trim 动态条件",
    summary: "trim：自定义 WHERE 前缀并移除开头的 AND / OR",
    sqlTemplate: `<select>
  SELECT id, name, enabled
  FROM users
  <trim prefix="WHERE " prefixOverrides="AND |OR ">
    <if test="keyword != null and keyword != ''">
      AND name LIKE #{keyword}
    </if>
    <if test="enabled != null">
      AND enabled = #{enabled}
    </if>
  </trim>
</select>`,
    params: [
      { name: "keyword", type: "string", value: "%cat%" },
      { name: "enabled", type: "boolean", value: "true" },
    ],
  },
  {
    id: "batch-delete",
    label: "批量删除",
    summary: "delete + foreach：按归属对象安全展开待删除 ID",
    sqlTemplate: `<delete>
  DELETE FROM user_sessions
  WHERE user_id = #{userId}
    AND id IN
    <foreach collection="sessionIds" item="sessionId" open="(" separator="," close=")">
      #{sessionId}
    </foreach>
</delete>`,
    params: [
      { name: "userId", type: "number", value: "101" },
      { name: "sessionIds", type: "array", value: "[11, 12, 15]" },
    ],
  },
  {
    id: "dynamic-order",
    label: "动态排序字段",
    summary: "${}：演示原样替换与 SQL 注入风险检查",
    sqlTemplate: `<select>
  SELECT id, name, created_at
  FROM users
  ORDER BY \${orderBy}
  LIMIT #{pageSize}
</select>`,
    params: [
      { name: "orderBy", type: "string", value: "created_at DESC" },
      { name: "pageSize", type: "number", value: "20" },
    ],
  },
];

const defaultExample = MYBATIS_EXAMPLES[0];
const mybatisState = {
  nextId: defaultExample.params.length + 1,
  activeExampleId: defaultExample.id,
  sqlTemplate: defaultExample.sqlTemplate,
  paramRows: defaultExample.params.map((param, index) => ({ id: index + 1, ...param })),
};
</script>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import {
  CircleCheck,
  CopyDocument,
  Delete,
  Plus,
  Refresh,
  VideoPlay,
} from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../bridge/tauri";

interface Binding {
  name: string;
  mode: string;
  value: unknown;
}

interface Issue {
  level: string;
  message: string;
}

const examples = MYBATIS_EXAMPLES;
let nextId = mybatisState.nextId;
let extractTimer: ReturnType<typeof setTimeout> | null = null;
let extractRequestId = 0;
let manualExtractRequestId = 0;
let renderRequestId = 0;
let lintRequestId = 0;

const sqlTemplate = ref(mybatisState.sqlTemplate);
const paramRows = ref<ParamRow[]>(mybatisState.paramRows.map((row) => ({ ...row })));
const activeExampleId = ref(mybatisState.activeExampleId);
const activeResultTab = ref("sql");
const renderedSql = ref("");
const bindings = ref<Binding[]>([]);
const issues = ref<Issue[]>([]);
const extractLoading = ref(false);
const renderLoading = ref(false);
const lintLoading = ref(false);
const lintPassed = ref(false);
const lastRenderedSnapshot = ref("");

const activeExample = computed(() =>
  examples.find((example) => example.id === activeExampleId.value),
);
const currentSnapshot = computed(() =>
  JSON.stringify({ sqlTemplate: sqlTemplate.value, params: buildParamsJson() }),
);
const resultStale = computed(
  () => !!renderedSql.value && lastRenderedSnapshot.value !== currentSnapshot.value,
);

function inferType(name: string): ParamType {
  if (/ids$/i.test(name) || /list$/i.test(name)) return "array";
  if (/id$/i.test(name) && !/ids$/i.test(name)) return "number";
  if (/^(is|has|enable|enabled)/i.test(name)) return "boolean";
  return "string";
}

function defaultValue(type: ParamType, name: string): string {
  if (type === "number") return "1";
  if (type === "boolean") return "true";
  if (type === "array") return "[1, 2, 3]";
  if (type === "string") return name;
  return "";
}

function addRow() {
  paramRows.value.push({ id: nextId++, name: "", type: "string", value: "" });
}

function removeRow(id: number) {
  paramRows.value = paramRows.value.filter((row) => row.id !== id);
}

function onTypeChange(row: ParamRow) {
  row.value = defaultValue(row.type, row.name);
}

function buildParamsJson(): string {
  const params: Record<string, unknown> = {};
  for (const row of paramRows.value) {
    const name = row.name.trim();
    if (!name) continue;

    if (row.type === "string") params[name] = row.value;
    if (row.type === "number") {
      const value = Number(row.value);
      params[name] = Number.isFinite(value) ? value : 0;
    }
    if (row.type === "boolean") params[name] = row.value === "true";
    if (row.type === "null") params[name] = null;
    if (row.type === "array") {
      try {
        const value = JSON.parse(row.value);
        params[name] = Array.isArray(value) ? value : [value];
      } catch {
        params[name] = row.value
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean);
      }
    }
  }
  return JSON.stringify(params);
}

function clearResults() {
  renderedSql.value = "";
  bindings.value = [];
  issues.value = [];
  lintPassed.value = false;
  lastRenderedSnapshot.value = "";
  activeResultTab.value = "sql";
}

function applyExample(exampleId: string) {
  const example = examples.find((item) => item.id === exampleId);
  if (!example) return;

  activeExampleId.value = example.id;
  sqlTemplate.value = example.sqlTemplate;
  paramRows.value = example.params.map((param) => ({ id: nextId++, ...param }));
  clearResults();
}

async function extractParams(replace = false, silent = false) {
  const template = sqlTemplate.value;
  if (!template.trim()) return;

  const requestId = ++extractRequestId;
  if (!silent) {
    manualExtractRequestId = requestId;
    extractLoading.value = true;
  }

  try {
    const data = (await invokeToolByChannel("tool:mybatis:extract-params", {
      sqlTemplate: template,
    })) as { params?: string[] };
    if (requestId !== extractRequestId || template !== sqlTemplate.value) return;

    const extracted = Array.isArray(data?.params) ? data.params : [];
    const existingRows = new Map(paramRows.value.map((row) => [row.name, row]));
    if (replace) {
      paramRows.value = extracted.map((name) => {
        const existing = existingRows.get(name);
        if (existing) return existing;
        const type = inferType(name);
        return { id: nextId++, name, type, value: defaultValue(type, name) };
      });
      return;
    }

    for (const name of extracted) {
      if (existingRows.has(name)) continue;
      const type = inferType(name);
      const row = { id: nextId++, name, type, value: defaultValue(type, name) };
      paramRows.value.push(row);
      existingRows.set(name, row);
    }
  } catch (error) {
    if (silent) console.warn("extract params failed:", error);
    else ElMessage.error((error as Error).message);
  } finally {
    if (!silent && manualExtractRequestId === requestId) extractLoading.value = false;
  }
}

watch(sqlTemplate, (template) => {
  const selected = examples.find((example) => example.id === activeExampleId.value);
  if (selected?.sqlTemplate !== template) activeExampleId.value = "";
  lintPassed.value = false;
  if (extractTimer) clearTimeout(extractTimer);
  extractTimer = setTimeout(() => void extractParams(false, true), 800);
});

onUnmounted(() => {
  if (extractTimer) clearTimeout(extractTimer);
  mybatisState.sqlTemplate = sqlTemplate.value;
  mybatisState.paramRows = paramRows.value.map((row) => ({ ...row }));
  mybatisState.nextId = nextId;
  mybatisState.activeExampleId = activeExampleId.value;
});

async function renderSql() {
  const requestId = ++renderRequestId;
  const snapshot = currentSnapshot.value;
  renderLoading.value = true;
  try {
    const data = (await invokeToolByChannel("tool:mybatis:render", {
      sqlTemplate: sqlTemplate.value,
      params: buildParamsJson(),
      safeSubstitution: true,
    })) as { sql?: string; bindings?: Binding[]; warnings?: string[] };
    if (requestId !== renderRequestId) return;

    renderedSql.value = data?.sql ?? "";
    bindings.value = Array.isArray(data?.bindings) ? data.bindings : [];
    const warnings = Array.isArray(data?.warnings) ? data.warnings : [];
    issues.value = warnings.map((message) => ({ level: "warn", message }));
    lastRenderedSnapshot.value = snapshot;
    activeResultTab.value = "sql";
  } catch (error) {
    if (requestId === renderRequestId) ElMessage.error((error as Error).message);
  } finally {
    if (requestId === renderRequestId) renderLoading.value = false;
  }
}

async function lintTemplate() {
  const requestId = ++lintRequestId;
  const template = sqlTemplate.value;
  lintLoading.value = true;
  try {
    const data = (await invokeToolByChannel("tool:mybatis:lint", {
      sqlTemplate: template,
    })) as { issues?: Issue[] };
    if (requestId !== lintRequestId || template !== sqlTemplate.value) return;

    issues.value = Array.isArray(data?.issues) ? data.issues : [];
    lintPassed.value = issues.value.length === 0;
    activeResultTab.value = "issues";
  } catch (error) {
    if (requestId === lintRequestId) ElMessage.error((error as Error).message);
  } finally {
    if (requestId === lintRequestId) lintLoading.value = false;
  }
}

function formatValue(value: unknown): string {
  return typeof value === "string" ? value : (JSON.stringify(value) ?? String(value));
}

async function copyRenderedSql() {
  if (!renderedSql.value) return;
  try {
    await navigator.clipboard.writeText(renderedSql.value);
    ElMessage.success("SQL 已复制");
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
  min-width: 0;
}

.tool-strip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-height: 44px;
  padding: 8px 10px 8px 14px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-0);
}

.example-picker,
.run-actions,
.section-title-wrap,
.section-actions,
.tab-label {
  display: flex;
  align-items: center;
}

.example-picker {
  gap: 10px;
  min-width: 0;
}

.strip-label,
.section-title {
  color: var(--lc-text);
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
}

.example-select {
  width: 190px;
  flex: 0 0 auto;
}

.example-option {
  display: flex;
  justify-content: space-between;
  gap: 24px;
  width: 100%;
}

.example-option span:last-child {
  color: var(--lc-text-muted);
  font-size: 12px;
}

.example-summary,
.result-hint {
  overflow: hidden;
  color: var(--lc-text-secondary);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.run-actions,
.section-actions {
  gap: 8px;
  flex: 0 0 auto;
}

.workspace-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.08fr) minmax(430px, 0.92fr);
  gap: 12px;
  min-width: 0;
}

.workspace-section,
.result-section {
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-0);
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  height: 48px;
  padding: 0 14px;
  border-bottom: 1px solid var(--lc-border);
  background: var(--lc-surface-1);
}

.section-title-wrap {
  gap: 8px;
}

.section-count,
.tab-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  height: 20px;
  padding: 0 6px;
  border-radius: 10px;
  background: var(--lc-surface-3);
  color: var(--lc-text-secondary);
  font-size: 11px;
}

.template-input :deep(.el-textarea__inner) {
  height: 328px;
  padding: 14px 16px;
  border-radius: 0;
  box-shadow: none;
  background: #fbfdff;
  font-family: var(--lc-font-mono);
  font-variant-ligatures: none;
  font-feature-settings:
    "liga" 0,
    "calt" 0;
  font-size: 13px;
  line-height: 1.65;
  tab-size: 2;
}

.template-input :deep(.el-textarea__inner:focus) {
  box-shadow: 0 0 0 1px var(--lc-border-active) inset;
}

.param-table-wrapper {
  height: 328px;
  overflow: auto;
}

.param-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
  font-size: 13px;
}

.param-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 8px;
  border-bottom: 1px solid var(--lc-border);
  background: #fbfdff;
  color: var(--lc-text-secondary);
  font-size: 12px;
  font-weight: 500;
  text-align: left;
}

.param-table td {
  height: 42px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.param-table tbody tr:hover td {
  background: var(--lc-surface-1);
}

.col-name {
  width: 29%;
}
.col-type {
  width: 25%;
}
.col-delete {
  width: 42px;
}
.delete-cell {
  text-align: center;
}

.empty-hint {
  height: 120px !important;
  color: var(--lc-text-muted);
  text-align: center;
}

.param-table :deep(.el-input__wrapper),
.param-table :deep(.el-select__wrapper) {
  box-shadow: none;
  background: transparent;
}

.param-table :deep(.el-input__wrapper:hover),
.param-table :deep(.el-input__wrapper.is-focus),
.param-table :deep(.el-select__wrapper:hover),
.param-table :deep(.el-select__wrapper.is-focused) {
  background: var(--lc-surface-0);
  box-shadow: 0 0 0 1px var(--lc-border-active) inset;
}

.result-section {
  min-height: 230px;
}

.result-tabs :deep(.el-tabs__header) {
  height: 44px;
  margin: 0;
  padding: 0 14px;
  border-bottom: 1px solid var(--lc-border);
  background: var(--lc-surface-1);
}

.result-tabs :deep(.el-tabs__nav-wrap::after) {
  display: none;
}

.result-tabs :deep(.el-tabs__item) {
  height: 44px;
  padding: 0 16px;
  font-size: 13px;
}

.tab-label {
  gap: 6px;
}

.issue-count {
  background: rgba(251, 191, 36, 0.16);
  color: #a16207;
}

.result-pane {
  min-height: 184px;
}

.result-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 38px;
  padding: 0 10px 0 14px;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.sql-output {
  min-height: 145px;
  margin: 0;
  padding: 14px 16px;
  overflow: auto;
  background: #fbfdff;
  color: var(--lc-text);
  font-family: var(--lc-font-mono);
  font-variant-ligatures: none;
  font-feature-settings:
    "liga" 0,
    "calt" 0;
  font-size: 13px;
  line-height: 1.65;
  white-space: pre-wrap;
  word-break: break-word;
}

.binding-value {
  font-family: var(--lc-font-mono);
  font-variant-ligatures: none;
  font-feature-settings:
    "liga" 0,
    "calt" 0;
  color: var(--lc-text);
  word-break: break-all;
}

.issues-pane {
  padding: 12px 14px;
}

.issue-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.issue-row {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-sm);
  background: var(--lc-surface-1);
  color: var(--lc-text-secondary);
  font-size: 13px;
  line-height: 1.5;
}

.result-pane :deep(.el-empty) {
  padding: 24px 0 18px;
}

.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

@media (max-width: 1000px) {
  .workspace-grid {
    grid-template-columns: 1fr;
  }

  .example-summary {
    display: none;
  }
}

@media (max-width: 680px) {
  .tool-strip {
    align-items: stretch;
    flex-direction: column;
  }

  .example-picker {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
  }

  .example-select {
    width: 100%;
  }

  .run-actions {
    justify-content: flex-end;
  }

  .section-header {
    height: auto;
    min-height: 48px;
    flex-wrap: wrap;
    padding: 8px 10px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .mybatis-panel *,
  .mybatis-panel *::before,
  .mybatis-panel *::after {
    transition-duration: 0.01ms !important;
  }
}
</style>
