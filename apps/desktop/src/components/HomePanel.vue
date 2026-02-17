<template>
  <div class="home-panel">
    <section class="home-section">
      <div class="home-section-header">
        <h2>收藏页面</h2>
        <el-text type="info">优先显示你常用的工具页面</el-text>
      </div>
      <div v-if="favoriteTools.length" class="home-card-grid">
        <div
          v-for="tool in favoriteTools"
          :key="tool.id"
          class="home-tool-card"
          tabindex="0"
          @click="emit('openTool', tool.id)"
          @keyup.enter="emit('openTool', tool.id)"
        >
          <el-button class="home-tool-card-action" text type="warning" @click.stop="emit('toggleFavorite', tool.id)">
            取消收藏
          </el-button>
          <div class="home-tool-card-title">{{ tool.name }}</div>
          <div class="home-tool-card-desc">{{ tool.desc }}</div>
        </div>
      </div>
      <el-empty v-else description="暂无收藏，进入工具页面后点击右上角“收藏”" />
    </section>

    <section class="home-section">
      <div class="home-section-header">
        <h2>最近一个月高频页面</h2>
        <div class="home-section-header-right">
          <el-text type="info">按点击次数排序，已排除收藏区页面</el-text>
          <el-radio-group v-model="homeTopLimitModel" size="small">
            <el-radio-button :value="6">Top 6</el-radio-button>
            <el-radio-button :value="12">Top 12</el-radio-button>
          </el-radio-group>
        </div>
      </div>
      <div v-if="topMonthlyTools.length" class="home-card-grid">
        <div
          v-for="item in topMonthlyTools"
          :key="item.tool.id"
          class="home-tool-card"
          tabindex="0"
          @click="emit('openTool', item.tool.id)"
          @keyup.enter="emit('openTool', item.tool.id)"
        >
          <el-button
            class="home-tool-card-action"
            text
            :type="isFavorite(item.tool.id) ? 'warning' : 'primary'"
            @click.stop="emit('toggleFavorite', item.tool.id)"
          >
            {{ isFavorite(item.tool.id) ? "取消收藏" : "收藏" }}
          </el-button>
          <div class="home-tool-card-title">{{ item.tool.name }}</div>
          <div class="home-tool-card-desc">{{ item.tool.desc }}</div>
          <div class="home-tool-card-meta">最近30天点击 {{ item.count }} 次</div>
        </div>
      </div>
      <el-empty v-else description="暂无使用记录，先去使用几个工具吧" />
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

interface ToolCard {
  id: string;
  name: string;
  desc: string;
}

interface TopMonthlyItem {
  tool: ToolCard;
  count: number;
}

const props = defineProps<{
  favoriteTools: ToolCard[];
  topMonthlyTools: TopMonthlyItem[];
  homeTopLimit: 6 | 12;
  isFavorite: (id: string) => boolean;
}>();

const emit = defineEmits<{
  (event: "openTool", id: string): void;
  (event: "toggleFavorite", id: string): void;
  (event: "update:homeTopLimit", value: 6 | 12): void;
}>();

const homeTopLimitModel = computed({
  get: () => props.homeTopLimit,
  set: (value) => emit("update:homeTopLimit", value)
});
</script>
