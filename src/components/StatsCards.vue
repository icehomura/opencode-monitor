<template>
  <div class="cards-wrap">
    <!-- ── 第一行：当前分钟 / 词元数明细 / 时间窗口内 ── -->
    <section class="cards cards-3">
      <div class="card">
        <div class="card-title"><span>当前分钟</span></div>
        <div class="card-body">
          <div class="tok-grid">
            <div class="tok-cell tok-input">
              <span class="tok-label">输入 TPM</span>
              <span class="tok-num">{{ fmtTokens(minInput, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-output">
              <span class="tok-label">输出 TPM</span>
              <span class="tok-num">{{ fmtTokens(minOutput, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-cached">
              <span class="tok-label">缓存 TPM</span>
              <span class="tok-num">{{ fmtTokens(minCache, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-total">
              <span class="tok-label">合计 TPM</span>
              <span class="tok-num">{{ fmtTokens(minTotal, convertUnits) }}</span>
            </div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title"><span>词元数明细</span></div>
        <div class="card-body">
          <div class="tok-grid">
            <div class="tok-cell tok-input">
              <span class="tok-label">输入</span>
              <span class="tok-num">{{ fmtTokens(wsInput, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-output">
              <span class="tok-label">输出</span>
              <span class="tok-num">{{ fmtTokens(wsOutput, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-cached">
              <span class="tok-label">缓存</span>
              <span class="tok-num">{{ fmtTokens(wsCache, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-total">
              <span class="tok-label">合计</span>
              <span class="tok-num">{{ fmtTokens(wsTotal, convertUnits) }}</span>
            </div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title"><span>时间窗口内</span></div>
        <div class="card-body">
          <div class="tok-grid tok-grid-4">
            <div class="tok-cell tok-input">
              <span class="tok-label">总请求数</span>
              <span class="tok-num">{{ fmtTokens(ws.requests, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-output">
              <span class="tok-label">总输出词元数</span>
              <span class="tok-num">{{ fmtTokens(ws.output_tokens, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-cached">
              <span class="tok-label">金额</span>
              <span class="tok-num">{{ fmtUsdFine(wsCost) }}</span>
            </div>
            <div class="tok-cell tok-cached">
              <span class="tok-label">空闲时间</span>
              <span class="tok-num">{{ idleDisplay }}</span>
            </div>
            <div class="tok-cell tok-input">
              <span class="tok-label">平均 RPM</span>
              <span class="tok-num">{{ avgRpm }}</span>
            </div>
            <div class="tok-cell tok-output">
              <span class="tok-label">平均输出词元数/分</span>
              <span class="tok-num">{{ fmtTokens(avgTpm, convertUnits) }}</span>
            </div>
            <div class="tok-cell tok-cached">
              <span class="tok-label">平均金额/分</span>
              <span class="tok-num">{{ fmtUsdFine(avgCostPerMin) }}</span>
            </div>
            <div class="tok-cell tok-total">
              <span class="tok-label">时间利用率</span>
              <span class="tok-num">{{ utilization }}</span>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- ── 第二行：额度 / 当前模型请求限制 ── -->
    <section class="cards cards-4">
      <div class="card" v-for="m in meters" :key="m.key">
        <div class="card-title">
          <span class="title-group">{{ m.label }}</span>
          <span class="card-sub" v-if="m.resets_at">重置于 {{ fmtDateTime(m.resets_at) }}</span>
        </div>
        <div class="card-body">
          <div class="quota-main">
            <div class="quota-left">
              <span class="quota-label">剩余</span>
              <span class="quota-remain" :class="m.tone">{{ fmtUsd(m.remaining) }}</span>
              <span class="quota-limit">/ {{ fmtUsd(m.limit) }}</span>
            </div>
            <div class="quota-right">
              <span class="quota-used">已用 {{ fmtUsd(m.used) }}</span>
              <span class="quota-used">剩余 {{ m.pct.toFixed(0) }}%</span>
            </div>
          </div>
          <div class="bar">
            <div class="bar-fill" :class="m.tone" :style="{ width: m.pct + '%' }"></div>
          </div>
        </div>
      </div>

      <div class="card model-card">
        <div class="card-title model-title">
          <span class="title-group">当前模型请求限制</span>
          <div class="title-selects">
            <DdSelect
              class="card-dd"
              :options="accountOptions"
              v-model="accountId"
              @update:modelValue="onAccountChange"
            />
            <DdSelect
              class="card-dd"
              :options="modelOptions"
              v-model="selectedId"
              placeholder="请选择模型"
            />
          </div>
        </div>
        <div class="card-body model-body">
          <template v-if="selectedModel">
            <div class="req-rows">
              <div class="req-row" v-for="w in windows" :key="w.key">
                <span class="req-label">{{ w.label }}</span>
                <div class="req-bar">
                  <div
                    class="req-fill"
                    :class="toneOf(usedOf(w), limitOf(w))"
                    :style="{ width: pctOf(usedOf(w), limitOf(w)) + '%' }"
                  ></div>
                </div>
                <span class="req-text">{{ reqText(usedOf(w), limitOf(w)) }}</span>
              </div>
            </div>
          </template>
          <div v-else class="plan-empty">{{ models.length ? '请选择模型' : '暂无模型用量（登录后同步逐条日志）' }}</div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { fmtUsd, fmtUsdFine, fmtTokens, fmtDateTime, remainPct } from '../utils/format'
import DdSelect from './DdSelect.vue'

const props = defineProps({
  quota: { type: Object, default: null },
  reason: { type: String, default: '' },
  models: { type: Array, default: () => [] },
  defaultModelId: { type: String, default: '' },
  accounts: { type: Array, default: () => [] },
  modelsAccount: { type: String, default: '' },
  windowStats: { type: Object, default: () => ({ requests: 0, input_tokens: 0, output_tokens: 0, cache_read_tokens: 0, active_minutes: 0, window_minutes: 0 }) },
  minute: { type: Object, default: () => ({ input_tokens: 0, output_tokens: 0, cache_read_tokens: 0 }) },
  convertUnits: { type: Boolean, default: false },
})

const emit = defineEmits(['models-account-change'])

// 第二行（额度卡 + 当前模型请求限制）只显示主账号数据

// ── 第一行：词元统计 ──
const ws = computed(() => props.windowStats || {})
const wsCache = computed(() => Number(ws.value.cache_read_tokens || 0))
// API 的 inputTokens 本身就不含缓存，直接用，不要再减
const wsInput = computed(() => Number(ws.value.input_tokens || 0))
const wsOutput = computed(() => Number(ws.value.output_tokens || 0))
const wsTotal = computed(() => wsInput.value + wsOutput.value)
const wsCost = computed(() => Number(ws.value.cost_micro_cents || 0))
// 平均金额/分：用浮点算，避免整数除法把小额抹平
const avgCostPerMin = computed(() => wsCost.value / Math.max(1, Number(ws.value.window_minutes || 0)))

const minCache = computed(() => Number(props.minute?.cache_read_tokens || 0))
const minInput = computed(() => Number(props.minute?.input_tokens || 0))
const minOutput = computed(() => Number(props.minute?.output_tokens || 0))
const minTotal = computed(() => minInput.value + minOutput.value)

const idleDisplay = computed(() => {
  const total = Number(ws.value.window_minutes || 0)
  const active = Number(ws.value.active_minutes || 0)
  const m = Math.max(0, total - active)
  if (m >= 60) return (m / 60).toFixed(1) + ' 小时'
  return m + ' 分钟'
})
const avgRpm = computed(() => {
  const total = Number(ws.value.window_minutes || 0)
  if (!total) return '0'
  return Math.round(Number(ws.value.requests || 0) / total).toString()
})
const avgTpm = computed(() => {
  const total = Number(ws.value.window_minutes || 0)
  if (!total) return 0
  return Math.round(Number(ws.value.output_tokens || 0) / total)
})
const utilization = computed(() => {
  const total = Number(ws.value.window_minutes || 0)
  if (!total) return '0%'
  const pct = (Number(ws.value.active_minutes || 0) / total) * 100
  return pct >= 10 ? Math.round(pct) + '%' : pct.toFixed(1) + '%'
})

// ── 第二行：额度 ──
function tone(pct) {
  if (pct <= 20) return 'danger'
  if (pct <= 50) return 'warn'
  return 'ok'
}
function meter(key, label) {
  const m = props.quota?.[key]
  const used = Number(m?.used_micro_cents || 0)
  const limit = Number(m?.limit_micro_cents || 0)
  const pct = remainPct(used, limit)
  return { key, label, used, limit, remaining: Math.max(0, limit - used), resets_at: m?.resets_at, pct, tone: tone(pct) }
}
const meters = computed(() => [
  meter('five_hour', '5 小时额度'),
  meter('week', '本周额度'),
  meter('month', '本月额度'),
])

// ── 第二行：当前模型 ──
const windows = [
  { key: 'five_hour', label: '5 小时', limitKey: 'req_5h' },
  { key: 'week', label: '本周', limitKey: 'req_week' },
  { key: 'month', label: '本月', limitKey: 'req_month' },
]
const selectedId = ref('')
let initialized = false
watch(
  () => props.defaultModelId,
  (v) => {
    if (initialized || !v) return
    selectedId.value = v
    initialized = true
  },
  { immediate: true },
)
const modelOptions = computed(() => props.models.map((m) => ({ v: m.id, label: m.name || m.id })))
const selectedModel = computed(() => props.models.find((m) => m.id === selectedId.value) || null)

// 账号下拉：'' = 全部账号（用量合计，上限按各账号套餐求和）
const accountId = ref(props.modelsAccount || '')
watch(() => props.modelsAccount, (v) => { accountId.value = v || '' })
const accountOptions = computed(() => [
  { v: '', label: '全部账号' },
  ...props.accounts.map((a) => ({ v: a.id, label: a.name || a.id })),
])
function onAccountChange(v) {
  accountId.value = v || ''
  emit('models-account-change', accountId.value)
}

function usedOf(w) { return Number(selectedModel.value?.usage?.[w.key]?.requests || 0) }
function limitOf(w) { const l = selectedModel.value?.limit; return l ? Number(l[w.limitKey]) : null }
function unlimited(limit) { return limit === null || limit === undefined || limit < 0 }
function pctOf(used, limit) {
  if (unlimited(limit) || !limit) return 0
  return Math.max(0, Math.min(100, (used / limit) * 100))
}
function toneOf(used, limit) {
  if (unlimited(limit)) return 'ok'
  const p = pctOf(used, limit)
  if (p >= 80) return 'danger'
  if (p >= 50) return 'warn'
  return 'ok'
}
function reqText(used, limit) {
  if (unlimited(limit)) return `${used.toLocaleString()} / 不限`
  return `${used.toLocaleString()} / ${Number(limit).toLocaleString()}`
}
</script>

<style scoped>
.cards-wrap { display: flex; flex-direction: column; gap: 16px; }
.cards { display: grid; gap: 16px; flex-shrink: 0; }
.cards-3 { grid-template-columns: 2fr 2fr 3fr; }
.cards-4 { grid-template-columns: 1fr 1fr 1fr 1.5fr; }

.card {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 14px 18px;
  display: flex;
  flex-direction: column;
}
.card-title {
  display: flex; align-items: baseline; justify-content: space-between; gap: 8px;
  color: var(--muted); font-size: 12px; margin-bottom: 10px; line-height: 1;
}
.card-sub { font-size: 10px; color: var(--muted); }
.title-group { display: inline-flex; align-items: center; }
.card-body { flex: 1; display: flex; flex-direction: column; justify-content: center; gap: 10px; }

.tok-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px 20px; width: 100%; }
.tok-grid-3 { grid-template-columns: repeat(3, 1fr) !important; }
.tok-grid-4 { grid-template-columns: repeat(4, 1fr) !important; }
.tok-cell { display: flex; flex-direction: column; align-items: center; gap: 2px; }
.tok-label { font-size: 11px; color: var(--muted); }
.tok-num { font-size: 20px; font-weight: 700; line-height: 1.2; }
.tok-input .tok-num { color: var(--blue); }
.tok-output .tok-num { color: var(--green); }
.tok-cached .tok-num { color: var(--muted); }
.tok-total .tok-num { color: var(--text); }

.quota-main { display: flex; align-items: flex-end; justify-content: space-between; gap: 8px; }
.quota-left { display: flex; align-items: baseline; gap: 4px; }
.quota-label { font-size: 11px; color: var(--muted); }
.quota-remain { font-size: 26px; font-weight: 700; line-height: 1; }
.quota-limit { font-size: 12px; color: var(--muted); }
.quota-right { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; }
.quota-used { font-size: 11px; color: var(--muted); }
.quota-remain.ok { color: var(--green); }
.quota-remain.warn { color: #f0a020; }
.quota-remain.danger { color: #ff6b6b; }

.bar { height: 6px; width: 100%; border-radius: 3px; background: var(--border); overflow: hidden; }
.bar-fill { height: 100%; border-radius: 3px; transition: width .3s; }
.bar-fill.ok { background: var(--green); }
.bar-fill.warn { background: #f0a020; }
.bar-fill.danger { background: #ff6b6b; }

.plan-empty { font-size: 12px; color: var(--muted); }

.model-card { justify-content: flex-start; }
.model-title { align-items: center; margin-bottom: 12px; flex-wrap: wrap; }
.title-selects { display: inline-flex; align-items: center; gap: 8px; flex-shrink: 0; }
.card-dd { flex-shrink: 0; }
.card-dd :deep(.dd-btn) { padding: 3px 8px; font-size: 11px; min-width: 0; max-width: 140px; }
.card-dd :deep(.dd-label) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.model-body { gap: 12px; justify-content: center; }
.req-rows { display: flex; flex-direction: column; gap: 8px; width: 100%; }
.req-row { display: grid; grid-template-columns: 46px 1fr auto; align-items: center; gap: 8px; }
.req-label { font-size: 11px; color: var(--muted); }
.req-bar { height: 6px; border-radius: 3px; background: var(--border); overflow: hidden; }
.req-fill { height: 100%; border-radius: 3px; transition: width .3s; }
.req-fill.ok { background: var(--green); }
.req-fill.warn { background: #f0a020; }
.req-fill.danger { background: #ff6b6b; }
.req-text { font-size: 11px; color: var(--muted); white-space: nowrap; }
</style>
