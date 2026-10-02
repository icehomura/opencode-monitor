<template>
  <Teleport to="body">
    <transition name="dpm-fade" appear>
      <div v-if="visible" class="dpm-mask" @click.self="$emit('close')">
        <div class="dpm" @click.stop>
          <!-- 模式切换 -->
          <div class="dpm-tabs">
            <button
              v-for="t in modes"
              :key="t.k"
              :class="['dpm-tab', { active: mode === t.k }]"
              @click="switchMode(t.k)"
            >{{ t.l }}</button>
          </div>

          <!-- 结果预览 -->
          <div class="dpm-range">
            <span :class="['dpm-date', { active: isRange && activeField === 'start' }]">{{ preview.start }}</span>
            <span class="dpm-sep">{{ isRange ? '→' : '～' }}</span>
            <span :class="['dpm-date', { active: isRange && activeField === 'end' }]">{{ preview.end }}</span>
          </div>

          <!-- 日历 / 月份 -->
          <div class="dpm-cal-wrap">
            <div class="dpm-cal-header">
              <button class="dpm-arrow" @click="prev">&lt;</button>
              <span class="dpm-cal-title">{{ calTitle }}</span>
              <button class="dpm-arrow" @click="next">&gt;</button>
            </div>

            <div v-if="showMonthGrid" class="dpm-month-grid">
              <button
                v-for="m in 12"
                :key="m"
                :class="['dpm-month-item', { active: month === m }]"
                @click="month = m"
              >{{ m }}月</button>
            </div>

            <div v-else class="dpm-day-grid">
              <div v-for="w in dayHeaders" :key="w" class="dpm-weekday">{{ w }}</div>
              <button
                v-for="(d, i) in calendarDays"
                :key="i"
                :class="dayClass(d)"
                :disabled="d.disabled"
                @click="selectDate(d)"
              >{{ d.day }}</button>
            </div>
          </div>

          <!-- 时间（仅指定范围模式） -->
          <div v-if="isRange" class="dpm-time-row">
            <div class="dpm-time-field">
              <span class="dpm-time-label" :class="{ active: activeField === 'start' }" @click="activeField = 'start'">开始时间</span>
              <div class="dpm-time-select">
                <select :value="startH" @change="startH = +$event.target.value" class="dpm-time-sel">
                  <option v-for="h in 24" :key="h - 1" :value="h - 1">{{ pad(h - 1) }}</option>
                </select>
                <span class="dpm-time-colon">:</span>
                <select :value="startM" @change="startM = +$event.target.value" class="dpm-time-sel">
                  <option v-for="m in 60" :key="m - 1" :value="m - 1">{{ pad(m - 1) }}</option>
                </select>
              </div>
            </div>
            <div class="dpm-time-field">
              <span class="dpm-time-label" :class="{ active: activeField === 'end' }" @click="activeField = 'end'">结束时间</span>
              <div class="dpm-time-select">
                <select :value="endH" @change="endH = +$event.target.value" class="dpm-time-sel">
                  <option v-for="h in 24" :key="h - 1" :value="h - 1">{{ pad(h - 1) }}</option>
                </select>
                <span class="dpm-time-colon">:</span>
                <select :value="endM" @change="endM = +$event.target.value" class="dpm-time-sel">
                  <option v-for="m in 60" :key="m - 1" :value="m - 1">{{ pad(m - 1) }}</option>
                </select>
              </div>
            </div>
          </div>

          <div v-if="errMsg" class="dpm-err">{{ errMsg }}</div>

          <!-- 页脚 -->
          <div class="dpm-footer">
            <button class="dpm-btn dpm-btn-cancel" @click="$emit('close')">取消</button>
            <button class="dpm-btn dpm-btn-ok" :disabled="!!errMsg" @click="onConfirm">确认</button>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<script setup>
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'

const props = defineProps({
  visible: { type: Boolean, default: false },
  fromMs: { type: Number, default: 0 },
  toMs: { type: Number, default: 0 },
})
const emit = defineEmits(['confirm', 'close'])

const modes = [
  { k: 'day', l: '日期' },
  { k: 'week', l: '周' },
  { k: 'month', l: '月份' },
  { k: 'range', l: '指定范围' },
]

const mode = ref('day')
const year = ref(0)
const month = ref(0)
const dateStart = ref(null) // 'YYYY-MM-DD'
const dateEnd = ref(null)
const startH = ref(0)
const startM = ref(0)
const endH = ref(0)
const endM = ref(0)
const activeField = ref('start')

const dayHeaders = ['一', '二', '三', '四', '五', '六', '日']
const pad = (n) => String(n).padStart(2, '0')

// ---- 打开时重置 / 预填 ----
watch(() => props.visible, (v) => {
  if (!v) return
  if (props.fromMs > 0 && props.toMs > props.fromMs) {
    const s = new Date(props.fromMs)
    const e = new Date(props.toMs)
    mode.value = 'range'
    year.value = s.getFullYear()
    month.value = s.getMonth() + 1
    dateStart.value = toDate(s)
    dateEnd.value = toDate(e)
    startH.value = s.getHours(); startM.value = s.getMinutes()
    endH.value = e.getHours(); endM.value = e.getMinutes()
    activeField.value = 'start'
    return
  }
  const d = new Date()
  mode.value = 'day'
  year.value = d.getFullYear()
  month.value = d.getMonth() + 1
  dateStart.value = toDate(d)
  dateEnd.value = null
  startH.value = d.getHours(); startM.value = d.getMinutes()
  endH.value = d.getHours(); endM.value = Math.min(d.getMinutes() + 5, 59)
  activeField.value = 'start'
})

function switchMode(v) {
  mode.value = v
  if (v === 'range') {
    if (!dateStart.value) dateStart.value = toDate(new Date())
    if (!dateEnd.value) dateEnd.value = dateStart.value
    activeField.value = 'start'
  } else if (v === 'month') {
    // 月份模式直接在当前年月上选择
  } else {
    // 日期 / 周：以当前选中日期为锚点，缺省用今天
    if (!dateStart.value) dateStart.value = toDate(new Date())
    dateEnd.value = null
    const d = new Date(dateStart.value + 'T00:00:00')
    year.value = d.getFullYear()
    month.value = d.getMonth() + 1
  }
}

// ---- 导航 ----
function prev() {
  if (showMonthGrid.value) {
    year.value--
    return
  }
  month.value--
  if (month.value < 1) { month.value = 12; year.value-- }
}
function next() {
  if (showMonthGrid.value) {
    year.value++
    return
  }
  month.value++
  if (month.value > 12) { month.value = 1; year.value++ }
}

const showMonthGrid = computed(() => mode.value === 'month')
const isRange = computed(() => mode.value === 'range')
const calTitle = computed(() => (showMonthGrid.value ? `${year.value}年` : `${year.value}年${month.value}月`))

// ---- 日历计算 ----
const calendarDays = computed(() => {
  const y = year.value, m = month.value
  const firstDow = new Date(y, m - 1, 1).getDay()
  const startIdx = firstDow === 0 ? 6 : firstDow - 1
  const dim = new Date(y, m, 0).getDate()
  const prevDim = new Date(y, m - 1, 0).getDate()

  const days = []
  for (let i = startIdx; i > 0; i--) days.push({ day: prevDim - i + 1, date: '', disabled: true })
  for (let d = 1; d <= dim; d++) days.push({ day: d, date: toDate(new Date(y, m - 1, d)), disabled: false })
  let nd = 1
  while (days.length % 7 !== 0) days.push({ day: nd++, date: '', disabled: true })
  return days
})

function dayClass(d) {
  if (!d.date) return 'dpm-day disabled'
  const cls = ['dpm-day']
  if (mode.value === 'week' && dateStart.value) {
    const ms = toMonday(dateStart.value)
    const me = addDays(ms, 6)
    if (d.date >= ms && d.date <= me) {
      cls.push(d.date === ms ? 'hl-start' : d.date === me ? 'hl-end' : 'hl-mid')
      return cls.join(' ')
    }
  } else if (mode.value === 'range') {
    if (dateStart.value && dateEnd.value) {
      const ms = dateStart.value <= dateEnd.value ? dateStart.value : dateEnd.value
      const me = dateStart.value <= dateEnd.value ? dateEnd.value : dateStart.value
      if (d.date >= ms && d.date <= me) {
        cls.push(d.date === ms ? 'hl-start' : d.date === me ? 'hl-end' : 'hl-mid')
        return cls.join(' ')
      }
    } else if (d.date === dateStart.value) {
      cls.push('hl-start')
    }
  } else if (d.date === dateStart.value) {
    cls.push('hl-start')
  }
  return cls.join(' ')
}

function selectDate(d) {
  if (d.disabled || !d.date) return
  if (mode.value !== 'range') {
    dateStart.value = d.date
    dateEnd.value = null
    return
  }
  if (activeField.value === 'start') {
    dateStart.value = d.date
    if (!dateEnd.value || dateEnd.value < dateStart.value) dateEnd.value = dateStart.value
    activeField.value = 'end'
  } else {
    dateEnd.value = d.date
    if (dateEnd.value < dateStart.value) {
      const tmp = dateStart.value
      dateStart.value = dateEnd.value
      dateEnd.value = tmp
    }
    activeField.value = 'start'
  }
}

// ---- 预览 ----
const fmt = (v) => v || '—'
const fmtT = (v, h, m) => (v ? `${v} ${pad(h)}:${pad(m)}` : '—')

const preview = computed(() => {
  if (mode.value === 'day') return { start: fmt(dateStart.value), end: fmt(addDays(dateStart.value, 1)) }
  if (mode.value === 'week') {
    if (!dateStart.value) return { start: '—', end: '—' }
    const m = toMonday(dateStart.value)
    return { start: fmt(m), end: fmt(addDays(m, 7)) }
  }
  if (mode.value === 'month') {
    const last = new Date(year.value, month.value, 0)
    return { start: `${year.value}-${pad(month.value)}-01`, end: fmt(addDays(toDate(last), 1)) }
  }
  return { start: fmtT(dateStart.value, startH.value, startM.value), end: fmtT(dateEnd.value, endH.value, endM.value) }
})

// ---- 时间戳（左闭右开） ----
function computeRange() {
  const day = dateStart.value || toDate(new Date())
  if (mode.value === 'day') {
    const s = new Date(day + 'T00:00:00').getTime()
    return { startMs: s, endMs: s + 86400000 }
  }
  if (mode.value === 'week') {
    const s = new Date(toMonday(day) + 'T00:00:00').getTime()
    return { startMs: s, endMs: s + 604800000 }
  }
  if (mode.value === 'month') {
    return {
      startMs: new Date(year.value, month.value - 1, 1).getTime(),
      endMs: new Date(year.value, month.value, 1).getTime(),
    }
  }
  if (!dateStart.value || !dateEnd.value) return null
  const s = new Date(`${dateStart.value}T${pad(startH.value)}:${pad(startM.value)}:00`).getTime()
  const e = new Date(`${dateEnd.value}T${pad(endH.value)}:${pad(endM.value)}:00`).getTime()
  return { startMs: s, endMs: e }
}

const errMsg = computed(() => {
  const r = computeRange()
  if (!r) return '请选择开始与结束日期'
  if (r.endMs <= r.startMs) return '结束时间必须晚于开始时间'
  return ''
})

function onConfirm() {
  const r = computeRange()
  if (!r || r.endMs <= r.startMs) return
  emit('confirm', { startMs: r.startMs, endMs: r.endMs })
}

// ---- 工具函数 ----
function toDate(d) {
  if (typeof d === 'string') return d
  return d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate())
}
function addDays(s, n) {
  if (!s) return ''
  const d = new Date(s + 'T00:00:00')
  d.setDate(d.getDate() + n)
  return toDate(d)
}
function toMonday(date) {
  const d = new Date(date + 'T00:00:00')
  const dow = d.getDay()
  d.setDate(d.getDate() + (dow === 0 ? -6 : 1 - dow))
  return toDate(d)
}

function onKeydown(e) {
  if (e.key === 'Escape' && props.visible) emit('close')
}
onMounted(() => document.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => document.removeEventListener('keydown', onKeydown))
</script>

<style scoped>
/* ── 遮罩 + 面板 ── */
.dpm-mask {
  position: fixed; inset: 0; z-index: 1000;
  display: flex; align-items: center; justify-content: center;
  background: rgba(0, 0, 0, .45); backdrop-filter: blur(4px);
}
.dpm {
  background: var(--panel); border: 1px solid var(--border);
  border-radius: 12px; padding: 18px;
  width: 360px; max-height: 80vh; overflow-y: auto;
  box-shadow: 0 8px 24px rgba(0, 0, 0, .35);
  animation: dpmSlideUp .22s ease-out;
}
@keyframes dpmSlideUp {
  from { opacity: 0; transform: translateY(12px) scale(.97); }
  to   { opacity: 1; transform: translateY(0) scale(1); }
}

/* ── 模式选项卡 ── */
.dpm-tabs {
  display: flex; gap: 4px;
  background: var(--bg); border-radius: 8px; padding: 3px;
  margin-bottom: 14px;
}
.dpm-tab {
  flex: 1; padding: 5px 0; border: none; border-radius: 6px;
  background: transparent; font-size: 13px; font-weight: 500;
  color: var(--muted); cursor: pointer; transition: all .15s;
}
.dpm-tab.active { background: var(--blue); color: #fff; }

/* ── 范围预览 ── */
.dpm-range {
  display: flex; align-items: center; justify-content: center;
  gap: 10px; margin-bottom: 14px; font-size: 14px; font-weight: 600;
  color: var(--text); white-space: nowrap;
}
.dpm-date { padding: 3px 8px; border-radius: 6px; transition: background .12s; }
.dpm-date.active { background: var(--blue); color: #fff; }
.dpm-sep { color: var(--muted); font-size: 13px; font-weight: 400; }

/* ── 日历区域 ── */
.dpm-cal-header {
  display: flex; align-items: center; justify-content: space-between;
  margin-bottom: 8px;
}
.dpm-cal-title { font-size: 14px; font-weight: 600; color: var(--text); }
.dpm-arrow {
  width: 28px; height: 28px; border: none; border-radius: 6px;
  background: transparent; font-size: 16px; cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  color: var(--muted); transition: background .12s;
}
.dpm-arrow:hover { background: var(--border); }

/* ── 月份网格 ── */
.dpm-month-grid {
  display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px; margin-bottom: 12px;
}
.dpm-month-item {
  padding: 8px 0; border: none; border-radius: 8px;
  background: var(--bg); font-size: 13px; font-weight: 500;
  color: var(--muted); cursor: pointer; transition: all .12s;
}
.dpm-month-item.active { background: var(--blue); color: #fff; }
.dpm-month-item:hover:not(.active) { background: var(--border); }

/* ── 日历日期 ── */
.dpm-day-grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; margin-bottom: 12px; }
.dpm-weekday { text-align: center; font-size: 11px; font-weight: 500; color: var(--muted); padding: 4px 0; }
.dpm-day {
  aspect-ratio: 1; display: flex; align-items: center; justify-content: center;
  border: none; border-radius: 8px; font-size: 13px; font-weight: 500;
  background: transparent; color: var(--text); cursor: pointer; transition: all .12s;
}
.dpm-day:not(.disabled):hover { background: var(--border); }
.dpm-day.disabled { color: var(--muted); opacity: .35; cursor: default; }
.dpm-day.hl-start { background: var(--blue); color: #fff; border-radius: 8px 0 0 8px; }
.dpm-day.hl-mid   { background: rgba(79, 140, 255, .18); color: var(--text); border-radius: 0; }
.dpm-day.hl-end   { background: var(--blue); color: #fff; border-radius: 0 8px 8px 0; }
.dpm-day.hl-start.hl-end { border-radius: 8px; }

/* ── 时间选择 ── */
.dpm-time-row { display: flex; gap: 16px; margin-bottom: 12px; }
.dpm-time-field { display: flex; flex-direction: column; gap: 4px; flex: 1; }
.dpm-time-label {
  font-size: 11px; font-weight: 500; color: var(--muted); padding-left: 2px;
  cursor: pointer; transition: color .12s;
}
.dpm-time-label.active { color: var(--blue); }
.dpm-time-select { display: flex; align-items: center; gap: 3px; }
.dpm-time-sel {
  width: 52px; padding: 4px 0; border: 1px solid var(--border); border-radius: 6px;
  background: var(--bg); font-size: 13px; text-align: center;
  color: var(--text); cursor: pointer;
}
.dpm-time-colon { font-size: 14px; font-weight: 600; color: var(--muted); }

/* ── 校验提示 ── */
.dpm-err {
  font-size: 12px; color: #ff6b6b;
  background: rgba(255, 107, 107, .12);
  border: 1px solid rgba(255, 107, 107, .35);
  border-radius: 6px; padding: 5px 8px; margin-bottom: 10px;
}

/* ── 页脚 ── */
.dpm-footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
.dpm-btn {
  padding: 7px 22px; border: none; border-radius: 8px;
  font-size: 13px; font-weight: 500; cursor: pointer; transition: opacity .15s;
}
.dpm-btn:disabled { opacity: .4; cursor: not-allowed; }
.dpm-btn-cancel { background: var(--bg); color: var(--muted); }
.dpm-btn-cancel:hover { opacity: .75; }
.dpm-btn-ok { background: var(--blue); color: #fff; }
.dpm-btn-ok:hover:not(:disabled) { opacity: .88; }

/* ── 过渡 ── */
.dpm-fade-enter-active { transition: opacity .18s; }
.dpm-fade-leave-active { transition: opacity .12s; }
.dpm-fade-enter-from, .dpm-fade-leave-to { opacity: 0; }
</style>
