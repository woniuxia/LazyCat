<template>
  <div class="regex-panel">
    <!-- 正则输入区 -->
    <div class="regex-input-section">
      <div class="regex-input-row">
        <el-input
          v-model="pattern"
          placeholder="输入正则表达式"
          class="regex-pattern-input"
          clearable
        >
          <template #prepend>/</template>
          <template #append>/{{ flags }}</template>
        </el-input>
      </div>
      <div class="regex-flags-row">
        <el-checkbox v-model="flagG" label="g" title="全局匹配" />
        <el-checkbox v-model="flagI" label="i" title="忽略大小写" />
        <el-checkbox v-model="flagM" label="m" title="多行模式" />
        <el-checkbox v-model="flagS" label="s" title="单行模式（. 匹配换行）" />
        <el-checkbox v-model="flagX" label="x" title="扩展模式（忽略空白）" />
      </div>
    </div>

    <!-- 匹配测试区 -->
    <div class="regex-test-section">
      <div class="regex-test-grid">
        <div class="textarea-wrap">
          <el-input
            v-model="input"
            type="textarea"
            :rows="6"
            placeholder="待匹配文本"
          />
        </div>
        <div class="textarea-wrap">
          <el-input
            v-model="replacement"
            type="textarea"
            :rows="6"
            placeholder="替换模式（支持 $1 $2 ${name}）"
          />
        </div>
      </div>
      <div class="regex-actions">
        <el-space>
          <el-button type="primary" @click="runRegexTest">执行匹配</el-button>
          <el-button @click="runReplace">执行替换</el-button>
          <el-button @click="clearAll">清空</el-button>
          <el-button @click="copyResult">复制结果</el-button>
        </el-space>
        <span v-if="matchCount >= 0" class="match-count">
          {{ matchCount }} 个匹配
        </span>
      </div>
    </div>

    <!-- 结果区 -->
    <div v-if="matchResults.length > 0 || replaceResult !== null" class="regex-result-section">
      <el-tabs v-model="activeResultTab">
        <el-tab-pane label="匹配列表" name="matches">
          <el-table :data="matchResults" border stripe max-height="300" size="small">
            <el-table-column label="#" width="50" align="center">
              <template #default="{ $index }">{{ $index + 1 }}</template>
            </el-table-column>
            <el-table-column label="匹配内容" prop="match" min-width="160" show-overflow-tooltip />
            <el-table-column label="位置" width="100" align="center">
              <template #default="{ row }">{{ row.index }}-{{ row.end }}</template>
            </el-table-column>
            <el-table-column label="捕获组" min-width="200" show-overflow-tooltip>
              <template #default="{ row }">
                <span v-if="row.groups && row.groups.length > 0">
                  <span v-for="(g, gi) in row.groups" :key="gi" class="capture-group">
                    <template v-if="g.value !== null">
                      <strong>{{ g.name || ('$' + g.index) }}</strong>: {{ g.value }}
                    </template>
                    <template v-else>
                      <strong>{{ g.name || ('$' + g.index) }}</strong>: (empty)
                    </template>
                    <span v-if="gi < row.groups.length - 1" class="group-sep"> | </span>
                  </span>
                </span>
                <span v-else class="text-muted">-</span>
              </template>
            </el-table-column>
          </el-table>
        </el-tab-pane>
        <el-tab-pane label="替换结果" name="replace">
          <el-input
            :model-value="replaceResult ?? ''"
            type="textarea"
            :rows="6"
            readonly
            placeholder="替换结果将显示在这里"
          />
        </el-tab-pane>
      </el-tabs>
    </div>

    <!-- 模板库 -->
    <div class="regex-templates-section">
      <div class="templates-header">
        <span class="section-title">模板库</span>
        <el-input
          v-model="searchQuery"
          placeholder="搜索模板名称或描述"
          clearable
          class="template-search"
          :prefix-icon="SearchIcon"
        />
      </div>
      <div class="category-tabs">
        <el-radio-group v-model="selectedCategory" size="small">
          <el-radio-button value="">全部</el-radio-button>
          <el-radio-button
            v-for="cat in categories"
            :key="cat.id"
            :value="cat.id"
          >{{ cat.name }}</el-radio-button>
        </el-radio-group>
      </div>
      <el-table
        :data="filteredTemplates"
        border
        stripe
        max-height="360"
        size="small"
        class="template-table"
      >
        <el-table-column label="名称" prop="name" width="160" show-overflow-tooltip />
        <el-table-column label="分类" width="100" align="center">
          <template #default="{ row }">{{ categoryName(row.category) }}</template>
        </el-table-column>
        <el-table-column label="说明" prop="description" min-width="200" show-overflow-tooltip />
        <el-table-column label="表达式" min-width="260" show-overflow-tooltip>
          <template #default="{ row }">
            <code class="expr-code">{{ row.expression }}</code>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="70" align="center" fixed="right">
          <template #default="{ row }">
            <el-button class="template-use-btn" type="primary" link size="small" @click="useTemplate(row)">使用</el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch, shallowRef, markRaw } from "vue";
import { ElMessage } from "element-plus";
import { Search } from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../bridge/tauri";
import type { RegexTemplate, RegexMatchResult } from "../types/regex";

const SearchIcon = shallowRef(markRaw(Search));

const pattern = ref("");
const flagG = ref(true);
const flagI = ref(false);
const flagM = ref(false);
const flagS = ref(false);
const flagX = ref(false);
const input = ref("");
const replacement = ref("");
const matchResults = ref<RegexMatchResult[]>([]);
const replaceResult = ref<string | null>(null);
const activeResultTab = ref("matches");
const matchCount = ref(-1);

const searchQuery = ref("");
const selectedCategory = ref("");
const allTemplates = ref<RegexTemplate[]>([]);

const flags = computed(() => {
  let f = "";
  if (flagG.value) f += "g";
  if (flagI.value) f += "i";
  if (flagM.value) f += "m";
  if (flagS.value) f += "s";
  if (flagX.value) f += "x";
  return f;
});

const CATEGORY_MAP: Record<string, string> = {
  common: "通用验证",
  network: "网络相关",
  china: "中国特色",
  datetime: "日期时间",
  programming: "编程语言",
  finance: "金融数字",
  address: "地址路径",
  data: "数据格式",
  password: "密码安全",
  text: "文本处理",
};

const categories = computed(() =>
  Object.entries(CATEGORY_MAP).map(([id, name]) => ({ id, name }))
);

function categoryName(id: string): string {
  return CATEGORY_MAP[id] || id;
}

const filteredTemplates = computed(() => {
  let list = allTemplates.value;
  if (selectedCategory.value) {
    list = list.filter((t) => t.category === selectedCategory.value);
  }
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.trim().toLowerCase();
    list = list.filter(
      (t) =>
        t.name.toLowerCase().includes(q) ||
        t.description.toLowerCase().includes(q)
    );
  }
  return list;
});

async function runRegexTest() {
  if (!pattern.value.trim()) {
    matchResults.value = [];
    matchCount.value = -1;
    return;
  }
  try {
    const data = await invokeToolByChannel("tool:regex:test", {
      pattern: pattern.value,
      flags: flags.value,
      input: input.value,
    });
    const results = Array.isArray(data) ? (data as RegexMatchResult[]) : [];
    matchResults.value = results;
    matchCount.value = results.length;
    activeResultTab.value = "matches";
  } catch (error) {
    ElMessage.error((error as Error).message);
    matchResults.value = [];
    matchCount.value = -1;
  }
}

async function runReplace() {
  if (!pattern.value.trim()) {
    ElMessage.warning("请输入正则表达式");
    return;
  }
  try {
    const data = await invokeToolByChannel("tool:regex:replace", {
      pattern: pattern.value,
      flags: flags.value,
      input: input.value,
      replacement: replacement.value,
    });
    replaceResult.value = typeof data === "string" ? data : JSON.stringify(data);
    activeResultTab.value = "replace";
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function clearAll() {
  pattern.value = "";
  input.value = "";
  replacement.value = "";
  matchResults.value = [];
  replaceResult.value = null;
  matchCount.value = -1;
}

function copyResult() {
  let text = "";
  if (activeResultTab.value === "replace" && replaceResult.value !== null) {
    text = replaceResult.value;
  } else if (matchResults.value.length > 0) {
    text = matchResults.value.map((r) => r.match).join("\n");
  }
  if (!text) {
    ElMessage.warning("没有可复制的结果");
    return;
  }
  navigator.clipboard.writeText(text).then(() => {
    ElMessage.success("已复制");
  });
}

function useTemplate(tpl: RegexTemplate) {
  pattern.value = tpl.expression;
  input.value = tpl.example_input;
  replacement.value = "";
  replaceResult.value = null;
  // Scroll to top
  const el = document.querySelector(".regex-panel");
  if (el) el.scrollIntoView({ behavior: "smooth" });
}

async function loadTemplates() {
  try {
    const data = await invokeToolByChannel("tool:regex:templates", {});
    allTemplates.value = Array.isArray(data) ? (data as RegexTemplate[]) : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

// Auto-test on input change with debounce
let timer: ReturnType<typeof setTimeout> | null = null;
watch([pattern, flags, input], () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    if (!pattern.value.trim() && !input.value.trim()) {
      matchResults.value = [];
      matchCount.value = -1;
      return;
    }
    runRegexTest();
  }, 300);
});

onMounted(() => loadTemplates());
</script>

<style scoped>
.regex-panel {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.regex-input-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.regex-input-row {
  display: flex;
  gap: 10px;
}

.regex-pattern-input {
  flex: 1;
}

.regex-flags-row {
  display: flex;
  gap: 16px;
  align-items: center;
}

.regex-test-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.regex-test-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
}

.regex-actions {
  display: flex;
  align-items: center;
  gap: 16px;
}

.match-count {
  color: var(--lc-accent-light);
  font-size: 13px;
  font-weight: 500;
}

.regex-result-section {
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  padding: 12px;
  background: var(--lc-surface-1);
}

.capture-group strong {
  color: var(--lc-accent-light);
}

.group-sep {
  color: var(--lc-text-muted);
  margin: 0 2px;
}

.text-muted {
  color: var(--lc-text-muted);
}

.regex-templates-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.templates-header {
  display: flex;
  align-items: center;
  gap: 14px;
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--lc-text);
  white-space: nowrap;
}

.template-search {
  max-width: 320px;
}

.category-tabs {
  overflow-x: auto;
}

.expr-code {
  font-family: var(--lc-font-mono, "Cascadia Code", "JetBrains Mono", monospace);
  font-size: 12px;
  color: var(--lc-accent-light);
  word-break: break-all;
}

.template-table :deep(.el-table__body-wrapper) {
  scrollbar-width: thin;
}

.template-use-btn {
  --el-button-text-color: var(--el-color-primary);
  --el-button-hover-text-color: var(--el-color-primary-light-3);
  --el-button-active-text-color: var(--el-color-primary-dark-2);
}
</style>

