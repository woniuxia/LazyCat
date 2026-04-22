<template>
  <Teleport to="body">
    <div class="rte-image-preview" @click="close" @wheel.prevent="onWheel">
      <div class="rte-image-preview__backdrop" />
      <img
        :src="src"
        class="rte-image-preview__img"
        :style="imgStyle"
        @click.stop
        @mousedown.prevent="onDragStart"
        @dragstart.prevent
      />
      <span class="rte-image-preview__close" @click.stop="close">&times;</span>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';

defineProps<{ src: string }>();
const emit = defineEmits<{ (e: 'close'): void }>();

const ZOOM_STEP = 0.15;
const MAX_SCALE = 8;

const scale = ref(1);
const ox = ref(0);
const oy = ref(0);
const dragging = ref(false);
const anchor = ref({ mx: 0, my: 0, ox: 0, oy: 0 });

const imgStyle = computed(() => ({
  transform: `translate(${ox.value}px, ${oy.value}px) scale(${scale.value})`,
  cursor: scale.value > 1 ? (dragging.value ? 'grabbing' : 'grab') : 'default',
  transition: dragging.value ? 'none' : 'transform 0.2s ease',
}));

function onWheel(e: WheelEvent) {
  const d = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
  scale.value = Math.max(1, Math.min(MAX_SCALE, scale.value + d));
  if (scale.value <= 1) {
    ox.value = 0;
    oy.value = 0;
  }
}

function onDragStart(e: MouseEvent) {
  if (scale.value <= 1) return;
  dragging.value = true;
  anchor.value = { mx: e.clientX, my: e.clientY, ox: ox.value, oy: oy.value };
}

function onMouseMove(e: MouseEvent) {
  if (!dragging.value) return;
  ox.value = anchor.value.ox + (e.clientX - anchor.value.mx);
  oy.value = anchor.value.oy + (e.clientY - anchor.value.my);
}

function onMouseUp() {
  dragging.value = false;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') close();
}

function close() {
  emit('close');
}

onMounted(() => {
  document.addEventListener('keydown', onKeydown);
  document.addEventListener('mousemove', onMouseMove);
  document.addEventListener('mouseup', onMouseUp);
});

onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown);
  document.removeEventListener('mousemove', onMouseMove);
  document.removeEventListener('mouseup', onMouseUp);
});
</script>

<style scoped>
.rte-image-preview {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  user-select: none;
  animation: rte-preview-in 0.15s ease-out;
}

@keyframes rte-preview-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

.rte-image-preview__backdrop {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.72);
}

.rte-image-preview__img {
  position: relative;
  max-width: 90vw;
  max-height: 90vh;
  object-fit: contain;
  border-radius: 4px;
  user-select: none;
  -webkit-user-drag: none;
}

.rte-image-preview__close {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  line-height: 1;
  color: rgba(255, 255, 255, 0.85);
  background: rgba(0, 0, 0, 0.45);
  border-radius: 50%;
  cursor: pointer;
  transition: background 0.15s;
}

.rte-image-preview__close:hover {
  background: rgba(0, 0, 0, 0.7);
}
</style>
