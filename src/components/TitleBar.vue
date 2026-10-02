<template>
  <div class="titlebar" :class="{ 'titlebar--mac': isMac }" ref="titlebarRef" @mousedown="tryDrag" @dblclick="onDblClick">
    <div class="titlebar-brand">
      <strong>OpenCode Monitor</strong>
      <template v-if="quota">
        <span class="plan-badge">{{ quota.plan_name }}</span>
        <span class="plan-expiry">到期 {{ fmtDate(quota.ends_at) }}</span>
        <span v-if="quota.cancel_at_period_end" class="plan-warn">到期后不再续费</span>
      </template>
      <span class="plan-badge muted" v-else>未配置</span>
    </div>
    <div class="titlebar-actions">
      <IconButton title="设置" @click="$emit('open-settings')">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      </IconButton>
      <IconButton title="窗口置顶" :active="pinned" @click="togglePin">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 17v5" />
          <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1z" />
        </svg>
      </IconButton>
      <IconButton title="最小化" @click="win?.minimize()" v-if="!isMac">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M5 12h14" />
        </svg>
      </IconButton>
      <IconButton title="最大化" @click="toggleMaximize" v-if="!isMac">
        <svg v-show="!isMaximized" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round">
          <rect x="5" y="5" width="14" height="14" rx="1.5" />
        </svg>
        <svg v-show="isMaximized" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round">
          <rect x="8" y="8" width="11" height="11" rx="1.5" />
          <path d="M5 16V6.5A1.5 1.5 0 0 1 6.5 5H16" />
        </svg>
      </IconButton>
      <IconButton title="隐藏到托盘" class="ibtn--close" @click="win?.close()" v-if="!isMac">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M18 6 6 18M6 6l12 12" />
        </svg>
      </IconButton>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useTauri } from '../composables/useTauri'
import { fmtDate } from '../utils/format'
import IconButton from './base/IconButton.vue'

defineProps({
  quota: { type: Object, default: null },
  configured: { type: Boolean, default: false },
})
defineEmits(['open-settings'])

const { getCurrentWindow } = useTauri()
const win = getCurrentWindow()
const pinned = ref(false)
const isMaximized = ref(false)
const titlebarRef = ref(null)
const isMac = /mac|darwin/i.test(navigator.userAgent)

function isInteractive(el) {
  return el?.closest('button, select, input, a, .dd')
}

function tryDrag(e) {
  if (e.button !== 0 || isInteractive(e.target)) return
  e.preventDefault()
  try { win?.startDragging() } catch {}
}

function onDblClick(e) {
  if (isInteractive(e.target)) return
  toggleMaximize()
}

async function toggleMaximize() {
  try {
    const maximized = await win?.isMaximized?.()
    if (maximized) await win?.unmaximize?.()
    else await win?.maximize?.()
  } catch {}
  setTimeout(syncMaxIcon, 150)
}

async function togglePin() {
  pinned.value = !pinned.value
  try { await win?.setAlwaysOnTop(pinned.value) } catch {}
}

async function syncMaxIcon() {
  try { isMaximized.value = await win?.isMaximized() } catch {}
}

onMounted(() => {
  syncMaxIcon()
  win?.onResized?.(() => syncMaxIcon())
})
</script>

<style scoped>
.titlebar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 40px;
  padding-left: 14px;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
  user-select: none;
  flex-shrink: 0;
}
.titlebar-brand { display: flex; align-items: center; gap: 10px; }
.titlebar-brand strong { font-size: 14px; font-weight: 600; color: var(--text); }
.plan-badge {
  font-size: 11px; color: var(--green); background: var(--border);
  padding: 1px 8px; border-radius: 10px; line-height: 1.6; white-space: nowrap;
}
.plan-badge.muted { color: var(--muted); }
.plan-expiry { font-size: 11px; color: var(--muted); white-space: nowrap; }
.plan-warn {
  font-size: 11px; color: #f0a020; white-space: nowrap;
  background: rgba(240,160,32,.12); border: 1px solid rgba(240,160,32,.35);
  border-radius: 10px; padding: 1px 8px;
}
.titlebar-actions { display: flex; height: 100%; }
</style>
