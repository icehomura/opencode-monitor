<template>
  <div :class="['input-wrap', { 'input-wrap--spinner': spinner }]">
    <template v-if="spinner">
      <button class="input-spin-btn" @click="dec" :disabled="atMin">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><path d="M5 12h14"/></svg>
      </button>
    </template>
    <input
      ref="el"
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :min="min"
      :max="max"
      :class="['input', { 'input--spinner': spinner }]"
      @input="onInput"
    />
    <template v-if="spinner">
      <button class="input-spin-btn" @click="inc" :disabled="atMax">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
      </button>
    </template>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  modelValue: { type: [String, Number], default: '' },
  type: { type: String, default: 'text' },
  placeholder: { type: String, default: '' },
  min: { type: Number, default: undefined },
  max: { type: Number, default: undefined },
  block: { type: Boolean, default: false },
  spinner: { type: Boolean, default: false },
  step: { type: Number, default: 1 },
})

const emit = defineEmits(['update:modelValue'])

const atMin = computed(() => props.min != null && Number(props.modelValue) <= props.min)
const atMax = computed(() => props.max != null && Number(props.modelValue) >= props.max)

function clamp(v) {
  let n = Number(v)
  if (props.min != null) n = Math.max(props.min, n)
  if (props.max != null) n = Math.min(props.max, n)
  return n
}

function onInput(e) {
  const raw = e.target.value
  if (props.type !== 'number') {
    emit('update:modelValue', raw)
    return
  }
  if (raw === '' || raw === '-') {
    emit('update:modelValue', raw)
    return
  }
  emit('update:modelValue', clamp(raw))
}

function inc() {
  emit('update:modelValue', clamp(Number(props.modelValue) + props.step))
}

function dec() {
  emit('update:modelValue', clamp(Number(props.modelValue) - props.step))
}
</script>

<style scoped>
.input {
  background: var(--bg); color: var(--text);
  border: 1px solid var(--border); border-radius: 6px;
  padding: 7px 10px; font-size: 13px; outline: none;
  width: 100%; min-width: 0;
}
.input:focus { border-color: var(--blue); }
.input::placeholder { color: var(--muted); opacity: .6; }
.input::-webkit-outer-spin-button,
.input::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }
.input[type='number'] { -moz-appearance: textfield; }

.input-wrap { display: flex; align-items: center; width: 100%; }
.input-wrap--spinner .input { text-align: center; border-radius: 0; flex: 1; min-width: 0; }
.input-spin-btn {
  display: flex; align-items: center; justify-content: center;
  width: 32px; height: 32px; flex-shrink: 0;
  background: var(--border); color: var(--muted);
  border: 1px solid var(--border); cursor: pointer;
  transition: background .1s, color .1s;
}
.input-spin-btn:first-child { border-radius: 6px 0 0 6px; border-right: none; }
.input-spin-btn:last-child { border-radius: 0 6px 6px 0; }
.input-spin-btn:hover:not(:disabled) { background: var(--bg); color: var(--text); }
.input-spin-btn:disabled { opacity: .3; cursor: not-allowed; }
</style>
