<template>
  <header class="toolbar" ref="toolbarRef" @mousedown="tryDrag" @dblclick="onDblClick">
    <span class="status-dot" :class="{ ok: login.logged_in, syncing: sync.syncing }"></span>
    <span class="sync-text" :title="sync.last_error || ''">
      {{ statusText }}
    </span>
    <span class="login-badge" :class="{ on: login.logged_in }" :title="login.logged_in ? ('已授权 · ' + (login.org_id || '')) : '未授权，点击设置→账户登录'" @click="$emit('open-login')">
      {{ login.logged_in ? '已授权' : '未授权' }}
    </span>
    <span v-if="sync.last_error" class="err-badge" :title="sync.last_error">⚠ 同步异常</span>

    <div class="toolbar-right">
      <span v-if="activeRangeText" class="active-range" title="点击编辑自定义范围" @click="editRange">
        {{ activeRangeText }}
      </span>
      <IconButton v-if="activeRangeText" class="range-edit-btn" title="编辑自定义范围" @click="editRange">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
          <path d="m18.5 2.5 3 3L12 15l-4 1 1-4Z" />
        </svg>
      </IconButton>
      <span class="local-text">新增 {{ sync.last_added || 0 }} 条</span>
      <span class="local-text">本地 {{ sync.local_rows || 0 }} 条</span>
      <BaseButton @click="doSync()" :disabled="!login.logged_in || sync.syncing">
        {{ sync.syncing ? '同步中…' : '立即同步' }}
      </BaseButton>
      <DdSelect :options="rangeOptions" :modelValue="selectedRange" @update:modelValue="onRangeSelect" />
    </div>
    <DatePickerModal
      :visible="showDatePicker"
      :from-ms="confirmedRange ? confirmedRange.startMs : 0"
      :to-ms="confirmedRange ? confirmedRange.endMs : 0"
      @close="onDatePickerClose"
      @confirm="onDatePickerConfirm"
    />
  </header>
</template>

<script setup>
import { ref, computed, watch, onMounted } from 'vue'
import DdSelect from './DdSelect.vue'
import BaseButton from './base/BaseButton.vue'
import IconButton from './base/IconButton.vue'
import DatePickerModal from './DatePickerModal.vue'
import { useTauri } from '../composables/useTauri'
import { fmtAgo, fmtClock } from '../utils/format'

const props = defineProps({
  sync: { type: Object, default: () => ({}) },
  range: { type: String, default: '1h' },
  rangeFrom: { type: Number, default: 0 },
  rangeTo: { type: Number, default: 0 },
  login: { type: Object, default: () => ({ logged_in: false, org_id: '' }) },
})
const emit = defineEmits(['range-change', 'sync-done', 'open-login'])

const { invoke, getCurrentWindow } = useTauri()
const win = getCurrentWindow()
const toolbarRef = ref(null)
const msg = ref('')

const rangeOptions = [
  { v: '1h', label: '近1小时' },
  { v: '5h', label: '当前5小时' },
  { v: 'today', label: '今日' },
  { v: 'week', label: '本周' },
  { v: 'month', label: '本月' },
  { v: 'all', label: '全部' },
  { v: 'custom', label: '自定义时间范围' },
]

// 当前已确认的自定义区间（来自父级 stats，重启后也能恢复徽章）
const confirmedRange = computed(() => (
  props.range === 'custom' && props.rangeFrom > 0 && props.rangeTo > props.rangeFrom
    ? { startMs: props.rangeFrom, endMs: props.rangeTo }
    : null
))

const activeRangeText = computed(() => {
  if (!confirmedRange.value) return ''
  const fmt = (ms) => {
    const d = new Date(ms)
    const M = String(d.getMonth() + 1).padStart(2, '0')
    const D = String(d.getDate()).padStart(2, '0')
    const h = String(d.getHours()).padStart(2, '0')
    const m = String(d.getMinutes()).padStart(2, '0')
    return `${M}-${D} ${h}:${m}`
  }
  return `${fmt(confirmedRange.value.startMs)} → ${fmt(confirmedRange.value.endMs)}`
})

const selectedRange = ref(props.range)
const prevRange = ref(props.range)
const showDatePicker = ref(false)
const openedFromDropdown = ref(false)

// 父级 range 变化（确认/切换/重启恢复）时同步下拉显示
watch(() => props.range, (v) => {
  selectedRange.value = v
  prevRange.value = v
})

// 选中「自定义时间范围」只打开弹窗，不切换当前 range
function onRangeSelect(v) {
  if (v === 'custom') {
    if (props.range !== 'custom') prevRange.value = props.range
    openedFromDropdown.value = true
    showDatePicker.value = true
    return
  }
  prevRange.value = v
  selectedRange.value = v
  emit('range-change', v)
}

function editRange() {
  openedFromDropdown.value = false
  showDatePicker.value = true
}

function onDatePickerClose() {
  showDatePicker.value = false
  // 从下拉框打开但未确认：回退到之前的 range
  if (openedFromDropdown.value) selectedRange.value = prevRange.value
  openedFromDropdown.value = false
}

function onDatePickerConfirm({ startMs, endMs }) {
  showDatePicker.value = false
  openedFromDropdown.value = false
  selectedRange.value = 'custom'
  localStorage.setItem('ocm_range', 'custom')
  localStorage.setItem('ocm_range_from', String(startMs))
  localStorage.setItem('ocm_range_to', String(endMs))
  emit('range-change', { v: 'custom', fromMs: startMs, toMs: endMs })
}

const statusText = computed(() => {
  if (!props.login?.logged_in) return '未登录 · 点击设置→账户登录'
  if (props.sync.syncing) return '正在同步云端日志…'
  if (props.sync.last_error) return `同步失败：${props.sync.last_error}`
  return `已同步 · ${fmtAgo(props.sync.last_sync_ms)} · ${fmtClock(props.sync.last_sync_ms)}`
})

async function doSync() {
  msg.value = ''
  try {
    const r = await invoke('sync_request_logs_now', { full: false })
    msg.value = `✓ 已处理 ${r.rows} 条`
    emit('sync-done')
  } catch (e) {
    msg.value = String(e)
  }
}

function isInteractive(el) { return el?.closest('button, select, input, a, .dd') }
function tryDrag(e) {
  if (e.button !== 0 || isInteractive(e.target)) return
  e.preventDefault()
  try { win?.startDragging() } catch {}
}
function onDblClick(e) {
  if (isInteractive(e.target)) return
  try { win?.toggleMaximize() } catch {}
}

onMounted(() => {})
</script>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 46px;
  box-sizing: border-box;
  padding: 0 20px;
  border-bottom: 1px solid var(--border);
  background: var(--panel);
  flex-shrink: 0;
}
.status-dot {
  width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;
  background: var(--muted);
}
.status-dot.ok { background: var(--green); box-shadow: 0 0 6px var(--green); }
.status-dot.syncing { background: var(--blue); }
.sync-text {
  color: var(--muted); font-size: 12px; white-space: nowrap;
  overflow: hidden; text-overflow: ellipsis; min-width: 0;
}
.err-badge {
  flex-shrink: 0; display: inline-flex; align-items: center; height: 20px;
  font-size: 12px; font-weight: 600; color: #ff6b6b;
  background: rgba(255, 107, 107, 0.12); border: 1px solid rgba(255, 107, 107, 0.35);
  border-radius: 6px; padding: 0 8px; white-space: nowrap; cursor: help;
}
.login-badge {
  flex-shrink: 0; display: inline-flex; align-items: center; height: 20px;
  font-size: 11px; color: var(--muted); cursor: pointer;
  background: var(--border); border-radius: 10px; padding: 0 9px; white-space: nowrap;
}
.login-badge.on { color: var(--green); }
.toolbar-right { margin-left: auto; display: flex; align-items: center; gap: 10px; flex-shrink: 0; }
.local-text { font-size: 12px; color: var(--muted); white-space: nowrap; max-width: 260px; overflow: hidden; text-overflow: ellipsis; }
.active-range {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--blue);
  background: rgba(79, 140, 255, .08);
  border: 1px solid rgba(79, 140, 255, .2);
  border-radius: 6px;
  padding: 4px 10px;
  white-space: nowrap;
  cursor: pointer;
  transition: background .12s;
}
.active-range:hover { background: rgba(79, 140, 255, .15); }
.range-edit-btn {
  width: 28px !important; height: 28px !important;
  flex-shrink: 0; border-radius: 6px;
  margin-left: -4px;
}
.range-edit-btn :deep(svg) { width: 13px; height: 13px; }
</style>
