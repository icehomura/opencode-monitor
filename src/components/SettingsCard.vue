<template>
  <section class="settings-card" :class="[`settings-card--${modifier}`, { 'settings-card--stretch': stretch }, { 'settings-card--auto': auto }]">
    <div class="card-label-wrap">
      <div class="card-label-row">
        <div>
          <h3 class="card-title">{{ title }}</h3>
          <p v-if="description" class="card-description">{{ description }}</p>
        </div>
        <div v-if="$slots.actions" class="card-actions">
          <slot name="actions" />
        </div>
      </div>
    </div>
    <div class="card-content">
      <div class="card-main">
        <slot />
      </div>
      <div v-if="$slots.hint" class="card-hint">
        <slot name="hint" />
      </div>
    </div>
  </section>
</template>

<script setup>
defineProps({
  title: { type: String, required: true },
  description: { type: String, default: '' },
  modifier: { type: String, default: '' },
  stretch: { type: Boolean, default: false },
  auto: { type: Boolean, default: false },
})
</script>

<style scoped>
.settings-card {
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  height: auto;
  min-height: 0;
  box-sizing: border-box;
  min-width: 0;
  overflow: visible;
}
.settings-card--auto {
  height: auto;
  max-height: none;
}
.card-label-wrap {
  flex-shrink: 0;
  margin-bottom: 6px;
}
.card-label-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}
.card-actions {
  flex-shrink: 0;
  display: flex;
  align-items: flex-start;
}
.card-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  line-height: 1.4;
}
.card-description {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--muted);
  line-height: 1.4;
}
.card-content {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.card-main {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  gap: 6px;
  overflow: visible;
}
.card-main > * {
  flex-shrink: 0;
}
.settings-card--auto .card-main {
  justify-content: flex-start;
  overflow: visible;
  scrollbar-gutter: auto;
}
.card-hint {
  flex-shrink: 0;
  margin-top: 6px;
}
.settings-card--stretch .card-main {
  justify-content: stretch;
}
.card-main > :deep(.toggle-wrap),
.card-main > :deep(.port-row),
.card-main > :deep(.theme-row),
.card-main > :deep(.input-wrap),
.card-main > :deep(.dd) {
  width: 100%;
}
.card-main > :deep(.input-wrap),
.card-main > :deep(.port-row),
.card-main > :deep(.status-row) {
  justify-self: center;
  align-self: center;
}

/* 单位转换：开关所在内容区保持水平居中 */
.settings-card--unit .card-main {
  align-items: center;
}
</style>