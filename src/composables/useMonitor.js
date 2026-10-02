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
  // 启动时恢复 range；custom 若缺少 from/to 则回落到默认预设
  const savedRange = localStorage.getItem('ocm_range') || '1h'
  const savedFrom = Number(localStorage.getItem('ocm_range_from') || 0)
  const savedTo = Number(localStorage.getItem('ocm_range_to') || 0)
  const hasSavedCustom = savedRange === 'custom' && savedFrom > 0 && savedTo > savedFrom
  const savedModelsAccount = localStorage.getItem('ocm_models_account') || ''

  const stats = reactive({
    loading: false,
    range: hasSavedCustom ? 'custom' : (savedRange === 'custom' ? '1h' : savedRange),
    rangeFrom: hasSavedCustom ? savedFrom : 0,
    rangeTo: hasSavedCustom ? savedTo : 0,
    login: { logged_in: false, org_id: '' },
    accounts: [],
    models: [],
    defaultModelId: '',
    modelsAccount: savedModelsAccount,
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

  // 账号列表：标题栏 / 第二行卡片只用主账号；图表与第一行是全部账号聚合
  async function refreshAccounts() {
    try {
      const r = await invoke('list_accounts')
      const list = r?.accounts || []
      stats.accounts = list
    } catch {
      stats.accounts = []
    }
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
  // 永远不改变默认选中的模型（即使用量顺序变了）。accountId 为空 = 全部账号。
  let modelsLoaded = false
  let lastModelsRefresh = 0
  async function fetchModels() {
    const r = await invoke('get_models', { accountId: stats.modelsAccount || '' })
    return r?.models || []
  }
  async function loadModelsOnce() {
    if (modelsLoaded) return
    modelsLoaded = true
    try {
      const list = await fetchModels()
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
      const list = await fetchModels()
      if (list.length || stats.models.length) stats.models = list
    } catch {}
  }

  // 切换「当前模型请求限制」所属账号：持久化后立刻重拉（空串 = 全部账号）
  async function setModelsAccount(id) {
    stats.modelsAccount = id || ''
    localStorage.setItem('ocm_models_account', stats.modelsAccount)
    modelsLoaded = true
    lastModelsRefresh = Date.now()
    try {
      stats.models = await fetchModels()
    } catch {}
  }

  function persistRange() {
    localStorage.setItem('ocm_range', stats.range)
    if (stats.range === 'custom') {
      localStorage.setItem('ocm_range_from', String(stats.rangeFrom))
      localStorage.setItem('ocm_range_to', String(stats.rangeTo))
    }
  }

  async function refresh(range) {
    if (range && typeof range === 'object') {
      if (range.fromMs > 0 && range.toMs > range.fromMs) {
        stats.range = 'custom'
        stats.rangeFrom = range.fromMs
        stats.rangeTo = range.toMs
      }
    } else if (range) {
      stats.range = range
    }
    if (range) persistRange()
    const current = ++seq
    stats.loading = true
    try {
      const args = stats.range === 'custom'
        ? { range: 'custom', sinceMs: stats.rangeFrom, untilMs: stats.rangeTo }
        : { range: stats.range }
      const d = await invoke('get_dashboard', args)
      if (current !== seq) return
      stats.summary = d?.summary || null
      stats.series = d?.series || []
      stats.granularity = d?.granularity || 'day'
      if (d?.window_stats) stats.windowStats = d.window_stats
      if (d?.minute) stats.minute = d.minute
      stats.localRows = d?.local_rows || 0
      stats.localCost = d?.local_cost_micro_cents || 0
    } catch (e) {
      if (current === seq) console.error('get_dashboard failed:', e)
    } finally {
      if (current === seq) stats.loading = false
    }
    refreshQuota()
    refreshSync()
    refreshLogin()
    refreshAccounts()
    refreshModels()
  }

  return { stats, refresh, refreshQuota, refreshSync, refreshLogin, refreshAccounts, loadModelsOnce, refreshModels, setModelsAccount }
}
