<template>
  <button
    :class="['toggle', { 'toggle--on': modelValue }]"
    @click="$emit('update:modelValue', !modelValue)"
  >
    <span class="toggle-track">
      <span class="toggle-thumb" />
    </span>
    <span class="toggle-label">{{ modelValue ? labelOn : labelOff }}</span>
  </button>
</template>

<script setup>
defineProps({
  modelValue: { type: Boolean, default: false },
  labelOn: { type: String, default: '已开启' },
  labelOff: { type: String, default: '已关闭' },
})
defineEmits(['update:modelValue'])
</script>

<style scoped>
.toggle {
  display: inline-flex; align-items: center; gap: 8px;
  background: none; border: none; cursor: pointer; padding: 0;
  font-size: 13px; color: var(--muted);
}
.toggle-track {
  display: inline-block; width: 36px; height: 20px;
  background: var(--border); border-radius: 10px;
  position: relative; transition: background .2s;
}
.toggle-thumb {
  position: absolute; top: 2px; left: 2px;
  width: 16px; height: 16px; border-radius: 50%;
  background: var(--muted); transition: transform .2s, background .2s;
}
.toggle--on .toggle-track { background: var(--blue); }
.toggle--on .toggle-thumb { transform: translateX(16px); background: #fff; }
.toggle--on .toggle-label { color: var(--blue); }
</style>
