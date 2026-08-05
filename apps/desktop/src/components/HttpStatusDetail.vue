<template>
  <div class="status-detail" @click.stop>
    <section v-if="status.explanation" class="detail-section detail-section--wide">
      <h4>语义解释</h4>
      <p>{{ status.explanation }}</p>
    </section>

    <section v-if="status.usage" class="detail-section">
      <h4>常见场景</h4>
      <p>{{ status.usage }}</p>
    </section>

    <section v-if="status.causes" class="detail-section">
      <h4>常见原因</h4>
      <ul>
        <li v-for="cause in splitItems(status.causes)" :key="cause">{{ cause }}</li>
      </ul>
    </section>

    <section v-if="status.troubleshooting" class="detail-section detail-section--wide">
      <h4>排查建议</h4>
      <ul>
        <li v-for="item in splitItems(status.troubleshooting)" :key="item">{{ item }}</li>
      </ul>
    </section>

    <section v-if="status.responseHeaders.length" class="detail-section detail-section--wide">
      <h4>相关响应头</h4>
      <dl class="header-list">
        <div v-for="header in status.responseHeaders" :key="header.name" class="header-item">
          <dt>
            <code>{{ header.name }}</code>
          </dt>
          <dd>{{ header.description }}</dd>
        </div>
      </dl>
    </section>
  </div>
</template>

<script setup lang="ts">
import type { HttpStatusCode } from "../types/httpStatus";

defineProps<{ status: HttpStatusCode }>();

function splitItems(value: string): string[] {
  return value
    .split("; ")
    .map((item) => item.trim())
    .filter(Boolean);
}
</script>

<style scoped>
.status-detail {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px 18px;
  padding: 12px 20px 14px 52px;
  background: var(--el-fill-color-lighter);
  color: var(--el-text-color-regular);
  line-height: 1.55;
}

.detail-section {
  min-width: 0;
}

.detail-section--wide {
  grid-column: 1 / -1;
}

.detail-section h4 {
  margin: 0 0 4px;
  color: var(--el-text-color-primary);
  font-size: 12px;
  font-weight: 600;
}

.detail-section p,
.detail-section ul {
  margin: 0;
}

.detail-section ul {
  padding-left: 18px;
}

.detail-section li + li {
  margin-top: 2px;
}

.header-list {
  display: grid;
  gap: 4px;
  margin: 0;
}

.header-item {
  display: grid;
  grid-template-columns: minmax(150px, 0.3fr) minmax(0, 1fr);
  gap: 10px;
  align-items: baseline;
}

.header-item dt,
.header-item dd {
  margin: 0;
}

.header-item code {
  color: var(--el-color-primary);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
}

@media (max-width: 760px) {
  .status-detail {
    grid-template-columns: 1fr;
    padding-left: 22px;
  }

  .detail-section--wide {
    grid-column: auto;
  }

  .header-item {
    grid-template-columns: 1fr;
    gap: 0;
  }
}
</style>
