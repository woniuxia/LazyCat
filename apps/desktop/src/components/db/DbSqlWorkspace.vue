<template>
  <div class="db-sql-workspace">
    <aside class="schema-side">
      <div class="db-selector">
        <el-select
          :model-value="activeDatabase"
          filterable
          size="small"
          placeholder="选择数据库"
          @update:model-value="switchDatabase"
        >
          <el-option v-for="db in opened.databases" :key="db" :label="db" :value="db" />
        </el-select>
        <el-button size="small" circle title="刷新表列表" @click="loadTables">
          <el-icon><Refresh /></el-icon>
        </el-button>
      </div>
      <el-input
        v-model="tableFilter"
        size="small"
        placeholder="过滤表名"
        clearable
        class="table-filter"
      />
      <div v-loading="tablesLoading" class="table-list">
        <div
          v-for="t in filteredTables"
          :key="t.name"
          class="table-item"
          :title="t.comment ? `${t.name} — ${t.comment}` : t.name"
          @dblclick="openDataTab(t.name)"
        >
          <span class="table-icon" :class="t.tableType">{{ t.tableType === "view" ? "V" : "T" }}</span>
          <span class="table-name">{{ t.name }}</span>
          <span v-if="t.comment" class="table-comment">{{ t.comment }}</span>
          <span class="table-actions">
            <el-button link size="small" title="表结构" @click.stop="openStructureTab(t.name)">结构</el-button>
            <el-button link size="small" title="浏览数据" @click.stop="openDataTab(t.name)">数据</el-button>
          </span>
        </div>
        <div v-if="!tablesLoading && filteredTables.length === 0" class="table-empty">
          {{ tables.length === 0 ? "当前库没有表" : "没有匹配的表" }}
        </div>
      </div>
    </aside>

    <main class="work-main">
      <div class="work-toolbar">
        <el-button size="small" type="primary" @click="openQueryTab()">新建查询</el-button>
        <el-button size="small" @click="savedDrawer = true">收藏</el-button>
        <el-button size="small" @click="openHistoryDrawer">历史</el-button>
        <span class="conn-info">
          <span class="env-dot" :style="{ background: DB_ENV_COLORS[connection.envTag] }" />
          {{ DB_ENV_LABELS[connection.envTag] }}
          <el-tag v-if="connection.readOnly" size="small" type="info" effect="plain">只读</el-tag>
          <span class="server-version">{{ opened.serverVersion }}</span>
        </span>
      </div>

      <el-tabs
        v-if="tabs.length > 0"
        v-model="activeTabKey"
        type="card"
        closable
        class="work-tabs"
        @tab-remove="closeTab"
      >
        <el-tab-pane v-for="tab in tabs" :key="tab.key" :name="tab.key">
          <template #label>
            <span class="tab-label">{{ tab.title }}</span>
          </template>

          <!-- 查询页签 -->
          <div v-if="tab.type === 'query'" class="query-pane">
            <div class="query-editor">
              <DbSqlEditor
                :ref="(el) => setEditorRef(tab.key, el)"
                v-model="tab.sql"
                :dialect="dialect"
                :completions="completionWords"
                @execute="runQuery(tab)"
              />
            </div>
            <div class="query-actions">
              <el-button size="small" type="primary" :loading="tab.running" @click="runQuery(tab)">
                执行 (Ctrl+Enter)
              </el-button>
              <el-button v-if="tab.running" size="small" type="warning" @click="cancelQuery(tab)">
                取消
              </el-button>
              <el-button size="small" :disabled="!tab.sql.trim()" @click="saveToFavorites(tab)">
                收藏 SQL
              </el-button>
              <el-button
                size="small"
                :disabled="!lastReadonlySql(tab)"
                title="重新执行查询并全量导出（不受行数上限约束）"
                @click="openExportDialog(tab)"
              >
                导出全部
              </el-button>
              <span v-if="tab.error" class="query-error" :title="tab.error.message">
                第 {{ tab.error.statementIndex + 1 }} 条语句失败：{{ tab.error.message }}
              </span>
            </div>
            <div class="query-results">
              <template v-if="tab.results.length > 0">
                <div v-for="(res, ri) in tab.results" :key="ri" class="result-block">
                  <div v-if="tab.results.length > 1" class="result-title">
                    结果 {{ ri + 1 }}：{{ res.sql.slice(0, 60) }}
                  </div>
                  <DbResultGrid :result="res" class="result-grid" />
                </div>
              </template>
              <div v-else-if="!tab.running" class="result-empty">执行 SQL 后在此查看结果</div>
            </div>
          </div>

          <!-- 表数据页签 -->
          <div v-else-if="tab.type === 'data'" class="data-pane">
            <div class="filter-bar">
              <el-select v-model="tab.draftFilter.column" size="small" placeholder="列" class="f-col" filterable>
                <el-option v-for="c in tab.result?.columns ?? []" :key="c.name" :label="c.name" :value="c.name" />
              </el-select>
              <el-select v-model="tab.draftFilter.op" size="small" class="f-op">
                <el-option v-for="op in FILTER_OPS" :key="op" :label="op" :value="op" />
              </el-select>
              <el-input
                v-model="tab.draftFilter.value"
                size="small"
                placeholder="值"
                class="f-val"
                :disabled="tab.draftFilter.op === 'IS NULL' || tab.draftFilter.op === 'IS NOT NULL'"
                @keyup.enter="addFilter(tab)"
              />
              <el-button size="small" @click="addFilter(tab)">添加筛选</el-button>
              <el-tag
                v-for="(f, fi) in tab.filters"
                :key="fi"
                closable
                size="small"
                class="filter-tag"
                @close="removeFilter(tab, fi)"
              >
                {{ f.column }} {{ f.op }} {{ f.value }}
              </el-tag>
            </div>
            <DbResultGrid
              v-if="tab.result"
              :ref="(el) => setGridRef(tab.key, el)"
              :result="tab.result"
              :editable="!connection.readOnly"
              :pk-columns="tab.pkColumns"
              :applying="tab.applying"
              :total="tab.total"
              :page="tab.page"
              :page-size="tab.pageSize"
              server-sort
              class="data-grid"
              v-loading="tab.loading"
              @page-change="(p) => changePage(tab, p)"
              @sort-change="(col, asc) => changeSort(tab, col, asc)"
              @apply="(changes) => applyChanges(tab, changes)"
            />
            <div v-else v-loading="tab.loading" class="data-loading" />
          </div>

          <!-- 表结构页签 -->
          <DbTableStructure
            v-else
            :connection-id="connection.id"
            :database="activeDatabase"
            :table="tab.table"
            class="structure-pane"
          />
        </el-tab-pane>
      </el-tabs>
      <div v-else class="work-empty">
        <p>双击左侧表名浏览数据，或</p>
        <el-button type="primary" @click="openQueryTab()">新建查询</el-button>
      </div>
    </main>

    <!-- SQL 收藏抽屉 -->
    <el-drawer v-model="savedDrawer" title="SQL 收藏" size="420px">
      <div v-loading="savedLoading" class="drawer-list">
        <div v-for="q in savedQueries" :key="q.id" class="drawer-item" @dblclick="useSavedQuery(q)">
          <div class="drawer-item-head">
            <span class="drawer-item-title">{{ q.title }}</span>
            <el-button link size="small" type="danger" @click.stop="deleteSavedQuery(q)">删除</el-button>
          </div>
          <pre class="drawer-item-sql">{{ q.sql }}</pre>
          <span v-if="!q.connectionId" class="drawer-item-scope">全局</span>
        </div>
        <el-empty v-if="!savedLoading && savedQueries.length === 0" description="双击收藏可回填到查询页签" />
      </div>
    </el-drawer>

    <!-- 执行历史抽屉 -->
    <el-drawer v-model="historyDrawer" title="执行历史" size="480px">
      <div class="drawer-toolbar">
        <el-button size="small" @click="clearHistory">清空本连接历史</el-button>
      </div>
      <div v-loading="historyLoading" class="drawer-list">
        <div v-for="h in history" :key="h.id" class="drawer-item" @dblclick="useHistoryEntry(h)">
          <div class="drawer-item-head">
            <el-tag :type="h.status === 'ok' ? 'success' : 'danger'" size="small" effect="plain">
              {{ h.status === "ok" ? "成功" : "失败" }}
            </el-tag>
            <span class="drawer-item-meta">
              {{ formatTime(h.executedAt) }} · {{ h.durationMs ?? 0 }}ms · {{ h.rowCount ?? 0 }} 行
            </span>
          </div>
          <pre class="drawer-item-sql">{{ h.sql }}</pre>
        </div>
        <el-empty v-if="!historyLoading && history.length === 0" description="暂无执行历史" />
      </div>
    </el-drawer>

    <!-- 导出对话框 -->
    <el-dialog v-model="exportDialog.visible" title="导出查询结果" width="460px">
      <el-form label-width="90px">
        <el-form-item label="格式">
          <el-radio-group v-model="exportDialog.format">
            <el-radio-button value="csv">CSV</el-radio-button>
            <el-radio-button value="json">JSON</el-radio-button>
            <el-radio-button value="insert">INSERT</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item v-if="exportDialog.format === 'insert'" label="目标表名">
          <el-input v-model="exportDialog.tableName" placeholder="INSERT 语句的表名" />
        </el-form-item>
        <el-alert
          type="info"
          :closable="false"
          title="导出会重新执行该查询（不受行数上限约束），结果可能与当前屏幕快照有差异"
        />
      </el-form>
      <template #footer>
        <el-button @click="exportDialog.visible = false">取消</el-button>
        <el-button type="primary" :loading="exportDialog.running" @click="doExport">选择保存位置并导出</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Refresh } from "@element-plus/icons-vue";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../../bridge/tauri";
import DbSqlEditor from "./DbSqlEditor.vue";
import DbResultGrid from "./DbResultGrid.vue";
import DbTableStructure from "./DbTableStructure.vue";
import { renderChangeSql, summarizeChanges } from "../../utils/dbGridChanges";
import { classifyStatement, splitStatements } from "../../utils/dbSqlClassify";
import type { OpenedConnection } from "../../composables/useDbConnections";
import {
  DB_ENV_COLORS,
  DB_ENV_LABELS,
  type DbConfirmReason,
  type DbConnection,
  type DbDataFilter,
  type DbGridChange,
  type DbHistoryEntry,
  type DbNeedsConfirmation,
  type DbQueryExecuteResponse,
  type DbSavedQuery,
  type DbStatementResult,
  type DbTableBrief,
  type DbTableDataResponse,
  type DbTableDetail,
} from "../../types/db";

const FILTER_OPS = ["=", "<>", ">", "<", ">=", "<=", "LIKE", "NOT LIKE", "IS NULL", "IS NOT NULL"] as const;

interface QueryTab {
  key: string;
  type: "query";
  title: string;
  sql: string;
  running: boolean;
  queryId: string | null;
  results: DbStatementResult[];
  error: { statementIndex: number; message: string } | null;
}

interface DataTab {
  key: string;
  type: "data";
  title: string;
  table: string;
  page: number;
  pageSize: number;
  total: number;
  result: DbStatementResult | null;
  pkColumns: string[];
  filters: DbDataFilter[];
  draftFilter: { column: string; op: (typeof FILTER_OPS)[number]; value: string };
  orderBy: { column: string; ascending: boolean } | null;
  loading: boolean;
  applying: boolean;
  requestSeq: number;
}

interface StructureTab {
  key: string;
  type: "structure";
  title: string;
  table: string;
}

type WorkTab = QueryTab | DataTab | StructureTab;

const props = defineProps<{
  connection: DbConnection;
  opened: OpenedConnection;
}>();

const emit = defineEmits<{
  (e: "database-change", database: string): void;
}>();

const dialect = computed(() => (props.connection.engine === "mysql" ? "mysql" : "pg") as "mysql" | "pg");
const activeDatabase = computed(() => props.opened.activeDatabase);

const tables = ref<DbTableBrief[]>([]);
const tablesLoading = ref(false);
const tableFilter = ref("");
let tablesSeq = 0;

const tabs = ref<WorkTab[]>([]);
const activeTabKey = ref("");
let tabCounter = 0;

const completionWords = ref<string[]>([]);
const editorRefs = new Map<string, InstanceType<typeof DbSqlEditor>>();
const gridRefs = new Map<string, InstanceType<typeof DbResultGrid>>();

const savedDrawer = ref(false);
const savedQueries = ref<DbSavedQuery[]>([]);
const savedLoading = ref(false);
const historyDrawer = ref(false);
const history = ref<DbHistoryEntry[]>([]);
const historyLoading = ref(false);

const exportDialog = reactive({
  visible: false,
  format: "csv" as "csv" | "json" | "insert",
  tableName: "exported_table",
  sql: "",
  running: false,
});

const filteredTables = computed(() => {
  const kw = tableFilter.value.trim().toLowerCase();
  if (!kw) return tables.value;
  return tables.value.filter(
    (t) => t.name.toLowerCase().includes(kw) || t.comment.toLowerCase().includes(kw)
  );
});

function setEditorRef(key: string, el: unknown): void {
  if (el) {
    editorRefs.set(key, el as InstanceType<typeof DbSqlEditor>);
  } else {
    editorRefs.delete(key);
  }
}

function setGridRef(key: string, el: unknown): void {
  if (el) {
    gridRefs.set(key, el as InstanceType<typeof DbResultGrid>);
  } else {
    gridRefs.delete(key);
  }
}

// ---------- 表列表 ----------

async function loadTables(): Promise<void> {
  const seq = ++tablesSeq;
  tablesLoading.value = true;
  try {
    const data = (await invokeToolByChannel("tool:db:schema-tables", {
      connectionId: props.connection.id,
      database: activeDatabase.value,
    })) as { tables: DbTableBrief[] };
    if (seq !== tablesSeq) return;
    tables.value = data.tables;
    mergeCompletions(data.tables.map((t) => t.name));
  } catch (error) {
    if (seq === tablesSeq) ElMessage.error((error as Error).message);
  } finally {
    if (seq === tablesSeq) tablesLoading.value = false;
  }
}

function mergeCompletions(words: string[]): void {
  const set = new Set(completionWords.value);
  for (const w of words) {
    set.add(w);
    // schema.table 形态额外补裸表名
    const bare = w.includes(".") ? w.split(".").pop() : null;
    if (bare) set.add(bare);
  }
  completionWords.value = Array.from(set);
}

function switchDatabase(database: string): void {
  if (database === activeDatabase.value) return;
  // 库上下文变化后，旧页签（表数据/结构/查询结果）不再成立，整体关闭
  tabs.value = [];
  activeTabKey.value = "";
  emit("database-change", database);
}

watch(activeDatabase, () => {
  tables.value = [];
  completionWords.value = [];
  void loadTables();
});

onMounted(loadTables);

// ---------- 页签管理 ----------

function openQueryTab(initialSql = ""): QueryTab {
  const key = `q${++tabCounter}`;
  const tab: QueryTab = {
    key,
    type: "query",
    title: `查询 ${tabCounter}`,
    sql: initialSql,
    running: false,
    queryId: null,
    results: [],
    error: null,
  };
  tabs.value = [...tabs.value, tab];
  activeTabKey.value = key;
  return tab;
}

function openDataTab(table: string): void {
  const existing = tabs.value.find((t) => t.type === "data" && t.table === table) as DataTab | undefined;
  if (existing) {
    activeTabKey.value = existing.key;
    return;
  }
  const key = `d${++tabCounter}`;
  const tab: DataTab = reactive({
    key,
    type: "data",
    title: `数据:${shortName(table)}`,
    table,
    page: 0,
    pageSize: 200,
    total: 0,
    result: null,
    pkColumns: [],
    filters: [],
    draftFilter: { column: "", op: "=", value: "" },
    orderBy: null,
    loading: false,
    applying: false,
    requestSeq: 0,
  });
  tabs.value = [...tabs.value, tab];
  activeTabKey.value = key;
  void loadDataPage(tab);
  void loadPkColumns(tab);
}

function openStructureTab(table: string): void {
  const existing = tabs.value.find((t) => t.type === "structure" && t.table === table);
  if (existing) {
    activeTabKey.value = existing.key;
    return;
  }
  const key = `s${++tabCounter}`;
  tabs.value = [...tabs.value, { key, type: "structure", title: `结构:${shortName(table)}`, table }];
  activeTabKey.value = key;
}

function shortName(table: string): string {
  return table.includes(".") ? table.split(".").pop()! : table;
}

function closeTab(key: string | number): void {
  const idx = tabs.value.findIndex((t) => t.key === key);
  if (idx === -1) return;
  tabs.value = tabs.value.filter((t) => t.key !== key);
  if (activeTabKey.value === key) {
    activeTabKey.value = tabs.value[Math.max(0, idx - 1)]?.key ?? "";
  }
}

// ---------- 查询执行 ----------

/** 两段式确认：格式化原因并请求用户确认 */
async function confirmReasons(reasons: DbConfirmReason[]): Promise<boolean> {
  const lines = reasons.map((r) => {
    const label = r.kind === "prodWrite" ? "生产环境写操作" : "UPDATE/DELETE 缺少 WHERE";
    return `【${label}】${r.preview}`;
  });
  try {
    await ElMessageBox.confirm(lines.join("\n"), "执行前确认", {
      type: "warning",
      confirmButtonText: "确认执行",
      cancelButtonText: "取消",
      customStyle: { whiteSpace: "pre-line" } as Record<string, string>,
    });
    return true;
  } catch {
    return false;
  }
}

async function runQuery(tab: QueryTab): Promise<void> {
  if (tab.running) return;
  const editor = editorRefs.get(tab.key);
  const sql = editor?.getExecutableSql() ?? tab.sql.trim();
  if (!sql) {
    ElMessage.warning("没有可执行的语句");
    return;
  }
  const queryId = crypto.randomUUID();
  tab.queryId = queryId;
  tab.running = true;
  tab.error = null;
  try {
    let data = (await invokeToolByChannel("tool:db:query-execute", {
      connectionId: props.connection.id,
      database: activeDatabase.value,
      sql,
      queryId,
    })) as DbQueryExecuteResponse | DbNeedsConfirmation;

    if ("needsConfirmation" in data && data.needsConfirmation) {
      const ok = await confirmReasons(data.reasons);
      if (!ok) return;
      data = (await invokeToolByChannel("tool:db:query-execute", {
        connectionId: props.connection.id,
        database: activeDatabase.value,
        sql,
        queryId,
        confirmed: true,
      })) as DbQueryExecuteResponse;
    }

    const response = data as DbQueryExecuteResponse;
    tab.results = response.results;
    tab.error = response.error;
  } catch (error) {
    tab.results = [];
    tab.error = { statementIndex: 0, message: (error as Error).message };
  } finally {
    tab.running = false;
    tab.queryId = null;
  }
}

async function cancelQuery(tab: QueryTab): Promise<void> {
  if (!tab.queryId) return;
  try {
    await invokeToolByChannel("tool:db:query-cancel", {
      connectionId: props.connection.id,
      queryId: tab.queryId,
    });
    ElMessage.info("已发送取消指令");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

/** 页签中最近一次可导出（单条只读）的 SQL */
function lastReadonlySql(tab: QueryTab): string | null {
  const sql = tab.sql.trim();
  if (!sql) return null;
  const stmts = splitStatements(sql, dialect.value);
  if (stmts.length !== 1) return null;
  return classifyStatement(stmts[0], dialect.value).readonly ? stmts[0] : null;
}

// ---------- 表数据浏览 ----------

async function loadPkColumns(tab: DataTab): Promise<void> {
  try {
    const detail = (await invokeToolByChannel("tool:db:schema-table-detail", {
      connectionId: props.connection.id,
      database: activeDatabase.value,
      table: tab.table,
    })) as DbTableDetail;
    tab.pkColumns = detail.columns.filter((c) => c.primaryKey).map((c) => c.name);
    mergeCompletions(detail.columns.map((c) => c.name));
  } catch {
    tab.pkColumns = [];
  }
}

async function loadDataPage(tab: DataTab): Promise<void> {
  const seq = ++tab.requestSeq;
  tab.loading = true;
  try {
    const data = (await invokeToolByChannel("tool:db:table-data-page", {
      connectionId: props.connection.id,
      database: activeDatabase.value,
      table: tab.table,
      page: tab.page,
      pageSize: tab.pageSize,
      filters: tab.filters,
      orderBy: tab.orderBy ? { column: tab.orderBy.column, ascending: tab.orderBy.ascending } : undefined,
    })) as DbTableDataResponse;
    if (seq !== tab.requestSeq) return;
    tab.result = data.result;
    tab.total = data.total;
  } catch (error) {
    if (seq === tab.requestSeq) ElMessage.error((error as Error).message);
  } finally {
    if (seq === tab.requestSeq) tab.loading = false;
  }
}

function changePage(tab: DataTab, page: number): void {
  tab.page = page;
  void loadDataPage(tab);
}

function changeSort(tab: DataTab, column: string | null, ascending: boolean): void {
  tab.orderBy = column ? { column, ascending } : null;
  tab.page = 0;
  void loadDataPage(tab);
}

function addFilter(tab: DataTab): void {
  const d = tab.draftFilter;
  if (!d.column) {
    ElMessage.warning("请选择筛选列");
    return;
  }
  const needsValue = d.op !== "IS NULL" && d.op !== "IS NOT NULL";
  if (needsValue && d.value === "") {
    ElMessage.warning("请填写筛选值");
    return;
  }
  tab.filters = [...tab.filters, { column: d.column, op: d.op, value: needsValue ? d.value : "" }];
  tab.draftFilter = { column: "", op: "=", value: "" };
  tab.page = 0;
  void loadDataPage(tab);
}

function removeFilter(tab: DataTab, index: number): void {
  tab.filters = tab.filters.filter((_, i) => i !== index);
  tab.page = 0;
  void loadDataPage(tab);
}

async function applyChanges(tab: DataTab, changes: DbGridChange[]): Promise<void> {
  const previews = changes.map((c) =>
    renderChangeSql(c, tab.table, activeDatabase.value, props.connection.engine)
  );
  try {
    await ElMessageBox.confirm(
      `<div class="db-apply-preview"><p>${summarizeChanges(changes)}，即将执行：</p><pre>${escapeHtml(
        previews.join("\n")
      )}</pre></div>`,
      "应用更改",
      {
        dangerouslyUseHTMLString: true,
        confirmButtonText: "在事务中执行",
        cancelButtonText: "取消",
        customClass: "db-apply-confirm",
      }
    );
  } catch {
    return;
  }

  tab.applying = true;
  try {
    let data = (await invokeToolByChannel("tool:db:table-apply-changes", {
      connectionId: props.connection.id,
      database: activeDatabase.value,
      table: tab.table,
      changes,
    })) as Record<string, unknown>;

    if (data.needsConfirmation) {
      const ok = await confirmReasons(data.reasons as DbConfirmReason[]);
      if (!ok) return;
      data = (await invokeToolByChannel("tool:db:table-apply-changes", {
        connectionId: props.connection.id,
        database: activeDatabase.value,
        table: tab.table,
        changes,
        confirmed: true,
      })) as Record<string, unknown>;
    }

    if (data.ok) {
      ElMessage.success(`已应用 ${changes.length} 条变更`);
      gridRefs.get(tab.key)?.clearStaged();
      void loadDataPage(tab);
    } else {
      ElMessageBox.alert(String(data.message ?? "应用失败"), "变更未生效", { type: "error" });
    }
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    tab.applying = false;
  }
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// ---------- 导出 ----------

function openExportDialog(tab: QueryTab): void {
  const sql = lastReadonlySql(tab);
  if (!sql) return;
  exportDialog.sql = sql;
  exportDialog.visible = true;
}

async function doExport(): Promise<void> {
  const ext = exportDialog.format === "insert" ? "sql" : exportDialog.format;
  const path = await saveDialog({
    title: "导出查询结果",
    defaultPath: `export-${Date.now()}.${ext}`,
    filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
  });
  if (!path) return;
  exportDialog.running = true;
  try {
    const data = (await invokeToolByChannel("tool:db:result-export", {
      connectionId: props.connection.id,
      database: activeDatabase.value,
      sql: exportDialog.sql,
      format: exportDialog.format,
      outputPath: path,
      queryId: crypto.randomUUID(),
      tableName: exportDialog.tableName,
    })) as { rowCount: number; path: string };
    ElMessage.success(`已导出 ${data.rowCount} 行到 ${data.path}`);
    exportDialog.visible = false;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    exportDialog.running = false;
  }
}

// ---------- 收藏与历史 ----------

watch(savedDrawer, (open) => {
  if (open) void loadSavedQueries();
});

async function loadSavedQueries(): Promise<void> {
  savedLoading.value = true;
  try {
    const data = (await invokeToolByChannel("tool:db:saved-query-list", {
      connectionId: props.connection.id,
    })) as { queries: DbSavedQuery[] };
    savedQueries.value = data.queries;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    savedLoading.value = false;
  }
}

async function saveToFavorites(tab: QueryTab): Promise<void> {
  try {
    const { value } = await ElMessageBox.prompt("收藏标题", "收藏 SQL", {
      inputValue: tab.sql.trim().slice(0, 30),
      inputValidator: (v: string) => (v.trim() ? true : "标题不能为空"),
    });
    await invokeToolByChannel("tool:db:saved-query-save", {
      connectionId: props.connection.id,
      title: value.trim(),
      sql: tab.sql.trim(),
    });
    ElMessage.success("已收藏");
  } catch {
    /* 用户取消 */
  }
}

function useSavedQuery(q: DbSavedQuery): void {
  openQueryTab(q.sql);
  savedDrawer.value = false;
}

async function deleteSavedQuery(q: DbSavedQuery): Promise<void> {
  await invokeToolByChannel("tool:db:saved-query-delete", { id: q.id });
  await loadSavedQueries();
}

function openHistoryDrawer(): void {
  historyDrawer.value = true;
  void loadHistory();
}

async function loadHistory(): Promise<void> {
  historyLoading.value = true;
  try {
    const data = (await invokeToolByChannel("tool:db:history-list", {
      connectionId: props.connection.id,
      limit: 200,
    })) as { history: DbHistoryEntry[] };
    history.value = data.history;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    historyLoading.value = false;
  }
}

async function clearHistory(): Promise<void> {
  try {
    await ElMessageBox.confirm("清空本连接的执行历史？", "确认", { type: "warning" });
  } catch {
    return;
  }
  await invokeToolByChannel("tool:db:history-clear", { connectionId: props.connection.id });
  await loadHistory();
}

function useHistoryEntry(h: DbHistoryEntry): void {
  openQueryTab(h.sql);
  historyDrawer.value = false;
}

function formatTime(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getMonth() + 1}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
</script>

<style scoped>
.db-sql-workspace {
  height: 100%;
  display: flex;
  min-height: 0;
}
.schema-side {
  width: 240px;
  flex-shrink: 0;
  border-right: 1px solid var(--lc-border);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 10px 10px 0;
  min-height: 0;
}
.db-selector {
  display: flex;
  gap: 6px;
}
.db-selector .el-select {
  flex: 1;
}
.table-filter {
  flex-shrink: 0;
}
.table-list {
  flex: 1;
  overflow: auto;
  min-height: 0;
}
.table-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px;
  border-radius: 6px;
  cursor: default;
  font-size: 13px;
}
.table-item:hover {
  background: var(--el-fill-color-light);
}
.table-item .table-actions {
  display: none;
  margin-left: auto;
  flex-shrink: 0;
}
.table-item:hover .table-actions {
  display: inline-flex;
}
.table-icon {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border-radius: 4px;
  font-size: 11px;
  line-height: 18px;
  text-align: center;
  color: #fff;
  background: var(--el-color-primary);
}
.table-icon.view {
  background: var(--el-color-success);
}
.table-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.table-comment {
  color: var(--el-text-color-placeholder);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.table-empty {
  padding: 20px;
  text-align: center;
  color: var(--el-text-color-placeholder);
  font-size: 13px;
}
.work-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 10px 0 10px 10px;
}
.work-toolbar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.conn-info {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.env-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.server-version {
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.work-tabs {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.work-tabs :deep(.el-tabs__content) {
  flex: 1;
  min-height: 0;
}
.work-tabs :deep(.el-tab-pane) {
  height: 100%;
}
.work-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--el-text-color-secondary);
}
.query-pane {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}
.query-editor {
  height: 32%;
  min-height: 140px;
  flex-shrink: 0;
}
.query-actions {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}
.query-error {
  color: var(--el-color-danger);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.query-results {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.result-block {
  display: flex;
  flex-direction: column;
  min-height: 220px;
  flex: 1;
}
.result-title {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-bottom: 4px;
}
.result-grid {
  flex: 1;
}
.result-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--el-text-color-placeholder);
  font-size: 13px;
}
.data-pane {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}
.filter-bar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.f-col {
  width: 150px;
}
.f-op {
  width: 110px;
}
.f-val {
  width: 160px;
}
.filter-tag {
  font-family: var(--lc-font-mono, "Consolas", monospace);
}
.data-grid {
  flex: 1;
  min-height: 0;
}
.data-loading {
  flex: 1;
}
.structure-pane {
  height: 100%;
}
.drawer-toolbar {
  margin-bottom: 8px;
}
.drawer-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.drawer-item {
  border: 1px solid var(--lc-border);
  border-radius: 8px;
  padding: 8px 10px;
  cursor: default;
  position: relative;
}
.drawer-item:hover {
  border-color: var(--el-color-primary-light-5);
}
.drawer-item-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}
.drawer-item-title {
  font-weight: 600;
  font-size: 13px;
}
.drawer-item-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.drawer-item-sql {
  margin: 0;
  font-size: 12px;
  font-family: var(--lc-font-mono, "Consolas", monospace);
  color: var(--el-text-color-regular);
  max-height: 80px;
  overflow: hidden;
  white-space: pre-wrap;
  word-break: break-all;
}
.drawer-item-scope {
  position: absolute;
  top: 8px;
  right: 10px;
  font-size: 11px;
  color: var(--el-text-color-placeholder);
}
</style>

<style>
/* 应用更改预览弹窗（非 scoped：MessageBox 渲染在 body 下） */
.db-apply-confirm {
  max-width: 640px;
  --el-messagebox-width: 640px;
}
.db-apply-preview pre {
  max-height: 300px;
  overflow: auto;
  background: var(--el-fill-color-light);
  padding: 10px;
  border-radius: 6px;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
