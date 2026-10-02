<template>
  <div class="titlebar" :class="{ 'titlebar--mac': isMac }" ref="titlebarRef" @mousedown="tryDrag" @dblclick="onDblClick">
    <div class="titlebar-brand">
      <strong>OpenCode Monitor</strong>
      <template v-if="quota">
        <span class="plan-badge">{{ quota.plan_name }}</span>
        <span class="plan-expiry">到期 {{ fmtDate(quota.ends_at) }}</span>
        <span v-if="quota.cancel_at_period_end" class="plan-warn">到期后不自动续费</span>
        <span v-else class="plan-renew">到期后自动续费</span>
        <span v-if="estimate" class="plan-estimate" :title="estimateTitle">预估可用 {{ estimate }}</span>
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
import { ref, computed, onMounted } from 'vue'
import { useTauri } from '../composables/useTauri'
import { fmtDate } from '../utils/format'
import IconButton from './base/IconButton.vue'

const props = defineProps({
  quota: { type: Object, default: null },
})
defineEmits(['open-settings'])

// 标题栏的计划 / 到期 / 预估可用时长只来自主账号

// ── 预估可用时长 ──
// 速率直接来自额度计量：某窗口的 已用 microCents / 该窗口已过秒数。
// /request-logs 的 cost 恒为 0，不能用来算速率。
function windowRate(m, nowMs) {
  if (!m || !m.starts_at) return 0
  const used = Number(m.used_micro_cents || 0)
  const elapsed = (nowMs - new Date(m.starts_at).getTime()) / 1000
  if (used <= 0 || elapsed < 60) return 0
  return used / elapsed
}

function burnRate() {
  const q = props.quota
  if (!q) return 0
  const now = Date.now()
  // 优先 5 小时窗口（响应最新消耗），再回退到周 / 月
  return windowRate(q.five_hour, now) || windowRate(q.week, now) || windowRate(q.month, now) || 0
}

function fmtDuration(sec) {
  sec = Math.max(0, Math.floor(sec))
  const d = Math.floor(sec / 86400)
  const h = Math.floor((sec % 86400) / 3600)
  const m = Math.floor((sec % 3600) / 60)
  if (d > 0) return `${d}天${h}小时${m}分`
  if (h > 0) return `${h}小时${m}分`
  if (m > 0) return `${m}分`
  return `${sec}秒`
}

const estimate = computed(() => estimateInfo().text)
const estimateTitle = computed(() => estimateInfo().title)

function estimateInfo() {
  const q = props.quota
  if (!q) return { text: '', title: '' }
  const rate = burnRate()
  if (rate <= 0) return { text: '--', title: '额度窗口尚未开始消耗，无法估算' }
  const now = Date.now()
  const names = { five_hour: '5小时额度', week: '周额度', month: '月额度' }
  const cands = []
  for (const key of ['five_hour', 'week', 'month']) {
    const m = q[key]
    if (!m) continue
    const remaining = Number(m.limit_micro_cents || 0) - Number(m.used_micro_cents || 0)
    if (remaining <= 0) {
      cands.push({ sec: 0, why: `${names[key]}已用尽` })
      continue
    }
    const tExhaust = remaining / rate
    const tReset = m.resets_at ? Math.max(0, (new Date(m.resets_at).getTime() - now) / 1000) : Infinity
    // 重置前就能耗尽才算被它卡住；否则会先重置回血
    if (tExhaust <= tReset) cands.push({ sec: tExhaust, why: `受${names[key]}限制` })
  }
  if (q.cancel_at_period_end && q.ends_at) {
    cands.push({ sec: Math.max(0, (new Date(q.ends_at).getTime() - now) / 1000), why: '订阅到期' })
  }
  if (!cands.length) return { text: '充足', title: '按当前速率，额度会先重置，不会被耗尽' }
  cands.sort((a, b) => a.sec - b.sec)
  return { text: fmtDuration(cands[0].sec), title: `${cands[0].why}（按当前窗口平均速率估算）` }
}

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
/* 标题栏的胶囊统一形态：无描边、同字号/内边距/圆角（与「计划」胶囊一致），只靠文字色与底色区分状态 */
.plan-badge, .plan-warn, .plan-renew, .plan-estimate {
  font-size: 11px; white-space: nowrap; border: none;
  padding: 1px 8px; border-radius: 10px; line-height: 1.6;
}
.plan-badge { color: var(--green); background: var(--border); }
.plan-badge.muted { color: var(--muted); }
.plan-expiry { font-size: 11px; color: var(--muted); white-space: nowrap; }
.plan-warn { color: #f0a020; background: rgba(240, 160, 32, .14); }
.plan-renew { color: var(--green); background: rgba(53, 208, 165, .14); }
.plan-estimate { color: var(--blue); background: rgba(79, 140, 255, .14); }
.titlebar-actions { display: flex; height: 100%; }
</style>
