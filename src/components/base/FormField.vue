<template>
  <label class="ff">
    <span class="ff-label">{{ label }}</span>
    <div class="ff-body">
      <slot />
    </div>
    <small v-if="hint" :class="['ff-hint', hintType]">{{ hint }}</small>
  </label>
</template>

<script setup>
defineProps({
  label: { type: String, default: '' },
  hint: { type: String, default: '' },
  hintType: { type: String, default: '' },
})
</script>

<style scoped>
.ff { display: block; margin-top: 14px; }
.ff-label { display: block; color: var(--muted); font-size: 12px; margin-bottom: 6px; }
.ff-body { display: flex; flex-direction: column; gap: 0; }
.ff-body > :deep(.input) { flex: 1; }
.ff-row { display: flex; gap: 10px; align-items: center; }
.ff-row > :deep(.input) { flex: 1; }
.ff-row > :deep(.btn) { flex-shrink: 0; }
.ff-hint { display: block; margin-top: 6px; color: var(--muted); font-size: 11px; min-height: 14px; }
.ff-hint.ok { color: var(--green); }
.ff-hint.err { color: #ff6b6b; }
.ff-center { display: flex; justify-content: center; margin-top: 14px; }
</style>
