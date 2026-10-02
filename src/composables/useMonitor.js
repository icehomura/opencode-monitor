import { reactive } from 'vue'
import { useTauri } from './useTauri'

const { invoke } = useTauri()

let pending = null
let seq = 0

/**
 * OpenCode 监控数据源：额度 + 云端汇总 + 成本曲线 + 同步状态。
 * Rust 侧通过 `quota-updated` / `sync-status` 事件通知刷新。
 */
export function useMonitor() {
  const stats = reactive({
    loading: false,
    range: localStorage.getItem('ocm_range') || '1h',
    login: { logged_in: false, org_id: '' },
    models: [],
    defaultModelId: '',
    windowStats: { requests: 0, input_tokens: 0, output_tokens: 0, cache_read_tokens: 0, active_minutes: 0, window_minutes: 0 },
    minute: { input_tokens: 0, output_tokens: 0, cache_read_tokens: 0 },
    quota: null,
    quotaReason: '',
    summary: null,
    series: [],
    granularity: 'day',
    localRows: 0,
    localCost: 0,
    sync: {
      configured: false,
      syncing: false,
      last_sync_ms: 0,
      last_full_sync_ms: 0,
      last_error: null,
      local_rows: 0,
      total_cost_micro_cents: 0,
      source_note: '',
      last_added: 0,
    },
  })

  async function refreshLogin() {
    try {
      stats.login = (await invoke('login_status')) || { logged_in: false, org_id: '' }
    } catch {}
  }

  async function refreshQuota() {
    try {
      const r = await invoke('get_quota')
      if (r?.available) {
        stats.quota = r.quota
        stats.quotaReason = ''
      } else {
        stats.quota = null
        stats.quotaReason = r?.reason || '额度不可用'
      }
    } catch (e) {
      stats.quota = null
      stats.quotaReason = String(e)
    }
  }

  async function refreshSync() {
    try {
      stats.sync = (await invoke('get_sync_status')) || stats.sync
    } catch {}
  }

  // 模型额度：启动时加载一次并确定默认选中项；之后只刷新用量数字，
  // 永远不改变默认选中的模型（即使用量顺序变了）。
  let modelsLoaded = false
  let lastModelsRefresh = 0
  async function loadModelsOnce() {
    if (modelsLoaded) return
    modelsLoaded = true
    try {
      const r = await invoke('get_models')
      const list = r?.models || []
      stats.models = list
      let best = ''
      let bestN = -1
      for (const m of list) {
        const n = Number(m?.usage?.month?.requests || 0)
        if (n > bestN) {
          bestN = n
          best = m.id
        }
      }
      stats.defaultModelId = best
    } catch {}
  }

  // 只更新用量数据，不动 defaultModelId（选中项由父组件/用户保持）
  async function refreshModels() {
    if (!modelsLoaded) return
    const now = Date.now()
    if (now - lastModelsRefresh < 5000) return
    lastModelsRefresh = now
    try {
      const list = (await invoke('get_models'))?.models || []
      if (list.length || stats.models.length) stats.models = list
    } catch {}
  }

  async function refresh(range) {
    if (range) {
      stats.range = range
      localStorage.setItem('ocm_range', range)
    }
    const current = ++seq
    stats.loading = true
    try {
      const d = await invoke('get_dashboard', { range: stats.range })
      if (current !== seq) return
      stats.summary = d?.summary || null
      stats.series = d?.series || []
      stats.granularity = d?.granularity || 'day'
      if (d?.window_stats) stats.windowStats = d.window_stats
      if (d?.minute) stats.minute = d.minute
      stats.localRows = d?.local_rows || 0
      stats.localCost = d?.local_cost_micro_cents || 0
      if (d && d.configured === false) stats.quotaReason = d.reason || ''
    } catch (e) {
      if (current === seq) console.error('get_dashboard failed:', e)
    } finally {
      if (current === seq) stats.loading = false
    }
    refreshQuota()
    refreshSync()
    refreshLogin()
    refreshModels()
  }

  return { stats, refresh, refreshQuota, refreshSync, refreshLogin, loadModelsOnce, refreshModels }
}
