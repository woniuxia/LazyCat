<template>
  <div class="spotlight" @keydown="onKeydown">
    <div class="spotlight-input-wrap">
      <input
        ref="inputRef"
        v-model="query"
        class="spotlight-input"
        placeholder="搜索工具：输入名称、拼音首字母、别名…"
        spellcheck="false"
        autocomplete="off"
      />
    </div>
    <div class="spotlight-results">
      <div v-if="results.length === 0" class="spotlight-empty">
        没有匹配的工具
      </div>
      <div
        v-for="(entry, idx) in results"
        :key="entry.tool.id"
        class="spotlight-row"
        :class="{ 'is-active': idx === activeIndex }"
        @mouseenter="activeIndex = idx"
        @click="commit(entry)"
      >
        <span class="spotlight-row-index">{{ idx + 1 }}</span>
        <div class="spotlight-row-main">
          <span class="spotlight-row-name">{{ entry.tool.name }}</span>
          <span class="spotlight-row-desc">{{ entry.tool.desc }}</span>
        </div>
        <div class="spotlight-row-tags">
          <span v-if="entry.isFavorite" class="spotlight-tag spotlight-tag-fav">收藏</span>
          <span v-else-if="entry.count > 0" class="spotlight-tag spotlight-tag-hot">高频</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useSpotlightCatalog, type SpotlightEntry } from "../composables/useSpotlightCatalog";

const RESULT_LIMIT = 8;

const { mergedHomeTools, searchTools, ensureLoaded, loadFromStorage } = useSpotlightCatalog();

const query = ref("");
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
let unlistenReset: UnlistenFn | null = null;

const results = computed<SpotlightEntry[]>(() => {
  if (!query.value.trim()) {
    return mergedHomeTools.value.slice(0, RESULT_LIMIT);
  }
  return searchTools(query.value, RESULT_LIMIT);
});

watch(results, () => {
  if (activeIndex.value >= results.value.length) {
    activeIndex.value = results.value.length > 0 ? 0 : 0;
  }
});

onMounted(async () => {
  await ensureLoaded();
  await nextTick();
  inputRef.value?.focus();
  inputRef.value?.select();

  try {
    unlistenReset = await listen("spotlight-reset", () => {
      query.value = "";
      activeIndex.value = 0;
      loadFromStorage();
      nextTick(() => {
        inputRef.value?.focus();
        inputRef.value?.select();
      });
    });
  } catch {
    /* ignore in non-Tauri env */
  }
});

onBeforeUnmount(() => {
  unlistenReset?.();
});

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    void closeWindow();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (results.value.length === 0) return;
    activeIndex.value = (activeIndex.value + 1) % results.value.length;
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    if (results.value.length === 0) return;
    activeIndex.value =
      (activeIndex.value - 1 + results.value.length) % results.value.length;
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    const entry = results.value[activeIndex.value];
    if (entry) void commit(entry);
    return;
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  if (e.key >= "1" && e.key <= "9") {
    const idx = parseInt(e.key, 10) - 1;
    const entry = results.value[idx];
    if (entry) {
      e.preventDefault();
      void commit(entry);
    }
  }
}

async function commit(entry: SpotlightEntry) {
  try {
    await invoke("spotlight_pick", { target: entry.tool.id });
  } catch (err) {
    console.error("[Spotlight] 跳转失败:", err);
    await closeWindow();
  }
}

async function closeWindow() {
  try {
    await invoke("spotlight_close");
  } catch {
    /* ignore in non-Tauri env */
  }
}
</script>

<style scoped>
.spotlight {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  background: #ffffff;
  border-radius: 12px;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.18);
  overflow: hidden;
  font-family: inherit;
}

.spotlight-input-wrap {
  flex-shrink: 0;
  padding: 0 20px;
  height: 64px;
  display: flex;
  align-items: center;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
}

.spotlight-input {
  width: 100%;
  height: 100%;
  border: none;
  outline: none;
  font-size: 16px;
  color: #303133;
  background: transparent;
}

.spotlight-input::placeholder {
  color: #c0c4cc;
}

.spotlight-results {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.spotlight-empty {
  padding: 24px 20px;
  color: #909399;
  font-size: 13px;
  text-align: center;
}

.spotlight-row {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 56px;
  padding: 0 20px;
  cursor: pointer;
  transition: background-color 0.12s ease;
}

.spotlight-row.is-active {
  background: #f3f6fb;
}

.spotlight-row-index {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  background: #f0f2f5;
  color: #606266;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-variant-numeric: tabular-nums;
}

.spotlight-row.is-active .spotlight-row-index {
  background: #409eff;
  color: #fff;
}

.spotlight-row-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.spotlight-row-name {
  font-size: 14px;
  color: #303133;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-row-desc {
  font-size: 12px;
  color: #909399;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-row-tags {
  flex-shrink: 0;
  display: flex;
  gap: 6px;
}

.spotlight-tag {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 999px;
  line-height: 1.4;
}

.spotlight-tag-fav {
  background: rgba(245, 158, 11, 0.12);
  color: #b45309;
}

.spotlight-tag-hot {
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
}
</style>
