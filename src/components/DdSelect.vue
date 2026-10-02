<template>
  <div class="dd" :class="{ open: isOpen }" ref="ddRef">
    <button type="button" class="dd-btn" @click.stop="toggle">
      <span class="dd-label">{{ selectedLabel }}</span>
      <svg class="dd-chevron" width="12" height="12" viewBox="0 0 24 24" fill="none"
        stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="m6 9 6 6 6-6" />
      </svg>
    </button>
    <div class="dd-list">
      <div
        v-for="opt in options"
        :key="opt.v"
        class="dd-item"
        :class="{ selected: opt.v === modelValue }"
        @click.stop="select(opt.v)"
      >
        <span>{{ opt.label }}</span>
        <svg v-if="opt.v === modelValue" width="13" height="13" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20 6 9 17l-5-5" />
        </svg>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'

const props = defineProps({
  options: { type: Array, required: true },
  modelValue: { type: String, default: '' },
  block: { type: Boolean, default: false },
})

const emit = defineEmits(['update:modelValue'])

const isOpen = ref(false)
const ddRef = ref(null)

const selectedLabel = computed(() => {
  const found = props.options.find(o => o.v === props.modelValue)
  return found ? found.label : ''
})

function toggle() { isOpen.value = !isOpen.value }

function select(v) {
  emit('update:modelValue', v)
  isOpen.value = false
}

function closeAll(e) {
  if (ddRef.value && !ddRef.value.contains(e.target)) {
    isOpen.value = false
  }
}

function onKeydown(e) {
  if (e.key === 'Escape') isOpen.value = false
}

onMounted(() => {
  document.addEventListener('click', closeAll)
  document.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  document.removeEventListener('click', closeAll)
  document.removeEventListener('keydown', onKeydown)
})
</script>

<style scoped>
.dd { position: relative; user-select: none; }
.dd-block { width: 100%; }
.dd-btn {
  display: flex; align-items: center; justify-content: space-between;
  gap: 8px; width: 100%; min-width: 110px;
  background: var(--border); color: var(--text);
  border: 1px solid transparent; border-radius: 6px;
  padding: 6px 12px; font-size: 13px; cursor: pointer;
}
.dd-btn:hover { filter: brightness(1.2); }
.dd.open .dd-btn { border-color: var(--blue); filter: none; }
.dd-chevron { transition: transform .15s ease; color: var(--muted); flex-shrink: 0; }
.dd.open .dd-chevron { transform: rotate(180deg); }
.dd-list {
  position: absolute; top: calc(100% + 4px); right: 0;
  min-width: 100%; max-height: 260px; overflow-y: auto;
  background: var(--panel);
  border: 1px solid var(--border); border-radius: 8px;
  padding: 4px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, .3);
  opacity: 0; transform: translateY(-4px); pointer-events: none;
  transition: opacity .12s ease, transform .12s ease;
  z-index: 60;
}
.dd.open .dd-list { opacity: 1; transform: translateY(0); pointer-events: auto; }
.dd-item {
  padding: 6px 10px; border-radius: 6px;
  font-size: 13px; color: var(--muted); cursor: pointer; white-space: nowrap;
  display: flex; align-items: center; justify-content: space-between; gap: 12px;
}
.dd-item:hover { background: var(--border); color: var(--text); }
.dd-item.selected { color: var(--blue); font-weight: 600; background: var(--border); }
</style>
