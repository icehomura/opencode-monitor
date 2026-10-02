<template>
  <button
    :class="['btn', `btn--${variant}`, { 'btn--block': block }]"
    :disabled="disabled"
    @click="$emit('click', $event)"
  >
    <slot />
  </button>
</template>

<script setup>
defineProps({
  variant: { type: String, default: 'default' },
  block: { type: Boolean, default: false },
  disabled: { type: Boolean, default: false },
})
defineEmits(['click'])
</script>

<style scoped>
.btn {
  display: inline-flex; align-items: center; justify-content: center;
  background: var(--border); color: var(--text);
  border: 1px solid transparent; border-radius: 6px;
  padding: 6px 14px; font-size: 13px; cursor: pointer;
  flex-shrink: 0; white-space: nowrap;
  transition: filter .1s;
}
.btn:hover { filter: brightness(1.2); }
.btn:disabled { opacity: .4; cursor: not-allowed; }
.btn--primary { background: var(--blue); color: #fff; }
.btn--primary:hover { filter: brightness(1.15); }
.btn--danger { background: #e81123; color: #fff; }
.btn--danger:hover { filter: brightness(1.15); }
.btn--block { width: 100%; }
</style>
