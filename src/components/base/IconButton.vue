<template>
  <button
    :class="['ibtn', { 'ibtn--active': active }]"
    :title="title"
    @click="$emit('click', $event)"
  >
    <slot />
  </button>
</template>

<script setup>
defineProps({
  title: { type: String, default: '' },
  active: { type: Boolean, default: false },
})
defineEmits(['click'])
</script>

<style scoped>
.ibtn {
  display: grid; place-items: center;
  width: 44px; height: 100%;
  border: 0; background: transparent;
  color: var(--muted); cursor: pointer;
  transition: background .1s, color .1s;
}
.ibtn:hover { background: var(--border); color: var(--text); }
.ibtn--active { color: var(--blue); background: var(--border); }
.ibtn--close:hover { background: #e81123 !important; color: #fff !important; }
</style>
