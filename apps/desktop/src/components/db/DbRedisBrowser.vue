<template>
  <div class="db-redis-browser">
    <aside class="key-side">
      <div class="key-toolbar">
        <el-select
          :model-value="opened.activeDatabase"
          size="small"
          class="db-select"
          @update:model-value="switchDb"
        >
          <el-option v-for="db in opened.databases" :key="db" :label="`db${db}`" :value="db" />
        </el-select>
        <el-input
          v-model="pattern"
          size="small"
          placeholder="匹配模式，如 user:*"
          clearable
          @keyup.enter="restartScan"
        />
        <el-button size="small" @click="restartScan">扫描</el-button>
      </div>
      <div class="key-meta">
        已加载 {{ loadedKeys.length }} 个 key
        <el-button v-if="!scanDone" link size="small" :loading="scanning" @click="scanMore">
          加载更多
        </el-button>
        <span v-else class="scan-done">已全部加载</span>
      </div>
      <div v-loading="scanning && loadedKeys.length === 0" class="key-tree">
        <el-tree
          :data="keyTree"
          :props="{ label: 'label', children: 'children' }"
          node-key="key"
          @node-click="onNodeClick"
        >
          <template #default="{ data }">
            <span class="tree-node">
              <el-tag v-if="data.key" :type="typeTagOf(data.keyType)" size="small" effect="plain" class="type-tag">
                {{ data.keyType }}
              </el-tag>
              <span class="tree-label">{{ data.label }}</span>
              <span v-if="!data.key" class="tree-count">{{ data.count }}</span>
            </span>
          </template>
        </el-tree>
        <el-empty
          v-if="!scanning && loadedKeys.length === 0"
          description="没有扫描到 key"
          :image-size="64"
        />
      </div>
    </aside>

    <main class="detail-main">
      <template v-if="detail">
        <div class="detail-head">
          <el-tag :type="typeTagOf(detail.type)" size="small">{{ detail.type }}</el-tag>
          <span class="detail-key" :title="detail.key">{{ detail.key }}</span>
          <el-button size="small" circle title="刷新" @click="loadDetail(detail.key)">
            <el-icon><Refresh /></el-icon>
          </el-button>
        </div>
        <div class="detail-meta">
          <span>TTL：{{ ttlText }}</span>
          <span>编码：{{ detail.encoding }}</span>
          <span v-if="detail.memory !== null">内存：{{ formatBytes(detail.memory) }}</span>
          <span v-if="detail.total > 0">共 {{ detail.total }} 项</span>
          <span v-if="detail.truncated" class="truncated-hint">（仅展示前 {{ memberCount }} 项）</span>
        </div>
        <div v-if="!connection.readOnly" class="detail-actions">
          <el-button size="small" @click="editTtl">改 TTL</el-button>
          <el-button size="small" @click="renameKey">重命名</el-button>
          <el-button size="small" type="danger" plain @click="deleteKey">删除 key</el-button>
          <el-button v-if="detail.type === 'string'" size="small" type="primary" plain @click="editStringValue">
            编辑值
          </el-button>
          <el-button v-else-if="canAddMember" size="small" type="primary" plain @click="addMember">
            添加成员
          </el-button>
        </div>

        <!-- string 值 -->
        <pre v-if="detail.type === 'string'" class="string-value">{{ prettyStringValue }}</pre>

        <!-- hash -->
        <el-table v-else-if="detail.type === 'hash'" :data="hashEntries" size="small" border height="100%" class="member-table">
          <el-table-column prop="field" label="field" min-width="160" show-overflow-tooltip />
          <el-table-column prop="value" label="value" min-width="240" show-overflow-tooltip />
          <el-table-column v-if="!connection.readOnly" label="操作" width="120" align="center">
            <template #default="{ row }">
              <el-button link size="small" @click="editHashField(row)">改值</el-button>
              <el-button link size="small" type="danger" @click="writeKey('hdel', { field: row.field })">删除</el-button>
            </template>
          </el-table-column>
        </el-table>

        <!-- list -->
        <el-table v-else-if="detail.type === 'list'" :data="listEntries" size="small" border height="100%" class="member-table">
          <el-table-column prop="index" label="#" width="70" />
          <el-table-column prop="value" label="value" min-width="260" show-overflow-tooltip />
          <el-table-column v-if="!connection.readOnly" label="操作" width="120" align="center">
            <template #default="{ row }">
              <el-button link size="small" @click="editListItem(row)">改值</el-button>
              <el-button link size="small" type="danger" @click="writeKey('lrem', { value: row.value })">移除</el-button>
            </template>
          </el-table-column>
        </el-table>

        <!-- set -->
        <el-table v-else-if="detail.type === 'set'" :data="setEntries" size="small" border height="100%" class="member-table">
          <el-table-column prop="member" label="member" min-width="260" show-overflow-tooltip />
          <el-table-column v-if="!connection.readOnly" label="操作" width="90" align="center">
            <template #default="{ row }">
              <el-button link size="small" type="danger" @click="writeKey('srem', { member: row.member })">移除</el-button>
            </template>
          </el-table-column>
        </el-table>

        <!-- zset -->
        <el-table v-else-if="detail.type === 'zset'" :data="zsetEntries" size="small" border height="100%" class="member-table">
          <el-table-column prop="member" label="member" min-width="200" show-overflow-tooltip />
          <el-table-column prop="score" label="score" width="140" />
          <el-table-column v-if="!connection.readOnly" label="操作" width="120" align="center">
            <template #default="{ row }">
              <el-button link size="small" @click="editZsetScore(row)">改分</el-button>
              <el-button link size="small" type="danger" @click="writeKey('zrem', { member: row.member })">移除</el-button>
            </template>
          </el-table-column>
        </el-table>

        <pre v-else class="string-value">{{ detail.value }}</pre>
      </template>
      <div v-else class="detail-empty">
        <p>点击左侧 key 查看详情</p>
        <p class="hint">{{ connection.readOnly ? "只读连接：所有写操作已禁用" : "支持编辑值、TTL、重命名与成员级增删改" }}</p>
      </div>

      <!-- 命令控制台 -->
      <div class="console" :class="{ collapsed: !consoleOpen }">
        <div class="console-head" @click="consoleOpen = !consoleOpen">
          <span>命令控制台</span>
          <span class="console-hint">阻塞/订阅类命令不可用；危险命令需输入命令名确认</span>
        </div>
        <template v-if="consoleOpen">
          <div ref="consoleLogEl" class="console-log">
            <div v-for="(entry, i) in consoleLog" :key="i" :class="['log-entry', entry.kind]">
              <pre>{{ entry.text }}</pre>
            </div>
          </div>
          <el-input
            v-model="consoleInput"
            size="small"
            placeholder="输入 Redis 命令，如 GET user:1:name"
            class="console-input"
            @keyup.enter="runCommand"
          >
            <template #append>
              <el-button :loading="commandRunning" @click="runCommand">执行</el-button>
            </template>
          </el-input>
        </template>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Refresh } from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../../bridge/tauri";
import { buildRedisKeyTree, type RedisTreeNode } from "../../utils/dbRedisKeyTree";
import type { OpenedConnection } from "../../composables/useDbConnections";
import type {
  DbConnection,
  RedisCommandResponse,
  RedisHashEntry,
  RedisKeyDetail,
  RedisScanItem,
  RedisScanResponse,
  RedisZsetEntry,
} from "../../types/db";

const props = defineProps<{
  connection: DbConnection;
  opened: OpenedConnection;
}>();

const emit = defineEmits<{
  (e: "database-change", database: string): void;
}>();

const pattern = ref("");
const loadedKeys = ref<RedisScanItem[]>([]);
const cursor = ref(0);
const scanDone = ref(false);
const scanning = ref(false);
let scanSeq = 0;

const detail = ref<RedisKeyDetail | null>(null);
let detailSeq = 0;

const consoleOpen = ref(false);
const consoleInput = ref("");
const consoleLog = ref<Array<{ kind: "cmd" | "ok" | "err"; text: string }>>([]);
const commandRunning = ref(false);
const consoleLogEl = ref<HTMLElement | null>(null);

const keyTree = computed<RedisTreeNode[]>(() => buildRedisKeyTree(loadedKeys.value));
const memberCount = computed(() => {
  const v = detail.value?.value;
  return Array.isArray(v) ? v.length : 0;
});
const canAddMember = computed(() =>
  ["hash", "list", "set", "zset"].includes(detail.value?.type ?? "")
);

const hashEntries = computed(() => (detail.value?.value as RedisHashEntry[]) ?? []);
const listEntries = computed(() =>
  ((detail.value?.value as string[]) ?? []).map((value, index) => ({ index, value }))
);
const setEntries = computed(() =>
  ((detail.value?.value as string[]) ?? []).map((member) => ({ member }))
);
const zsetEntries = computed(() => (detail.value?.value as RedisZsetEntry[]) ?? []);

const ttlText = computed(() => {
  const ttl = detail.value?.ttl ?? -2;
  if (ttl === -1) return "永不过期";
  if (ttl < 0) return "-";
  return `${ttl} 秒`;
});

const prettyStringValue = computed(() => {
  const raw = detail.value?.value;
  if (typeof raw !== "string") return "";
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
});

function typeTagOf(type?: string): "primary" | "success" | "warning" | "danger" | "info" {
  switch (type) {
    case "string":
      return "primary";
    case "hash":
      return "success";
    case "list":
      return "warning";
    case "zset":
      return "danger";
    default:
      return "info";
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

// ---------- 扫描 ----------

function restartScan(): void {
  loadedKeys.value = [];
  cursor.value = 0;
  scanDone.value = false;
  void scanMore();
}

async function scanMore(): Promise<void> {
  if (scanning.value || scanDone.value) return;
  const seq = ++scanSeq;
  scanning.value = true;
  try {
    const data = (await invokeToolByChannel("tool:db:redis-scan", {
      connectionId: props.connection.id,
      db: props.opened.activeDatabase,
      cursor: cursor.value,
      pattern: pattern.value.trim(),
      count: 300,
    })) as RedisScanResponse;
    if (seq !== scanSeq) return;
    loadedKeys.value = [...loadedKeys.value, ...data.keys];
    cursor.value = data.cursor;
    scanDone.value = data.done;
  } catch (error) {
    if (seq === scanSeq) ElMessage.error((error as Error).message);
  } finally {
    if (seq === scanSeq) scanning.value = false;
  }
}

function switchDb(db: string): void {
  detail.value = null;
  emit("database-change", db);
}

watch(
  () => props.opened.activeDatabase,
  () => {
    detail.value = null;
    restartScan();
  }
);

onMounted(restartScan);

// ---------- 详情 ----------

function onNodeClick(data: RedisTreeNode): void {
  if (data.key) void loadDetail(data.key);
}

async function loadDetail(key: string): Promise<void> {
  const seq = ++detailSeq;
  try {
    const data = (await invokeToolByChannel("tool:db:redis-key-detail", {
      connectionId: props.connection.id,
      db: props.opened.activeDatabase,
      key,
    })) as RedisKeyDetail;
    if (seq === detailSeq) detail.value = data;
  } catch (error) {
    if (seq === detailSeq) {
      ElMessage.error((error as Error).message);
      detail.value = null;
    }
  }
}

// ---------- 写操作 ----------

async function writeKey(action: string, extra: Record<string, unknown>, key?: string): Promise<void> {
  const targetKey = key ?? detail.value?.key;
  if (!targetKey) return;
  try {
    await invokeToolByChannel("tool:db:redis-key-write", {
      connectionId: props.connection.id,
      db: props.opened.activeDatabase,
      writeAction: action,
      key: targetKey,
      ...extra,
    });
    ElMessage.success("已执行");
    if (action === "del") {
      loadedKeys.value = loadedKeys.value.filter((k) => k.key !== targetKey);
      detail.value = null;
    } else if (action === "rename") {
      restartScan();
      void loadDetail(String(extra.newKey));
    } else {
      void loadDetail(targetKey);
    }
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function editTtl(): Promise<void> {
  if (!detail.value) return;
  try {
    const { value } = await ElMessageBox.prompt(
      "输入过期秒数；输入 -1 表示永不过期",
      "修改 TTL",
      {
        inputValue: detail.value.ttl >= 0 ? String(detail.value.ttl) : "-1",
        inputPattern: /^-?\d+$/,
        inputErrorMessage: "请输入整数",
      }
    );
    await writeKey("expire", { ttlSecs: Number(value) });
  } catch {
    /* 取消 */
  }
}

async function renameKey(): Promise<void> {
  if (!detail.value) return;
  try {
    const { value } = await ElMessageBox.prompt("新的 key 名称", "重命名", {
      inputValue: detail.value.key,
      inputValidator: (v: string) => (v.trim() ? true : "不能为空"),
    });
    await writeKey("rename", { newKey: value.trim() });
  } catch {
    /* 取消 */
  }
}

async function deleteKey(): Promise<void> {
  if (!detail.value) return;
  try {
    await ElMessageBox.confirm(`删除 key「${detail.value.key}」？`, "删除", { type: "warning" });
  } catch {
    return;
  }
  await writeKey("del", {});
}

async function editStringValue(): Promise<void> {
  if (!detail.value || typeof detail.value.value !== "string") return;
  try {
    const { value } = await ElMessageBox.prompt("新的字符串值", "编辑值", {
      inputType: "textarea",
      inputValue: detail.value.value,
    });
    await writeKey("set_string", { value });
  } catch {
    /* 取消 */
  }
}

async function editHashField(row: RedisHashEntry): Promise<void> {
  try {
    const { value } = await ElMessageBox.prompt(`字段 ${row.field} 的新值`, "编辑字段", {
      inputType: "textarea",
      inputValue: row.value,
    });
    await writeKey("hset", { field: row.field, value });
  } catch {
    /* 取消 */
  }
}

async function editListItem(row: { index: number; value: string }): Promise<void> {
  try {
    const { value } = await ElMessageBox.prompt(`第 ${row.index} 个元素的新值`, "编辑元素", {
      inputType: "textarea",
      inputValue: row.value,
    });
    await writeKey("lset", { index: row.index, value });
  } catch {
    /* 取消 */
  }
}

async function editZsetScore(row: RedisZsetEntry): Promise<void> {
  try {
    const { value } = await ElMessageBox.prompt(`成员 ${row.member} 的新分值`, "修改分值", {
      inputValue: String(row.score),
      inputPattern: /^-?\d+(\.\d+)?$/,
      inputErrorMessage: "请输入数字",
    });
    await writeKey("zadd", { member: row.member, score: Number(value) });
  } catch {
    /* 取消 */
  }
}

async function addMember(): Promise<void> {
  const type = detail.value?.type;
  try {
    if (type === "hash") {
      const { value: field } = await ElMessageBox.prompt("字段名", "添加字段");
      const { value } = await ElMessageBox.prompt("字段值", "添加字段", { inputType: "textarea" });
      await writeKey("hset", { field: field.trim(), value });
    } else if (type === "list") {
      const { value } = await ElMessageBox.prompt("追加到列表尾部的值", "追加元素", {
        inputType: "textarea",
      });
      await writeKey("rpush", { value });
    } else if (type === "set") {
      const { value } = await ElMessageBox.prompt("新成员", "添加成员");
      await writeKey("sadd", { member: value.trim() });
    } else if (type === "zset") {
      const { value: member } = await ElMessageBox.prompt("成员", "添加成员");
      const { value: score } = await ElMessageBox.prompt("分值", "添加成员", {
        inputPattern: /^-?\d+(\.\d+)?$/,
        inputErrorMessage: "请输入数字",
      });
      await writeKey("zadd", { member: member.trim(), score: Number(score) });
    }
  } catch {
    /* 取消 */
  }
}

// ---------- 命令控制台 ----------

async function runCommand(): Promise<void> {
  const command = consoleInput.value.trim();
  if (!command || commandRunning.value) return;
  commandRunning.value = true;
  appendLog("cmd", `> ${command}`);
  try {
    let data = (await invokeToolByChannel("tool:db:redis-command", {
      connectionId: props.connection.id,
      db: props.opened.activeDatabase,
      command,
    })) as RedisCommandResponse | { needsConfirmation: true; reasons: Array<{ verb: string }> };

    if ("needsConfirmation" in data && data.needsConfirmation) {
      const verb = data.reasons[0]?.verb ?? "";
      let typed: string;
      try {
        const { value } = await ElMessageBox.prompt(
          `「${verb}」是破坏性命令。如确认执行，请输入命令名 ${verb}：`,
          "危险命令确认",
          { inputValidator: (v: string) => v.trim().toUpperCase() === verb || "输入与命令名不一致" }
        );
        typed = value;
      } catch {
        appendLog("err", "(已取消)");
        return;
      }
      if (typed.trim().toUpperCase() !== verb) return;
      data = (await invokeToolByChannel("tool:db:redis-command", {
        connectionId: props.connection.id,
        db: props.opened.activeDatabase,
        command,
        confirmed: true,
      })) as RedisCommandResponse;
    }

    const resp = data as RedisCommandResponse;
    appendLog("ok", `${formatResult(resp.result)}\n(${resp.durationMs} ms)`);
    consoleInput.value = "";
  } catch (error) {
    appendLog("err", (error as Error).message);
  } finally {
    commandRunning.value = false;
  }
}

function formatResult(result: unknown): string {
  if (result === null) return "(nil)";
  if (typeof result === "string") return result;
  return JSON.stringify(result, null, 2);
}

function appendLog(kind: "cmd" | "ok" | "err", text: string): void {
  consoleLog.value = [...consoleLog.value.slice(-100), { kind, text }];
  void nextTick(() => {
    consoleLogEl.value?.scrollTo({ top: consoleLogEl.value.scrollHeight });
  });
}
</script>

<style scoped>
.db-redis-browser {
  height: 100%;
  display: flex;
  min-height: 0;
}
.key-side {
  width: 300px;
  flex-shrink: 0;
  border-right: 1px solid var(--lc-border);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 10px 10px 0;
  min-height: 0;
}
.key-toolbar {
  display: flex;
  gap: 6px;
}
.db-select {
  width: 90px;
  flex-shrink: 0;
}
.key-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  display: flex;
  align-items: center;
  gap: 8px;
}
.scan-done {
  color: var(--el-color-success);
}
.key-tree {
  flex: 1;
  overflow: auto;
  min-height: 0;
}
.tree-node {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
}
.type-tag {
  flex-shrink: 0;
  transform: scale(0.85);
}
.tree-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tree-count {
  color: var(--el-text-color-placeholder);
  font-size: 12px;
}
.detail-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 0 10px 10px;
  min-height: 0;
}
.detail-head {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}
.detail-key {
  font-family: var(--lc-font-mono, "Consolas", monospace);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.detail-meta {
  flex-shrink: 0;
  display: flex;
  gap: 16px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.truncated-hint {
  color: var(--el-color-warning);
}
.detail-actions {
  flex-shrink: 0;
  display: flex;
  gap: 8px;
}
.string-value {
  flex: 1;
  margin: 0;
  padding: 12px;
  overflow: auto;
  background: var(--el-fill-color-light);
  border-radius: 8px;
  font-family: var(--lc-font-mono, "Consolas", monospace);
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
  min-height: 0;
}
.member-table {
  flex: 1;
  min-height: 0;
}
.detail-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--el-text-color-secondary);
}
.detail-empty .hint {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}
.console {
  flex-shrink: 0;
  border: 1px solid var(--lc-border);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  max-height: 260px;
}
.console.collapsed {
  max-height: none;
}
.console-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  user-select: none;
}
.console-hint {
  font-size: 12px;
  font-weight: normal;
  color: var(--el-text-color-placeholder);
}
.console-log {
  flex: 1;
  overflow: auto;
  padding: 4px 10px;
  min-height: 80px;
  max-height: 160px;
  background: var(--el-fill-color-light);
}
.log-entry pre {
  margin: 2px 0;
  font-size: 12px;
  font-family: var(--lc-font-mono, "Consolas", monospace);
  white-space: pre-wrap;
  word-break: break-all;
}
.log-entry.cmd pre {
  color: var(--el-color-primary);
}
.log-entry.err pre {
  color: var(--el-color-danger);
}
.console-input {
  padding: 6px 10px 8px;
}
</style>
